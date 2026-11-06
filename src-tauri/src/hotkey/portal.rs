//! Atajos globales en Linux Wayland mediante el portal XDG `GlobalShortcuts`
//! (issue #38).
//!
//! En Wayland el compositor no entrega las teclas a `XGrabKey`, así que
//! `tauri-plugin-global-shortcut` registra sin error pero nunca recibe nada.
//! El portal funciona al revés que el plugin: la app pide vincular un conjunto
//! de atajos en una sesión (la primera vez GNOME lo confirma con un diálogo) y
//! después recibe las señales `Activated` / `Deactivated`.
//!
//! Diferencias con el plugin:
//! - Los tres atajos (dictado, cancelar, modo) se vinculan juntos. Cambiar uno
//!   cierra la sesión y abre otra con el conjunto nuevo.
//! - `Esc` no se puede registrar solo durante la grabación, así que cancelar
//!   tiene un atajo fijo ([`WAYLAND_CANCEL_SHORTCUT`]).
//! - Vincular es asíncrono: `register` valida y devuelve `Ok`; si el portal
//!   falla después, se emite `hotkey-error` y se registra en el log.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::oneshot;

use super::{
    ESCAPE, HotkeyError, ShortcutKind, ShortcutRegistrar, WAYLAND_CANCEL_SHORTCUT, mode_shortcut,
};

pub const ID_MAIN: &str = "dictate";
pub const ID_CANCEL: &str = "cancel";
pub const ID_MODE: &str = "next-mode";

/// Los registros del arranque llegan en ráfaga (modo y después el principal):
/// se agrupan para que GNOME pida confirmación una sola vez.
const BIND_DEBOUNCE: Duration = Duration::from_millis(250);

/// Convierte un atajo de la app ("Ctrl+Shift+Space") al formato de la
/// especificación XDG de atajos que entiende el portal ("CTRL+SHIFT+space").
pub fn portal_trigger(shortcut: &str) -> Result<String, HotkeyError> {
    let invalid = || HotkeyError::Invalid(shortcut.into());
    let mut mods: Vec<&str> = Vec::new();
    let mut key: Option<&str> = None;
    for part in shortcut.split('+') {
        let part = part.trim();
        if part.is_empty() {
            return Err(invalid());
        }
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "commandorcontrol" => mods.push("CTRL"),
            "alt" | "option" => mods.push("ALT"),
            "shift" => mods.push("SHIFT"),
            "super" | "meta" | "cmd" | "command" | "logo" => mods.push("LOGO"),
            _ => {
                if key.replace(part).is_some() {
                    return Err(invalid());
                }
            }
        }
    }
    let key = portal_key(key.ok_or_else(invalid)?).ok_or_else(invalid)?;
    mods.push(&key);
    Ok(mods.join("+"))
}

/// Nombre keysym de la tecla, como lo escribe la especificación.
fn portal_key(key: &str) -> Option<String> {
    let lower = key.to_ascii_lowercase();
    let named = match lower.as_str() {
        "space" => "space",
        "escape" | "esc" => "Escape",
        "enter" | "return" => "Return",
        "tab" => "Tab",
        "backspace" => "BackSpace",
        "delete" => "Delete",
        "insert" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" => "Page_Up",
        "pagedown" => "Page_Down",
        "up" | "arrowup" => "Up",
        "down" | "arrowdown" => "Down",
        "left" | "arrowleft" => "Left",
        "right" | "arrowright" => "Right",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.into());
    }
    if key.len() == 1 && key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Some(lower);
    }
    // F1..F24
    let fkey = lower
        .strip_prefix('f')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=24).contains(&n));
    fkey.then(|| lower.to_ascii_uppercase())
}

/// Atajo de la app al que corresponde un id del portal.
pub fn kind_for(id: &str) -> Option<ShortcutKind> {
    match id {
        ID_MAIN => Some(ShortcutKind::Main),
        ID_CANCEL => Some(ShortcutKind::Cancel),
        ID_MODE => Some(ShortcutKind::Mode),
        _ => None,
    }
}

/// Mensaje legible para un fallo del portal.
pub fn bind_error(e: &ashpd::Error) -> HotkeyError {
    use ashpd::desktop::ResponseError;
    match e {
        ashpd::Error::Response(ResponseError::Cancelled) => HotkeyError::Other(
            "Se canceló el diálogo de atajos de GNOME: el atajo de dictado no está activo. Cambia el atajo en Ajustes para volver a pedirlo".into(),
        ),
        ashpd::Error::PortalNotFound(_) | ashpd::Error::RequiresVersion(..) => HotkeyError::Other(
            "El portal de atajos globales (GlobalShortcuts) no está disponible en este escritorio".into(),
        ),
        other => HotkeyError::Other(format!("El portal de atajos globales falló: {other}")),
    }
}

/// Lo que hay que vincular: id, descripción y atajo preferido (formato de la app).
fn wanted(main: Option<&str>, mode: bool) -> Vec<(&'static str, &'static str, String)> {
    let mut list = Vec::new();
    if let Some(main) = main {
        list.push((ID_MAIN, "Dictar (mantener o alternar)", main.to_string()));
    }
    list.push((
        ID_CANCEL,
        "Cancelar el dictado",
        WAYLAND_CANCEL_SHORTCUT.to_string(),
    ));
    if mode {
        list.push((ID_MODE, "Siguiente modo", mode_shortcut().to_string()));
    }
    list
}

#[derive(Default)]
struct Shared {
    main: Option<String>,
    mode: bool,
    generation: u64,
    /// Cierra la sesión del portal en curso.
    stop: Option<oneshot::Sender<()>>,
}

/// Implementa [`ShortcutRegistrar`] sobre el portal.
pub struct PortalRegistrar<R: Runtime> {
    app: AppHandle<R>,
    shared: Arc<Mutex<Shared>>,
}

impl<R: Runtime> PortalRegistrar<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            shared: Arc::new(Mutex::new(Shared::default())),
        }
    }

    /// Programa una vinculación con el conjunto actual, agrupando los cambios
    /// que lleguen en los próximos milisegundos.
    fn schedule(&self) {
        let generation = {
            let mut s = self.shared.lock().expect("portal poisoned");
            s.generation += 1;
            s.generation
        };
        let app = self.app.clone();
        let shared = self.shared.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(BIND_DEBOUNCE).await;
            let (list, stop) = {
                let mut s = shared.lock().expect("portal poisoned");
                if s.generation != generation {
                    return; // llegó otro cambio: lo hará él
                }
                if let Some(old) = s.stop.take() {
                    let _ = old.send(());
                }
                let (tx, rx) = oneshot::channel();
                s.stop = Some(tx);
                (wanted(s.main.as_deref(), s.mode), rx)
            };
            if let Err(e) = bind_and_listen(&app, list, stop).await {
                let msg = bind_error(&e).to_string();
                tracing::warn!("hotkey: backend=portal status=error reason={msg}");
                let _ = app.emit("hotkey-error", msg);
            }
        });
    }
}

impl<R: Runtime> ShortcutRegistrar for PortalRegistrar<R> {
    fn register(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        if shortcut == ESCAPE {
            return Ok(()); // cancelar está siempre vinculado
        }
        portal_trigger(shortcut)?;
        {
            let mut s = self.shared.lock().expect("portal poisoned");
            if shortcut == mode_shortcut() {
                s.mode = true;
            } else {
                s.main = Some(shortcut.to_string());
            }
        }
        self.schedule();
        Ok(())
    }

    fn unregister(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        if shortcut == ESCAPE {
            return Ok(());
        }
        let changed = {
            let mut s = self.shared.lock().expect("portal poisoned");
            if shortcut == mode_shortcut() {
                std::mem::replace(&mut s.mode, false)
            } else if s.main.as_deref() == Some(shortcut) {
                s.main = None;
                true
            } else {
                false
            }
        };
        if changed {
            self.schedule();
        }
        Ok(())
    }
}

/// Vincula los atajos y reenvía las señales al controlador hasta que `stop`
/// se dispare; entonces cierra la sesión del portal.
async fn bind_and_listen<R: Runtime>(
    app: &AppHandle<R>,
    list: Vec<(&'static str, &'static str, String)>,
    mut stop: oneshot::Receiver<()>,
) -> ashpd::Result<()> {
    let proxy = GlobalShortcuts::new().await?;
    let session = proxy.create_session().await?;
    let mut shortcuts = Vec::with_capacity(list.len());
    for (id, description, trigger) in &list {
        // Ya se validó en `register`; un atajo raro se vincula sin preferencia.
        let preferred = portal_trigger(trigger).ok();
        tracing::info!("hotkey: backend=portal status=requested id={id} trigger={trigger}");
        shortcuts.push(NewShortcut::new(*id, *description).preferred_trigger(preferred.as_deref()));
    }
    let request = tokio::select! {
        r = proxy.bind_shortcuts(&session, &shortcuts, None) => r?,
        _ = &mut stop => {
            let _ = session.close().await;
            return Ok(());
        }
    };
    let bound = request.response()?;
    for s in bound.shortcuts() {
        tracing::info!(
            "hotkey: backend=portal status=bound id={} trigger={}",
            s.id(),
            s.trigger_description()
        );
    }
    let mut activated = proxy.receive_activated().await?;
    let mut deactivated = proxy.receive_deactivated().await?;
    loop {
        tokio::select! {
            Some(a) = activated.next() => {
                if let Some(kind) = kind_for(a.shortcut_id()) {
                    super::handle(app, kind, true);
                }
            }
            Some(d) = deactivated.next() => {
                if let Some(kind) = kind_for(d.shortcut_id()) {
                    super::handle(app, kind, false);
                }
            }
            _ = &mut stop => break,
            else => break,
        }
    }
    let _ = session.close().await;
    tracing::info!("hotkey: backend=portal status=closed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ashpd::desktop::ResponseError;

    #[test]
    fn triggers_follow_the_xdg_shortcuts_format() {
        assert_eq!(
            portal_trigger("Ctrl+Shift+Space").unwrap(),
            "CTRL+SHIFT+space"
        );
        assert_eq!(
            portal_trigger("Ctrl+Shift+Escape").unwrap(),
            "CTRL+SHIFT+Escape"
        );
        assert_eq!(portal_trigger("Ctrl+Shift+M").unwrap(), "CTRL+SHIFT+m");
        assert_eq!(portal_trigger("Ctrl+Alt+Space").unwrap(), "CTRL+ALT+space");
        assert_eq!(portal_trigger("Super+F9").unwrap(), "LOGO+F9");
        assert_eq!(portal_trigger("Option+Space").unwrap(), "ALT+space");
        assert_eq!(portal_trigger("Ctrl+Alt+ArrowUp").unwrap(), "CTRL+ALT+Up");
        assert_eq!(portal_trigger("Ctrl+1").unwrap(), "CTRL+1");
    }

    #[test]
    fn invalid_triggers_are_rejected() {
        for s in [
            "",
            "Ctrl+",
            "Ctrl+Shift",
            "Ctrl+NoExiste",
            "Ctrl+A+B",
            "F25",
        ] {
            assert_eq!(
                portal_trigger(s),
                Err(HotkeyError::Invalid(s.into())),
                "{s:?} debería ser inválido"
            );
        }
    }

    #[test]
    fn portal_ids_map_to_shortcut_kinds() {
        assert_eq!(kind_for(ID_MAIN), Some(ShortcutKind::Main));
        assert_eq!(kind_for(ID_CANCEL), Some(ShortcutKind::Cancel));
        assert_eq!(kind_for(ID_MODE), Some(ShortcutKind::Mode));
        assert_eq!(kind_for("otro"), None);
    }

    #[test]
    fn cancel_is_always_bound_and_main_is_optional() {
        fn ids(l: Vec<(&'static str, &'static str, String)>) -> Vec<&'static str> {
            l.into_iter().map(|(id, ..)| id).collect()
        }
        assert_eq!(ids(wanted(None, false)), [ID_CANCEL]);
        assert_eq!(ids(wanted(None, true)), [ID_CANCEL, ID_MODE]);
        let full = wanted(Some("Ctrl+Shift+Space"), true);
        assert_eq!(ids(full.clone()), [ID_MAIN, ID_CANCEL, ID_MODE]);
        assert_eq!(full[1].2, WAYLAND_CANCEL_SHORTCUT);
    }

    #[test]
    fn portal_errors_are_readable() {
        let cancelled = bind_error(&ashpd::Error::Response(ResponseError::Cancelled));
        assert!(matches!(cancelled, HotkeyError::Other(_)));
        assert!(cancelled.to_string().contains("GNOME"));

        let name = ashpd::zbus::names::OwnedInterfaceName::try_from(
            "org.freedesktop.portal.GlobalShortcuts",
        )
        .unwrap();
        let missing = bind_error(&ashpd::Error::PortalNotFound(name));
        assert!(missing.to_string().contains("no está disponible"));

        let other = bind_error(&ashpd::Error::NoResponse);
        assert!(other.to_string().contains("falló"));
    }
}
