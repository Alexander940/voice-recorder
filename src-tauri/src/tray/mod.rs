//! Icono y menú de la bandeja del sistema (HU-02).

mod state;

pub use state::{InvalidTransition, TrayEvent, TrayState, icon_rgba};

use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::modes::{Modes, ModesState};

pub const TRAY_ID: &str = "main";
pub const SETTINGS_WINDOW: &str = "settings";
const ICON_SIZE: u32 = 32;
const MODE_PREFIX: &str = "mode:";

/// Estado compartido de la bandeja, gestionado por Tauri.
#[derive(Default)]
pub struct TrayStore(pub Mutex<TrayState>);

/// Vista del frontend a la que se navega al abrir la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    Settings,
    History,
    Onboarding,
}

/// Qué hacer cuando el usuario cierra una ventana: ocultarla, nunca salir.
#[derive(Debug, PartialEq, Eq)]
pub enum CloseAction {
    Hide,
}

pub fn close_action(_label: &str) -> CloseAction {
    CloseAction::Hide
}

fn icon(state: TrayState) -> Image<'static> {
    Image::new_owned(icon_rgba(state, ICON_SIZE), ICON_SIZE, ICON_SIZE)
}

fn menu<R: Runtime>(app: &AppHandle<R>, modes: &ModesState) -> tauri::Result<Menu<R>> {
    let settings = MenuItem::with_id(app, "settings", "Ajustes", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", "Historial", true, None::<&str>)?;
    let mode_items = modes
        .modes
        .iter()
        .map(|m| {
            CheckMenuItem::with_id(
                app,
                format!("{MODE_PREFIX}{}", m.id),
                &m.name,
                true,
                m.id == modes.active,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = mode_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<R>)
        .collect();
    let mode_menu = Submenu::with_items(app, "Modo", true, &refs)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    Menu::with_items(app, &[&settings, &history, &mode_menu, &separator, &quit])
}

/// Id de menú → id de modo (`mode:email` → `email`).
pub fn mode_from_menu_id(id: &str) -> Option<&str> {
    id.strip_prefix(MODE_PREFIX)
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let modes = app.state::<Modes>().snapshot();
    let state = TrayState::default();
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(state))
        .tooltip(state.tooltip())
        .menu(&menu(app, &modes)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => show_window(app, View::Settings),
            "history" => show_window(app, View::History),
            "quit" => app.exit(0),
            id => {
                if let Some(mode) = mode_from_menu_id(id) {
                    let mode = mode.to_string();
                    let r = app.state::<Modes>().update(|s| s.set_active(&mode));
                    if r.is_ok() {
                        crate::modes::notify_changed(app);
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Reconstruye el menú cuando cambian los modos o el activo.
pub fn rebuild_menu<R: Runtime>(app: &AppHandle<R>, modes: &ModesState) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu(app, modes)?))?;
    }
    Ok(())
}

pub fn show_window<R: Runtime>(app: &AppHandle<R>, view: View) {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        let _ = window.emit("navigate", view);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Aplica un evento del ciclo de dictado y actualiza icono y tooltip.
pub fn apply<R: Runtime>(
    app: &AppHandle<R>,
    event: TrayEvent,
) -> Result<TrayState, InvalidTransition> {
    let store = app.state::<TrayStore>();
    let mut current = store.0.lock().expect("tray state poisoned");
    let next = current.next(event)?;
    *current = next;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon(next)));
        let _ = tray.set_tooltip(Some(next.tooltip()));
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_any_window_hides_it() {
        assert_eq!(close_action(SETTINGS_WINDOW), CloseAction::Hide);
    }

    #[test]
    fn no_window_is_visible_at_startup() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        let windows = conf["app"]["windows"].as_array().unwrap();
        assert!(!windows.is_empty());
        for w in windows {
            assert_eq!(
                w["visible"], false,
                "ventana {} visible al iniciar",
                w["label"]
            );
        }
        assert!(windows.iter().any(|w| w["label"] == SETTINGS_WINDOW));
    }

    #[test]
    fn mode_menu_ids_roundtrip() {
        assert_eq!(mode_from_menu_id("mode:email"), Some("email"));
        assert_eq!(mode_from_menu_id("settings"), None);
    }

    #[test]
    fn view_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&View::History).unwrap(),
            "\"history\""
        );
    }
}
