//! Implementaciones reales de los traits de salida.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

use super::{Clipboard, KeySender, Notifier, OutputError, PasteModifier, paste_modifier};

fn err(e: impl std::fmt::Display) -> OutputError {
    OutputError(e.to_string())
}

pub struct SystemClipboard(arboard::Clipboard);

impl SystemClipboard {
    pub fn new() -> Result<Self, OutputError> {
        arboard::Clipboard::new().map(Self).map_err(err)
    }
}

impl Clipboard for SystemClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.0.get_text().ok()
    }
    fn set_text(&mut self, text: &str) -> Result<(), OutputError> {
        self.0.set_text(text).map_err(err)
    }
    fn clear(&mut self) -> Result<(), OutputError> {
        self.0.clear().map_err(err)
    }
}

pub struct EnigoKeys(Enigo);

impl EnigoKeys {
    pub fn new() -> Result<Self, OutputError> {
        Enigo::new(&Settings::default()).map(Self).map_err(err)
    }
}

impl KeySender for EnigoKeys {
    fn paste(&mut self) -> Result<(), OutputError> {
        let modifier = match paste_modifier() {
            PasteModifier::Control => Key::Control,
            PasteModifier::Command => Key::Meta,
        };
        self.0.key(modifier, Direction::Press).map_err(err)?;
        let result = self.0.key(Key::Unicode('v'), Direction::Click).map_err(err);
        // Soltar el modificador siempre, aunque falle la V.
        self.0.key(modifier, Direction::Release).map_err(err)?;
        result
    }
}

pub struct TauriNotifier<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> Notifier for TauriNotifier<R> {
    fn notify(&self, title: &str, body: &str) {
        let _ = self
            .0
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Ida y vuelta con el portapapeles real de Windows. Se ejecuta a mano:
    /// `cargo test -- --ignored clipboard_roundtrip`
    #[test]
    #[ignore = "toca el portapapeles real del usuario"]
    fn clipboard_roundtrip() {
        let mut cb = SystemClipboard::new().unwrap();
        let before = cb.get_text();
        cb.set_text("voice-recorder-test ✓").unwrap();
        assert_eq!(cb.get_text().as_deref(), Some("voice-recorder-test ✓"));
        match before {
            Some(t) => cb.set_text(&t).unwrap(),
            None => cb.clear().unwrap(),
        }
    }
}
