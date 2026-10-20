//! Orquesta un dictado: atajo → audio → transcripción → salida.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, LogicalPosition, Manager, Runtime};

use crate::audio::{CpalSource, DEFAULT_MAX_DURATION, Recorder};
use crate::hotkey::{self, DictationAction};
use crate::output::{EnigoKeys, Notifier, OutputMode, Paster, SystemClipboard, TauriNotifier};
use crate::settings::AppKeys;
use crate::settings::keys::Provider;
use crate::stt::{GroqTranscriber, TranscribeOptions, Transcriber};
use crate::tray::{self, TrayEvent};

pub const OVERLAY_WINDOW: &str = "overlay";

#[derive(Default)]
pub struct DictationState {
    recorder: Mutex<Option<Recorder<CpalSource>>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordingFinished {
    pub bytes: usize,
}

pub fn on_action<R: Runtime>(app: &AppHandle<R>, action: DictationAction) {
    match action {
        DictationAction::Start => start(app),
        DictationAction::Stop => stop(app),
        DictationAction::Cancel => cancel(app),
    }
}

fn start<R: Runtime>(app: &AppHandle<R>) {
    let level_app = app.clone();
    let limit_app = app.clone();
    let result = CpalSource::new(None).and_then(|source| {
        Recorder::start(
            source,
            DEFAULT_MAX_DURATION,
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
            show_overlay(app);
        }
        Err(e) => {
            hotkey::reset(app);
            let _ = tray::apply(app, TrayEvent::Error);
            let _ = app.emit("dictation-error", e.to_string());
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
            let _ = app.emit("dictation-error", e.to_string());
        }
    }
}

/// Transcribe y entrega el texto. Corre en su propio hilo.
fn process<R: Runtime>(app: &AppHandle<R>, wav: Vec<u8>) {
    match transcribe_and_deliver(app, &wav) {
        Ok(()) => {
            let _ = tray::apply(app, TrayEvent::ProcessingDone);
        }
        Err(message) => {
            let _ = tray::apply(app, TrayEvent::Error);
            TauriNotifier(app.clone()).notify("Error de dictado", &message);
            let _ = app.emit("dictation-error", message);
        }
    }
}

fn transcribe_and_deliver<R: Runtime>(app: &AppHandle<R>, wav: &[u8]) -> Result<(), String> {
    let key = app
        .state::<AppKeys>()
        .get(Provider::Groq)
        .ok_or_else(|| "Falta la API key de Groq: configúrala en Ajustes".to_string())?;
    let transcript = GroqTranscriber::new(key)
        .transcribe(wav, &TranscribeOptions::default())
        .map_err(|e| e.to_string())?;
    if transcript.text.is_empty() {
        return Ok(());
    }
    let _ = app.emit("transcription", &transcript.text);
    let clipboard = SystemClipboard::new().map_err(|e| e.to_string())?;
    let keys = EnigoKeys::new().map_err(|e| e.to_string())?;
    Paster::new(clipboard, keys, TauriNotifier(app.clone()))
        .deliver(&transcript.text, OutputMode::Paste)
        .map(|_| ())
        .map_err(|e| e.to_string())
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
