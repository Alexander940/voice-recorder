//! Orquesta un dictado: atajo → audio → transcripción → limpieza → salida.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Runtime, State};

use crate::audio::{Chunker, CpalSource, Recorder, Segment, Stopped};
use crate::errors::{AppError, Lang};
use crate::history::{self, History, NewEntry};
use crate::hotkey::{self, DictationAction};
use crate::llm::{AnthropicCleaner, CleanupOutcome, TextCleaner, clean_or_raw};
use crate::logging::Redactions;
use crate::longform::{self, CHUNK_LEN, CHUNK_OVERLAP, LongText};
use crate::modes::{Mode, Modes};
use crate::output::{
    Delivery, EnigoKeys, Notifier, OutputMode, Paster, SystemClipboard, TauriNotifier,
};
use crate::settings::keys::Provider;
use crate::settings::{AppKeys, SettingsState};
use crate::sounds::{self, Sound};
use crate::stt::{GroqTranscriber, TranscribeOptions, Transcriber};
use crate::tray::{self, TrayEvent};
use crate::vocabulary::{Vocabulary, VocabularyStore};

pub const OVERLAY_WINDOW: &str = "overlay";

#[derive(Default)]
pub struct DictationState {
    recorder: Mutex<Option<Recorder<CpalSource>>>,
    /// Trozos de la grabación en curso que se transcriben mientras se graba.
    session: Mutex<Option<Arc<LongSession>>>,
    retry: RetryBuffer,
    /// Se incrementa con cada cambio del overlay: un ocultado diferido solo
    /// actúa si nadie lo ha cambiado desde entonces.
    overlay_generation: AtomicU64,
    /// Momento en que terminó la grabación, para medir el procesado.
    stopped_at: Mutex<Option<Instant>>,
}

/// Fase del procesado que muestra el overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Transcribing,
    Cleaning,
}

/// Lo que muestra el overlay (evento `overlay-state`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum OverlayState {
    Recording {
        mode: String,
        shortcut: String,
        toggle: bool,
    },
    Processing {
        stage: Stage,
        mode: String,
    },
    Pasted {
        words: usize,
        ms: u64,
        copied: bool,
    },
    Error {
        title: String,
        detail: String,
        retry: bool,
    },
}

/// Resultado de un dictado entregado (evento `dictation-result`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DictationResult {
    pub text: String,
    pub mode: String,
    pub audio_ms: u64,
    pub processing_ms: u64,
}

/// Cuánto sigue visible el overlay tras pegar y tras un error.
const PASTED_VISIBLE: Duration = Duration::from_millis(1500);
const ERROR_VISIBLE: Duration = Duration::from_secs(6);

pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Cada cuánto mira el trabajador si ya hay un trozo completo.
const WORKER_TICK: Duration = Duration::from_millis(250);

/// Lo que queda de un dictado que falló por un servicio, para reintentarlo.
#[derive(Debug, Clone)]
pub enum Pending {
    /// Dictado corto: todo su audio.
    Short(Vec<u8>),
    /// Dictado largo: el texto ya obtenido y el audio de los trozos que faltan.
    Long(Box<LongPending>),
}

#[derive(Debug, Clone)]
pub struct LongPending {
    pub text: LongText,
    pub wavs: Vec<Vec<u8>>,
    pub mode: Mode,
    pub total_ms: u64,
}

#[derive(Default)]
pub struct RetryBuffer(Mutex<Option<Pending>>);

impl RetryBuffer {
    pub fn store(&self, wav: Vec<u8>) {
        self.store_pending(Pending::Short(wav));
    }
    pub fn store_pending(&self, pending: Pending) {
        *self.0.lock().expect("retry poisoned") = Some(pending);
    }
    pub fn take_pending(&self) -> Option<Pending> {
        self.0.lock().expect("retry poisoned").take()
    }
    pub fn has_pending(&self) -> bool {
        self.0.lock().expect("retry poisoned").is_some()
    }
}

/// Lo acumulado por el trabajador de trozos.
#[derive(Default)]
struct LongProgress {
    text: LongText,
    /// Primer error: a partir de ahí los trozos se guardan sin procesar.
    failed: Option<AppError>,
    pending: Vec<Vec<u8>>,
}

/// Transcripción por trozos de la grabación en curso (dictados largos).
struct LongSession {
    /// Modo al empezar: todos los trozos se procesan igual.
    mode: Mode,
    stop: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
    progress: Mutex<LongProgress>,
}

/// Lo necesario para transcribir y limpiar con los ajustes actuales.
struct Pipeline {
    transcriber: GroqTranscriber,
    cleaner: Option<AnthropicCleaner>,
    options: TranscribeOptions,
    instructions: String,
    vocabulary: Vocabulary,
    mode: Mode,
    copy_only: bool,
}

impl Pipeline {
    fn cleaner(&self) -> Option<&dyn TextCleaner> {
        self.cleaner.as_ref().map(|c| c as &dyn TextCleaner)
    }
}

fn pipeline_for<R: Runtime>(app: &AppHandle<R>, mode: Mode) -> Result<Pipeline, AppError> {
    let keys = app.state::<AppKeys>();
    let groq_key = keys
        .get(Provider::Groq)
        .ok_or(AppError::MissingKey(Provider::Groq))?;
    let vocabulary = app.state::<VocabularyStore>().get();
    let settings = app.state::<SettingsState>().get();
    let cleaner = mode
        .wants_llm(settings.ai.cleanup)
        .then(|| keys.get(Provider::Anthropic))
        .flatten()
        .map(AnthropicCleaner::new);
    let instructions = [mode.instructions(), vocabulary.llm_instructions()].join("\n");
    Ok(Pipeline {
        transcriber: GroqTranscriber::new(groq_key),
        cleaner,
        options: TranscribeOptions {
            model: settings.transcription.model.clone(),
            language: settings.transcription.language.clone(),
            prompt: vocabulary.whisper_prompt(),
        },
        instructions,
        vocabulary,
        mode,
        copy_only: settings.output.copy_only,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordingFinished {
    pub bytes: usize,
}

/// Resultado de transcribir y limpiar.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineOutput {
    pub raw: String,
    pub text: String,
    pub cleaned: bool,
    pub stt_latency_ms: i64,
    pub llm_latency_ms: Option<i64>,
}

/// Núcleo del pipeline, sin Tauri: transcribe, limpia y aplica el vocabulario.
/// `Ok(None)` = no se dijo nada.
pub fn run_pipeline(
    transcriber: &dyn Transcriber,
    cleaner: Option<&dyn TextCleaner>,
    wav: &[u8],
    options: &TranscribeOptions,
    instructions: &str,
    vocabulary: &Vocabulary,
) -> Result<Option<PipelineOutput>, AppError> {
    run_pipeline_with_stages(
        transcriber,
        cleaner,
        wav,
        options,
        instructions,
        vocabulary,
        &|_| {},
    )
}

/// Como `run_pipeline`, avisando de cada fase (para el overlay).
pub fn run_pipeline_with_stages(
    transcriber: &dyn Transcriber,
    cleaner: Option<&dyn TextCleaner>,
    wav: &[u8],
    options: &TranscribeOptions,
    instructions: &str,
    vocabulary: &Vocabulary,
    on_stage: &dyn Fn(Stage),
) -> Result<Option<PipelineOutput>, AppError> {
    on_stage(Stage::Transcribing);
    let transcript = transcriber.transcribe(wav, options)?;
    if transcript.text.is_empty() {
        return Ok(None);
    }
    if cleaner.is_some() {
        on_stage(Stage::Cleaning);
    }
    let started = Instant::now();
    let outcome = clean_or_raw(cleaner, &transcript.text, instructions);
    Ok(Some(PipelineOutput {
        text: vocabulary.apply_replacements(outcome.text()),
        cleaned: matches!(outcome, CleanupOutcome::Cleaned(_)),
        raw: transcript.text,
        stt_latency_ms: transcript.latency.as_millis() as i64,
        llm_latency_ms: cleaner.map(|_| started.elapsed().as_millis() as i64),
    }))
}

pub fn on_action<R: Runtime>(app: &AppHandle<R>, action: DictationAction) {
    match action {
        DictationAction::Start => start(app),
        DictationAction::Stop => stop(app),
        DictationAction::Cancel => cancel(app),
    }
}

fn sound<R: Runtime>(app: &AppHandle<R>, s: Sound) {
    if app.state::<SettingsState>().get().feedback.sounds {
        sounds::play(s);
    }
}

/// Notificación nativa, evento para la UI y sonido de error.
fn report<R: Runtime>(app: &AppHandle<R>, error: &AppError, retry: bool) {
    let lang = Lang::system();
    let (title, body) = error.notification(lang);
    let (short, detail) = error.overlay(lang);
    set_overlay(
        app,
        OverlayState::Error {
            title: short,
            detail,
            retry,
        },
    );
    hide_overlay_after(app, ERROR_VISIBLE);
    TauriNotifier(app.clone()).notify(&title, &body);
    let _ = app.emit("dictation-error", format!("{title}: {body}"));
    sound(app, Sound::Error);
    tracing::warn!(error = ?error, "dictado fallido");
}

fn start<R: Runtime>(app: &AppHandle<R>) {
    let level_app = app.clone();
    let limit_app = app.clone();
    let settings = app.state::<SettingsState>().get();
    let result = CpalSource::new(settings.audio.device.as_deref()).and_then(|source| {
        Recorder::start(
            source,
            Duration::from_secs(settings.audio.max_duration_secs),
            move |e| {
                let _ = level_app.emit_to(OVERLAY_WINDOW, "audio-level", e);
            },
            // Se llama desde el hilo de audio: parar desde otro hilo evita bloquearlo.
            move || {
                std::thread::spawn(move || hotkey::force_stop(&limit_app));
            },
        )
    });
    match result {
        Ok(recorder) => {
            let mode = app.state::<Modes>().active();
            let session = Arc::new(LongSession {
                mode: mode.clone(),
                stop: AtomicBool::new(false),
                worker: Mutex::new(None),
                progress: Mutex::default(),
            });
            let chunker = recorder.chunker();
            let worker = {
                let app = app.clone();
                let session = session.clone();
                std::thread::spawn(move || chunk_worker(&app, chunker, &session))
            };
            *session.worker.lock().expect("session poisoned") = Some(worker);
            let state = app.state::<DictationState>();
            *state.recorder.lock().expect("dictation poisoned") = Some(recorder);
            *state.session.lock().expect("dictation poisoned") = Some(session);
            let _ = tray::apply(app, TrayEvent::StartRecording);
            let mode = mode.name;
            let _ = app.emit_to(OVERLAY_WINDOW, "mode-changed", &mode);
            set_overlay(
                app,
                OverlayState::Recording {
                    mode,
                    shortcut: settings.hotkey.shortcut.clone(),
                    toggle: settings.hotkey.mode == hotkey::HotkeyMode::Toggle,
                },
            );
            sound(app, Sound::Start);
        }
        Err(e) => {
            hotkey::reset(app);
            let _ = tray::apply(app, TrayEvent::Error);
            report(app, &AppError::from(e), false);
        }
    }
}

fn take_recorder<R: Runtime>(app: &AppHandle<R>) -> Option<Recorder<CpalSource>> {
    app.state::<DictationState>()
        .recorder
        .lock()
        .expect("dictation poisoned")
        .take()
}

fn take_session<R: Runtime>(app: &AppHandle<R>) -> Option<Arc<LongSession>> {
    app.state::<DictationState>()
        .session
        .lock()
        .expect("dictation poisoned")
        .take()
}

/// Mientras se graba, cada `CHUNK_LEN` saca un trozo y lo transcribe en segundo
/// plano, para que al parar solo falte el último.
fn chunk_worker<R: Runtime>(app: &AppHandle<R>, chunker: Chunker, session: &LongSession) {
    while !session.stop.load(Ordering::SeqCst) {
        std::thread::sleep(WORKER_TICK);
        let Some(segment) = chunker.take(CHUNK_LEN, CHUNK_OVERLAP) else {
            continue;
        };
        let chunks = transcribe_segment(app, session, segment);
        let _ = app.emit_to(OVERLAY_WINDOW, "dictation-progress", chunks);
    }
}

/// Transcribe un trozo y lo añade al texto. Tras el primer fallo, los trozos se
/// guardan sin procesar para reintentarlos. Devuelve los trozos transcritos.
fn transcribe_segment<R: Runtime>(
    app: &AppHandle<R>,
    session: &LongSession,
    segment: Segment,
) -> usize {
    let wav = match segment.encode() {
        Ok(wav) => wav,
        Err(e) => {
            let mut progress = session.progress.lock().expect("session poisoned");
            progress.failed = Some(AppError::Transcription(e.to_string()));
            return progress.text.chunks;
        }
    };
    let mut progress = session.progress.lock().expect("session poisoned");
    if progress.failed.is_some() {
        progress.pending.push(wav);
        return progress.text.chunks;
    }
    // La red puede tardar minutos: sin el candado mientras tanto.
    let mut text = std::mem::take(&mut progress.text);
    drop(progress);
    let result = pipeline_for(app, session.mode.clone()).and_then(|p| {
        longform::process_chunk(
            &p.transcriber,
            p.cleaner(),
            &wav,
            &p.options,
            p.instructions.trim(),
            &p.vocabulary,
            &mut text,
            &std::thread::sleep,
        )
    });
    let mut progress = session.progress.lock().expect("session poisoned");
    progress.text = text;
    if let Err(e) = result {
        tracing::warn!(error = ?e, "trozo fallido; se guarda para reintentar");
        progress.failed = Some(e);
        progress.pending.push(wav);
    }
    progress.text.chunks
}

fn stop<R: Runtime>(app: &AppHandle<R>) {
    let session = take_session(app);
    let Some(recorder) = take_recorder(app) else {
        if let Some(s) = session {
            s.stop.store(true, Ordering::SeqCst);
        }
        hide_overlay(app);
        return;
    };
    let stopped = recorder.stop_chunked();
    if let Ok(Stopped {
        chunked: true,
        rest,
        total,
    }) = stopped
    {
        // Dictado largo: falta el último trozo y lo que no se pudo procesar.
        *app.state::<DictationState>()
            .stopped_at
            .lock()
            .expect("dictation poisoned") = Some(Instant::now());
        set_overlay(
            app,
            OverlayState::Processing {
                stage: Stage::Transcribing,
                mode: app.state::<Modes>().active().name,
            },
        );
        sound(app, Sound::Stop);
        let _ = tray::apply(app, TrayEvent::StopRecording);
        let app = app.clone();
        std::thread::spawn(move || finish_long(&app, session, rest, total));
        return;
    }
    // Dictado corto: el trabajador no llegó a sacar ningún trozo.
    if let Some(s) = session {
        s.stop.store(true, Ordering::SeqCst);
    }
    match stopped.and_then(|s| s.rest.map(|r| r.encode()).transpose()) {
        Ok(Some(wav)) => {
            *app.state::<DictationState>()
                .stopped_at
                .lock()
                .expect("dictation poisoned") = Some(Instant::now());
            set_overlay(
                app,
                OverlayState::Processing {
                    stage: Stage::Transcribing,
                    mode: app.state::<Modes>().active().name,
                },
            );
            sound(app, Sound::Stop);
            let _ = tray::apply(app, TrayEvent::StopRecording);
            save_debug_wav(app, &wav);
            let _ = app.emit("recording-finished", RecordingFinished { bytes: wav.len() });
            // La red no debe bloquear el hilo del atajo.
            let app = app.clone();
            std::thread::spawn(move || process(&app, wav));
        }
        Ok(None) => {
            // Demasiado corto: se descarta en silencio.
            hide_overlay(app);
            let _ = tray::apply(app, TrayEvent::Cancel);
        }
        Err(e) => {
            let _ = tray::apply(app, TrayEvent::Error);
            report(app, &AppError::from(e), false);
        }
    }
}

/// Transcribe y entrega el texto. Corre en su propio hilo.
fn process<R: Runtime>(app: &AppHandle<R>, wav: Vec<u8>) {
    let retry = &app.state::<DictationState>().retry;
    match transcribe_and_deliver(app, &wav) {
        Ok(delivered) => {
            let _ = tray::apply(app, TrayEvent::ProcessingDone);
            let _ = app.emit("dictation-retry-available", false);
            match delivered {
                Some(state) => {
                    set_overlay(app, state);
                    hide_overlay_after(app, PASTED_VISIBLE);
                }
                None => hide_overlay(app),
            }
        }
        Err(error) => {
            let _ = tray::apply(app, TrayEvent::Error);
            let retryable = error.retryable();
            if retryable {
                retry.store(wav);
                let _ = app.emit("dictation-retry-available", true);
            }
            report(app, &error, retryable);
        }
    }
}

/// Termina un dictado largo: espera al trabajador y procesa lo que falte.
fn finish_long<R: Runtime>(
    app: &AppHandle<R>,
    session: Option<Arc<LongSession>>,
    rest: Option<Segment>,
    total: Duration,
) {
    let Some(session) = session else {
        hide_overlay(app);
        return;
    };
    session.stop.store(true, Ordering::SeqCst);
    let worker = session.worker.lock().expect("session poisoned").take();
    if let Some(worker) = worker {
        let _ = worker.join();
    }
    let progress = std::mem::take(&mut *session.progress.lock().expect("session poisoned"));
    let mut wavs = progress.pending;
    if let Some(rest) = rest {
        match rest.encode() {
            Ok(wav) => wavs.push(wav),
            Err(e) => tracing::warn!(error = %e, "no se pudo codificar el último trozo"),
        }
    }
    resume_long(
        app,
        LongPending {
            text: progress.text,
            wavs,
            mode: session.mode.clone(),
            total_ms: total.as_millis() as u64,
        },
    );
}

/// Procesa los trozos pendientes de un dictado largo y entrega el texto completo.
/// Si un trozo falla, guarda lo que falta para reintentarlo desde el Historial.
fn resume_long<R: Runtime>(app: &AppHandle<R>, mut pending: LongPending) {
    let p = match pipeline_for(app, pending.mode.clone()) {
        Ok(p) => p,
        Err(e) => return long_failed(app, pending, e),
    };
    while !pending.wavs.is_empty() {
        set_overlay(
            app,
            OverlayState::Processing {
                stage: Stage::Transcribing,
                mode: p.mode.name.clone(),
            },
        );
        let result = longform::process_chunk(
            &p.transcriber,
            p.cleaner(),
            &pending.wavs[0],
            &p.options,
            p.instructions.trim(),
            &p.vocabulary,
            &mut pending.text,
            &std::thread::sleep,
        );
        match result {
            Ok(()) => {
                pending.wavs.remove(0);
            }
            Err(e) => return long_failed(app, pending, e),
        }
    }
    let _ = tray::apply(app, TrayEvent::ProcessingDone);
    let _ = app.emit("dictation-retry-available", false);
    let text = pending.text;
    if text.text().trim().is_empty() {
        hide_overlay(app);
        return;
    }
    let out = PipelineOutput {
        raw: text.raw(),
        text: text.text(),
        cleaned: text.cleaned_any,
        stt_latency_ms: text.stt_ms,
        llm_latency_ms: text.llm_ms,
    };
    match deliver(app, &p, &out, pending.total_ms) {
        Ok(state) => {
            set_overlay(app, state);
            hide_overlay_after(app, PASTED_VISIBLE);
        }
        Err(e) => {
            let _ = tray::apply(app, TrayEvent::Error);
            report(app, &e, false);
        }
    }
}

fn long_failed<R: Runtime>(app: &AppHandle<R>, pending: LongPending, error: AppError) {
    let _ = tray::apply(app, TrayEvent::Error);
    let retryable = error.retryable();
    if retryable {
        app.state::<DictationState>()
            .retry
            .store_pending(Pending::Long(Box::new(pending)));
        let _ = app.emit("dictation-retry-available", true);
    }
    report(app, &error, retryable);
}

/// Devuelve el estado final del overlay, o `None` si no se dijo nada.
fn transcribe_and_deliver<R: Runtime>(
    app: &AppHandle<R>,
    wav: &[u8],
) -> Result<Option<OverlayState>, AppError> {
    let p = pipeline_for(app, app.state::<Modes>().active())?;
    let mode_name = p.mode.name.clone();
    let on_stage = |stage| {
        set_overlay(
            app,
            OverlayState::Processing {
                stage,
                mode: mode_name.clone(),
            },
        )
    };
    let Some(out) = run_pipeline_with_stages(
        &p.transcriber,
        p.cleaner(),
        wav,
        &p.options,
        p.instructions.trim(),
        &p.vocabulary,
        &on_stage,
    )?
    else {
        return Ok(None);
    };
    deliver(app, &p, &out, audio_ms(wav)).map(Some)
}

/// Guarda en el historial, pega (o copia) y avisa del resultado.
fn deliver<R: Runtime>(
    app: &AppHandle<R>,
    p: &Pipeline,
    out: &PipelineOutput,
    duration_ms: u64,
) -> Result<OverlayState, AppError> {
    let mode = &p.mode;
    // El texto dictado nunca debe aparecer en los logs.
    let redactions = app.state::<Redactions>();
    redactions.add(&out.raw);
    redactions.add(&out.text);

    // Se guarda antes de pegar: si el pegado falla, el texto sigue en el historial.
    app.state::<History>().record(&NewEntry {
        created_at: history::now_ms(),
        raw_text: out.raw.clone(),
        final_text: out.text.clone(),
        mode: if out.cleaned {
            mode.id.clone()
        } else {
            "literal".into()
        },
        duration_ms: duration_ms as i64,
        stt_latency_ms: out.stt_latency_ms,
        llm_latency_ms: out.llm_latency_ms,
    });
    let _ = app.emit("transcription", &out.text);
    let clipboard = SystemClipboard::new()?;
    // Si el teclado no se puede abrir (macOS sin Accesibilidad), el texto se
    // queda en el portapapeles en lugar de perderse.
    let keys = EnigoKeys::new();
    let delivery = Paster::new(clipboard, keys, TauriNotifier(app.clone())).deliver(
        &out.text,
        if p.copy_only {
            OutputMode::CopyOnly
        } else {
            OutputMode::Paste
        },
    )?;
    let processing_ms = app
        .state::<DictationState>()
        .stopped_at
        .lock()
        .expect("dictation poisoned")
        .map_or(0, |t| t.elapsed().as_millis() as u64);
    let _ = app.emit(
        "dictation-result",
        DictationResult {
            text: out.text.clone(),
            mode: if out.cleaned {
                mode.name.clone()
            } else {
                "Literal".into()
            },
            audio_ms: duration_ms,
            processing_ms,
        },
    );
    Ok(OverlayState::Pasted {
        words: word_count(&out.text),
        ms: processing_ms,
        copied: delivery == Delivery::InClipboard,
    })
}

/// WAV PCM de 16 bits a 16 kHz: 32 bytes por milisegundo tras la cabecera.
fn audio_ms(wav: &[u8]) -> u64 {
    (wav.len().saturating_sub(44) / 32) as u64
}

#[tauri::command]
pub fn dictation_retry_available(state: State<'_, DictationState>) -> bool {
    state.retry.has_pending()
}

/// Reintenta el último dictado fallido con el audio conservado en memoria.
#[tauri::command]
pub fn dictation_retry(app: AppHandle) -> Result<(), String> {
    let pending = app
        .state::<DictationState>()
        .retry
        .take_pending()
        .ok_or_else(|| "No hay ningún dictado pendiente de reintentar".to_string())?;
    let _ = tray::apply(&app, TrayEvent::StartRecording);
    let _ = tray::apply(&app, TrayEvent::StopRecording);
    *app.state::<DictationState>()
        .stopped_at
        .lock()
        .expect("dictation poisoned") = Some(Instant::now());
    set_overlay(
        &app,
        OverlayState::Processing {
            stage: Stage::Transcribing,
            mode: app.state::<Modes>().active().name,
        },
    );
    std::thread::spawn(move || match pending {
        Pending::Short(wav) => process(&app, wav),
        Pending::Long(long) => resume_long(&app, *long),
    });
    Ok(())
}

/// Botón cancelar del overlay.
#[tauri::command]
pub fn dictation_cancel(app: AppHandle) {
    hotkey::cancel_from_ui(&app);
}

/// Cierra el overlay (p. ej. tras un error que no se quiere reintentar).
#[tauri::command]
pub fn overlay_dismiss(app: AppHandle) {
    hide_overlay(&app);
}

/// El overlay ajusta su ventana a su contenido (tamaño lógico en px) y se recoloca.
#[tauri::command]
pub fn overlay_fit(app: AppHandle, width: f64, height: f64) {
    let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW) else {
        return;
    };
    let (width, height) = (width.clamp(80.0, 800.0), height.clamp(40.0, 240.0));
    let _ = overlay.set_size(LogicalSize::new(width, height));
    position_overlay(&overlay, width, height);
}

fn cancel<R: Runtime>(app: &AppHandle<R>) {
    hide_overlay(app);
    if let Some(session) = take_session(app) {
        session.stop.store(true, Ordering::SeqCst);
    }
    if let Some(recorder) = take_recorder(app) {
        recorder.cancel();
    }
    let _ = tray::apply(app, TrayEvent::Cancel);
}

/// En debug guarda la última grabación para poder escucharla.
fn save_debug_wav<R: Runtime>(app: &AppHandle<R>, wav: &[u8]) {
    if cfg!(debug_assertions)
        && let Ok(dir) = app.path().app_data_dir()
    {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("last-recording.wav"), wav);
    }
}

/// Muestra el overlay con un estado nuevo.
fn set_overlay<R: Runtime>(app: &AppHandle<R>, state: OverlayState) {
    app.state::<DictationState>()
        .overlay_generation
        .fetch_add(1, Ordering::SeqCst);
    let _ = app.emit_to(OVERLAY_WINDOW, "overlay-state", &state);
    let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW) else {
        return;
    };
    if !overlay.is_visible().unwrap_or(false) {
        if let (Ok(size), Ok(scale)) = (overlay.outer_size(), overlay.scale_factor()) {
            let win = size.to_logical::<f64>(scale);
            position_overlay(&overlay, win.width, win.height);
        }
        let _ = overlay.show();
    }
}

/// Centrado abajo en el monitor principal.
fn position_overlay<R: Runtime>(overlay: &tauri::WebviewWindow<R>, width: f64, height: f64) {
    if let Ok(Some(monitor)) = overlay.primary_monitor() {
        let screen = monitor.size().to_logical::<f64>(monitor.scale_factor());
        let _ = overlay.set_position(LogicalPosition::new(
            (screen.width - width) / 2.0,
            screen.height - height - 96.0,
        ));
    }
}

fn hide_overlay<R: Runtime>(app: &AppHandle<R>) {
    app.state::<DictationState>()
        .overlay_generation
        .fetch_add(1, Ordering::SeqCst);
    if let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW) {
        let _ = overlay.hide();
    }
}

/// Oculta el overlay pasado `after`, salvo que haya cambiado entretanto.
fn hide_overlay_after<R: Runtime>(app: &AppHandle<R>, after: Duration) {
    let generation = app
        .state::<DictationState>()
        .overlay_generation
        .load(Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(after);
        if app
            .state::<DictationState>()
            .overlay_generation
            .load(Ordering::SeqCst)
            == generation
        {
            hide_overlay(&app);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{Cleaned, LlmError};
    use crate::stt::{SttError, Transcript};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Falla las primeras `failures` veces y luego transcribe.
    struct FlakyTranscriber {
        failures: usize,
        calls: AtomicUsize,
    }

    impl Transcriber for FlakyTranscriber {
        fn transcribe(&self, wav: &[u8], _: &TranscribeOptions) -> Result<Transcript, SttError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n < self.failures {
                return Err(SttError::Network("tiempo de espera agotado".into()));
            }
            Ok(Transcript {
                text: format!("eh dictado de {} bytes", wav.len()),
                latency: Duration::from_millis(5),
            })
        }
    }

    struct UpperCleaner;
    impl TextCleaner for UpperCleaner {
        fn clean(&self, raw: &str, _: &str) -> Result<Cleaned, LlmError> {
            Ok(Cleaned {
                text: raw.trim_start_matches("eh ").to_uppercase(),
                latency: Duration::ZERO,
                usage: None,
            })
        }
    }

    fn run(
        t: &dyn Transcriber,
        c: Option<&dyn TextCleaner>,
        wav: &[u8],
    ) -> Result<Option<PipelineOutput>, AppError> {
        run_pipeline(
            t,
            c,
            wav,
            &TranscribeOptions::default(),
            "",
            &Vocabulary::default(),
        )
    }

    #[test]
    fn failed_transcription_keeps_audio_and_retry_completes() {
        let transcriber = FlakyTranscriber {
            failures: 1,
            calls: AtomicUsize::new(0),
        };
        let buffer = RetryBuffer::default();
        let wav = vec![7u8; 1000];

        // Primer intento: sin red → error reintentable, el audio queda en memoria.
        let err = run(&transcriber, Some(&UpperCleaner), &wav).unwrap_err();
        assert_eq!(err, AppError::NoNetwork);
        assert!(err.retryable());
        buffer.store(wav.clone());
        assert!(buffer.has_pending());

        // Reintento desde el historial: mismo audio, ahora completa.
        let Some(Pending::Short(retried)) = buffer.take_pending() else {
            panic!("debía quedar el audio del dictado corto");
        };
        assert_eq!(retried, wav);
        let out = run(&transcriber, Some(&UpperCleaner), &retried)
            .unwrap()
            .unwrap();
        assert_eq!(out.text, "DICTADO DE 1000 BYTES");
        assert!(out.cleaned);
        assert!(!buffer.has_pending());
    }

    #[test]
    fn pipeline_without_cleaner_returns_raw_with_replacements() {
        let t = FlakyTranscriber {
            failures: 0,
            calls: AtomicUsize::new(0),
        };
        let vocab = Vocabulary {
            terms: vec![],
            replacements: vec![crate::vocabulary::Replacement {
                from: "dictado".into(),
                to: "Dictado".into(),
            }],
        };
        let out = run_pipeline(
            &t,
            None,
            &[0; 10],
            &TranscribeOptions::default(),
            "",
            &vocab,
        )
        .unwrap()
        .unwrap();
        assert_eq!(out.text, "eh Dictado de 10 bytes");
        assert!(!out.cleaned);
        assert_eq!(out.llm_latency_ms, None);
    }

    #[test]
    fn empty_transcript_is_none() {
        struct Silent;
        impl Transcriber for Silent {
            fn transcribe(&self, _: &[u8], _: &TranscribeOptions) -> Result<Transcript, SttError> {
                Ok(Transcript {
                    text: String::new(),
                    latency: Duration::ZERO,
                })
            }
        }
        assert_eq!(run(&Silent, None, &[]).unwrap(), None);
    }
}
