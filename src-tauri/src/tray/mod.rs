//! Icono y menú de la bandeja del sistema (HU-02).

mod state;

pub use state::{InvalidTransition, TrayEvent, TrayState, icon_rgba};

use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub const TRAY_ID: &str = "main";
pub const SETTINGS_WINDOW: &str = "settings";
const ICON_SIZE: u32 = 32;

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

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Ajustes", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", "Historial", true, None::<&str>)?;
    // Los modos llegan en HU-09; por ahora solo existe "Limpio".
    let mode_clean =
        CheckMenuItem::with_id(app, "mode:clean", "Limpio", false, true, None::<&str>)?;
    let modes = Submenu::with_items(app, "Modo", true, &[&mode_clean])?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &history, &modes, &separator, &quit])?;

    let state = TrayState::default();
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(state))
        .tooltip(state.tooltip())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => show_window(app, View::Settings),
            "history" => show_window(app, View::History),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
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
    fn view_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&View::History).unwrap(),
            "\"history\""
        );
    }
}
