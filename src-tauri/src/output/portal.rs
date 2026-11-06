//! Pegado en Linux Wayland mediante el portal XDG `RemoteDesktop` (issue #38).
//!
//! `enigo` simula teclas por X11 y en Wayland no llega a las ventanas nativas.
//! El portal sí: se pide el dispositivo teclado, GNOME lo confirma con un
//! diálogo la primera vez y devuelve un token con el que recordar el permiso.
//! Si el usuario lo deniega, `paste` falla y el texto se queda en el
//! portapapeles ([`super::Paster`] ya lo gestiona y avisa).

use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use ashpd::desktop::PersistMode;
use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop};

use super::{KeySender, OutputError};

/// Lo que se le dice al usuario cuando GNOME no concede el teclado.
pub const PORTAL_HINT: &str =
    "Permite a Voice Recorder controlar el teclado en el diálogo de GNOME y vuelve a dictar";

// Códigos evdev (linux/input-event-codes.h), que es lo que acepta
// `NotifyKeyboardKeycode`.
const KEY_LEFTCTRL: i32 = 29;
const KEY_V: i32 = 47;
/// Entre pulsaciones, para que la app destino las procese en orden.
const KEY_GAP: Duration = Duration::from_millis(15);
/// Antes de cerrar la sesión, para que el pegado llegue.
const SETTLE: Duration = Duration::from_millis(80);

/// Dónde se guarda el token con el que GNOME recuerda el permiso.
pub trait TokenStore {
    fn load(&self) -> Option<String>;
    fn save(&self, token: &str);
}

/// Token en un archivo del directorio de datos de la app.
pub struct FileTokenStore(pub PathBuf);

impl TokenStore for FileTokenStore {
    fn load(&self) -> Option<String> {
        let token = fs::read_to_string(&self.0).ok()?;
        let token = token.trim();
        (!token.is_empty()).then(|| token.to_string())
    }

    fn save(&self, token: &str) {
        if let Some(dir) = self.0.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Err(e) = fs::write(&self.0, token) {
            tracing::warn!("paste: no se pudo guardar el token del portal: {e}");
        }
    }
}

#[derive(Debug)]
enum PasteError {
    Portal(ashpd::Error),
    /// La sesión arrancó pero sin teclado.
    NoKeyboard,
}

impl From<ashpd::Error> for PasteError {
    fn from(e: ashpd::Error) -> Self {
        PasteError::Portal(e)
    }
}

/// Mensaje legible para el usuario.
fn describe(e: &PasteError) -> String {
    use ashpd::desktop::ResponseError;
    match e {
        PasteError::Portal(ashpd::Error::Response(ResponseError::Cancelled)) => PORTAL_HINT.into(),
        PasteError::Portal(ashpd::Error::PortalNotFound(_))
        | PasteError::Portal(ashpd::Error::RequiresVersion(..)) => {
            "El portal RemoteDesktop no está disponible en este escritorio".into()
        }
        PasteError::Portal(other) => format!("El portal RemoteDesktop falló: {other}"),
        PasteError::NoKeyboard => "GNOME no concedió el control del teclado".into(),
    }
}

/// Teclado virtual sobre el portal. Construirlo nunca falla: el permiso se
/// pide (o se recupera con el token) en el primer `paste`.
pub struct PortalKeys {
    token: Option<String>,
    store: Box<dyn TokenStore + Send>,
}

impl PortalKeys {
    pub fn new(store: impl TokenStore + Send + 'static) -> Self {
        Self {
            token: store.load(),
            store: Box::new(store),
        }
    }
}

impl KeySender for PortalKeys {
    fn paste(&mut self) -> Result<(), OutputError> {
        let token = self.token.clone();
        match run_blocking(paste_via_portal(token)) {
            Ok(new_token) => {
                if let Some(t) = new_token
                    && self.token.as_deref() != Some(t.as_str())
                {
                    self.store.save(&t);
                    self.token = Some(t);
                }
                tracing::info!("paste: backend=portal status=ok");
                Ok(())
            }
            Err(e) => {
                let msg = describe(&e);
                tracing::warn!("paste: backend=portal status=error reason={msg}");
                Err(OutputError(msg))
            }
        }
    }
}

/// Ejecuta el futuro en un hilo propio: `paste` se llama desde hilos normales
/// y, si alguna vez se llamara desde el runtime, `block_on` no debe anidarse.
fn run_blocking<F>(f: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    std::thread::spawn(move || tauri::async_runtime::block_on(f))
        .join()
        .expect("el hilo del portal no debería fallar")
}

/// Pide el teclado, pulsa Ctrl+V y devuelve el token para la próxima vez.
async fn paste_via_portal(restore_token: Option<String>) -> Result<Option<String>, PasteError> {
    let proxy = RemoteDesktop::new().await?;
    let session = proxy.create_session().await?;
    proxy
        .select_devices(
            &session,
            DeviceType::Keyboard.into(),
            restore_token.as_deref(),
            PersistMode::ExplicitlyRevoked,
        )
        .await?
        .response()?;
    let devices = proxy.start(&session, None).await?.response()?;
    if !devices.devices().contains(DeviceType::Keyboard) {
        let _ = session.close().await;
        return Err(PasteError::NoKeyboard);
    }
    let token = devices.restore_token().map(str::to_owned);
    let sequence = [
        (KEY_LEFTCTRL, KeyState::Pressed),
        (KEY_V, KeyState::Pressed),
        (KEY_V, KeyState::Released),
        (KEY_LEFTCTRL, KeyState::Released),
    ];
    let mut result = Ok(());
    for (code, state) in sequence {
        if result.is_ok() {
            result = proxy.notify_keyboard_keycode(&session, code, state).await;
        } else if state == KeyState::Released {
            // Soltar siempre lo que se pulsó, aunque falle la V.
            let _ = proxy.notify_keyboard_keycode(&session, code, state).await;
        }
        tokio::time::sleep(KEY_GAP).await;
    }
    tokio::time::sleep(SETTLE).await;
    let _ = session.close().await;
    result?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ashpd::desktop::ResponseError;

    #[test]
    fn file_token_store_roundtrip_and_empty_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileTokenStore(dir.path().join("sub").join("remote-desktop.token"));
        assert_eq!(store.load(), None);
        store.save("abc-123");
        assert_eq!(store.load().as_deref(), Some("abc-123"));
        fs::write(&store.0, "  \n").unwrap();
        assert_eq!(store.load(), None);
    }

    #[test]
    fn errors_are_readable() {
        let denied = describe(&PasteError::Portal(ashpd::Error::Response(
            ResponseError::Cancelled,
        )));
        assert_eq!(denied, PORTAL_HINT);
        assert!(describe(&PasteError::NoKeyboard).contains("teclado"));
        assert!(describe(&PasteError::Portal(ashpd::Error::NoResponse)).contains("falló"));
    }

    #[test]
    fn paste_sequence_uses_ctrl_v_evdev_codes() {
        // Si alguien cambia los códigos, que se note: 29 = KEY_LEFTCTRL, 47 = KEY_V.
        assert_eq!(KEY_LEFTCTRL, 29);
        assert_eq!(KEY_V, 47);
    }
}
