//! Orquesta un dictado: atajo → audio → transcripción → limpieza → salida.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, LogicalPosition, Manager, Runtime, State};

use crate::audio::{CpalSource, Recorder};
use crate::errors::{AppError, Lang};
use crate::history::{self, History, NewEntry};
use crate::hotkey::{self, DictationAction};
use crate::llm::{AnthropicCleaner, CleanupOutcome, TextCleaner, clean_or_raw};
use crate::logging::Redactions;
use crate::modes::Modes;
use crate::output::{EnigoKeys, Notifier, OutputMode, Paster, SystemClipboard, TauriNotifier};
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
    retry: RetryBuffer,
}

/// Audio del último dictado que falló por un servicio, para reintentarlo.
#[derive(Default)]
pub struct RetryBuffer(Mutex<Option<Vec<u8>>>);

impl RetryBuffer {
    pub fn store(&self, wav: Vec<u8>) {
        *self.0.lock().expect("retry poisoned") = Some(wav);
    }
    pub fn take(&self) -> Option<Vec<u8>> {
        self.0.lock().expect("retry poisoned").take()
    }
    pub fn has_pending(&self) -> bool {
        self.0.lock().expect("retry poisoned").is_some()
    }
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
    let transcript = transcriber.transcribe(wav, options)?;
    if transcript.text.is_empty() {
        return Ok(None);
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
fn report<R: Runtime>(app: &AppHandle<R>, error: &AppError) {
    let (title, body) = error.notification(Lang::system());
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
            *app.state::<DictationState>()
                .recorder
                .lock()
                .expect("dictation poisoned") = Some(recorder);
            let _ = tray::apply(app, TrayEvent::StartRecording);
            let mode = app.state::<Modes>().active().name;
            let _ = app.emit_to(OVERLAY_WINDOW, "mode-changed", mode);
            show_overlay(app);
            sound(app, Sound::Start);
        }
        Err(e) => {
            hotkey::reset(app);
            let _ = tray::apply(app, TrayEvent::Error);
            report(app, &AppError::from(e));
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

fn stop<R: Runtime>(app: &AppHandle<R>) {
    hide_overlay(app);
    let Some(recorder) = take_recorder(app) else {
        return;
    };
    match recorder.stop() {
        Ok(Some(wav)) => {
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
            let _ = tray::apply(app, TrayEvent::Cancel);
        }
        Err(e) => {
            let _ = tray::apply(app, TrayEvent::Error);
            report(app, &AppError::from(e));
        }
    }
}

/// Transcribe y entrega el texto. Corre en su propio hilo.
fn process<R: Runtime>(app: &AppHandle<R>, wav: Vec<u8>) {
    let retry = &app.state::<DictationState>().retry;
    match transcribe_and_deliver(app, &wav) {
        Ok(()) => {
            let _ = tray::apply(app, TrayEvent::ProcessingDone);
            let _ = app.emit("dictation-retry-available", false);
        }
        Err(error) => {
            let _ = tray::apply(app, TrayEvent::Error);
            if error.retryable() {
                retry.store(wav);
                let _ = app.emit("dictation-retry-available", true);
            }
            report(app, &error);
        }
    }
}

fn transcribe_and_deliver<R: Runtime>(app: &AppHandle<R>, wav: &[u8]) -> Result<(), AppError> {
    let keys = app.state::<AppKeys>();
    let groq_key = keys
        .get(Provider::Groq)
        .ok_or(AppError::MissingKey(Provider::Groq))?;
    let vocabulary = app.state::<VocabularyStore>().get();
    let settings = app.state::<SettingsState>().get();
    let mode = app.state::<Modes>().active();
    let cleaner = mode
        .wants_llm(settings.ai.cleanup)
        .then(|| keys.get(Provider::Anthropic))
        .flatten()
        .map(AnthropicCleaner::new);
    let instructions = [mode.instructions(), vocabulary.llm_instructions()].join("\n");

    let Some(out) = run_pipeline(
        &GroqTranscriber::new(groq_key),
        cleaner.as_ref().map(|c| c as &dyn TextCleaner),
        wav,
        &TranscribeOptions {
            model: settings.transcription.model.clone(),
            language: settings.transcription.language.clone(),
            prompt: vocabulary.whisper_prompt(),
        },
        instructions.trim(),
        &vocabulary,
    )?
    else {
        return Ok(());
    };

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
        // WAV PCM de 16 bits a 16 kHz: 32 bytes por milisegundo tras la cabecera.
        duration_ms: (wav.len().saturating_sub(44) / 32) as i64,
        stt_latency_ms: out.stt_latency_ms,
        llm_latency_ms: out.llm_latency_ms,
    });
    let _ = app.emit("transcription", &out.text);
    let clipboard = SystemClipboard::new()?;
    let keys = EnigoKeys::new()?;
    Paster::new(clipboard, keys, TauriNotifier(app.clone())).deliver(
        &out.text,
        if settings.output.copy_only {
            OutputMode::CopyOnly
        } else {
            OutputMode::Paste
        },
    )?;
    Ok(())
}

#[tauri::command]
pub fn dictation_retry_available(state: State<'_, DictationState>) -> bool {
    state.retry.has_pending()
}

/// Reintenta el último dictado fallido con el audio conservado en memoria.
#[tauri::command]
pub fn dictation_retry(app: AppHandle) -> Result<(), String> {
    let wav = app
        .state::<DictationState>()
        .retry
        .take()
        .ok_or_else(|| "No hay ningún dictado pendiente de reintentar".to_string())?;
    let _ = tray::apply(&app, TrayEvent::StartRecording);
    let _ = tray::apply(&app, TrayEvent::StopRecording);
    std::thread::spawn(move || process(&app, wav));
    Ok(())
}

fn cancel<R: Runtime>(app: &AppHandle<R>) {
    hide_overlay(app);
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

fn show_overlay<R: Runtime>(app: &AppHandle<R>) {
    let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW) else {
        return;
    };
    // Centrado abajo en el monitor principal.
    if let (Ok(Some(monitor)), Ok(size)) = (overlay.primary_monitor(), overlay.outer_size()) {
        let scale = monitor.scale_factor();
        let screen = monitor.size().to_logical::<f64>(scale);
        let win = size.to_logical::<f64>(scale);
        let _ = overlay.set_position(LogicalPosition::new(
            (screen.width - win.width) / 2.0,
            screen.height - win.height - 96.0,
        ));
    }
    let _ = overlay.show();
}

fn hide_overlay<R: Runtime>(app: &AppHandle<R>) {
    if let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW) {
        let _ = overlay.hide();
    }
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
        let retried = buffer.take().unwrap();
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
