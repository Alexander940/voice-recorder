//! Implementaciones reales de los traits de salida.

use std::sync::atomic::{AtomicBool, Ordering};

use enigo::{Direction, Enigo, Key, Keyboard, NewConError, Settings};
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

use super::{Clipboard, KeySender, Notifier, OutputError, PasteModifier, paste_modifier};

fn err(e: impl std::fmt::Display) -> OutputError {
    OutputError(e.to_string())
}

pub struct SystemClipboard(arboard::Clipboard);

impl SystemClipboard {
    pub fn new() -> Result<Self, OutputError> {
        #[cfg(target_os = "linux")]
        keepalive::ensure();
        arboard::Clipboard::new().map(Self).map_err(err)
    }
}

/// En X11 el contenido del portapapeles solo existe mientras viva un
/// `arboard::Clipboard` del proceso que lo puso; al soltar el último se pierde.
/// GNOME en Wayland no ofrece `data-control`, así que `arboard` usa X11 vía
/// XWayland y pasa lo mismo. Se conserva un `Clipboard` toda la vida del
/// proceso para que el texto siga ahí tras pegar, copiar o restaurar (#38).
#[cfg(target_os = "linux")]
mod keepalive {
    use std::sync::{Mutex, OnceLock};

    static HANDLE: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();

    pub fn ensure() {
        HANDLE.get_or_init(|| Mutex::new(arboard::Clipboard::new().ok()));
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

/// Lo que se le dice al usuario cuando macOS no deja simular el pegado.
pub const ACCESSIBILITY_HINT: &str = "Activa Voice Recorder en Ajustes del Sistema → Privacidad y seguridad → Accesibilidad y vuelve a abrir la app";

/// El diálogo de permisos de macOS se abre como mucho una vez por sesión,
/// no en cada dictado.
static PERMISSION_PROMPTED: AtomicBool = AtomicBool::new(false);

impl EnigoKeys {
    pub fn new() -> Result<Self, OutputError> {
        let settings = Settings {
            open_prompt_to_get_permissions: !PERMISSION_PROMPTED.swap(true, Ordering::SeqCst),
            ..Settings::default()
        };
        Enigo::new(&settings).map(Self).map_err(|e| match e {
            NewConError::NoPermission => OutputError(ACCESSIBILITY_HINT.into()),
            other => err(other),
        })
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

/// Teclado con el que pegar en esta sesión: el portal `RemoteDesktop` en Linux
/// Wayland (issue #38) y `enigo` en el resto. El `Err` se entrega al
/// [`super::Paster`], que deja el texto en el portapapeles y avisa.
pub fn platform_keys<R: Runtime>(app: &AppHandle<R>) -> Result<Box<dyn KeySender>, OutputError> {
    #[cfg(target_os = "linux")]
    if crate::platform::uses_portals() {
        use tauri::Manager;
        let dir = app.path().app_data_dir().map_err(err)?;
        let store = super::portal::FileTokenStore(dir.join("remote-desktop.token"));
        return Ok(Box::new(super::portal::PortalKeys::new(store)));
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app;
    Ok(Box::new(EnigoKeys::new()?))
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

#[cfg(all(test, any(windows, target_os = "linux")))]
mod tests {
    use super::*;

    /// Ida y vuelta con el portapapeles real. Se ejecuta a mano:
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

    /// El texto sigue en el portapapeles cuando se suelta el `SystemClipboard`
    /// que lo puso (en X11 se perdía). A mano, en una sesión gráfica:
    /// `cargo test -- --ignored clipboard_survives_drop`
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "toca el portapapeles real del usuario"]
    fn clipboard_survives_drop() {
        let before = SystemClipboard::new().unwrap().get_text();
        {
            let mut cb = SystemClipboard::new().unwrap();
            cb.set_text("voice-recorder-drop ✓").unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut cb = SystemClipboard::new().unwrap();
        assert_eq!(cb.get_text().as_deref(), Some("voice-recorder-drop ✓"));
        match before {
            Some(t) => cb.set_text(&t).unwrap(),
            None => cb.clear().unwrap(),
        }
    }
}
