//! Icono y menú de la bandeja del sistema (HU-02).

mod state;

pub use state::{InvalidTransition, TrayEvent, TrayState, icon_rgba};

use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::modes::{Modes, ModesState};
use crate::settings::SettingsState;

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

/// Entrada del menú de la bandeja, independiente de Tauri para poder probarla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Item {
        id: String,
        text: String,
        enabled: bool,
    },
    Check {
        id: String,
        text: String,
        checked: bool,
    },
    Submenu {
        text: String,
        items: Vec<MenuEntry>,
    },
    Separator,
}

fn item(id: &str, text: &str, enabled: bool) -> MenuEntry {
    MenuEntry::Item {
        id: id.into(),
        text: text.into(),
        enabled,
    }
}

/// Contenido del menú según el estado, los modos y el atajo (docs/design/tray-spec.json).
pub fn menu_entries(state: TrayState, modes: &ModesState, shortcut: &str) -> Vec<MenuEntry> {
    let active = modes
        .modes
        .iter()
        .find(|m| m.id == modes.active)
        .map_or("", |m| m.name.as_str());
    let toggle = match state {
        TrayState::Idle => item("toggle", "Empezar a dictar", true),
        TrayState::Recording => item("toggle", "Terminar dictado", true),
        TrayState::Processing => item("toggle", "Empezar a dictar", false),
    };
    vec![
        item("status", state.status_text(), false),
        item("status-mode", &format!("Modo {active} · {shortcut}"), false),
        MenuEntry::Separator,
        toggle,
        MenuEntry::Submenu {
            text: "Modo".into(),
            items: modes
                .modes
                .iter()
                .map(|m| MenuEntry::Check {
                    id: format!("{MODE_PREFIX}{}", m.id),
                    text: m.name.clone(),
                    checked: m.id == modes.active,
                })
                .collect(),
        },
        item("history", "Historial", true),
        item("settings", "Ajustes…", true),
        MenuEntry::Separator,
        item("quit", "Salir de Voice Recorder", true),
    ]
}

fn build_item<R: Runtime>(
    app: &AppHandle<R>,
    entry: &MenuEntry,
) -> tauri::Result<Box<dyn tauri::menu::IsMenuItem<R>>> {
    Ok(match entry {
        MenuEntry::Item { id, text, enabled } => Box::new(MenuItem::with_id(
            app,
            id.as_str(),
            text,
            *enabled,
            None::<&str>,
        )?),
        MenuEntry::Check { id, text, checked } => Box::new(CheckMenuItem::with_id(
            app,
            id.as_str(),
            text,
            true,
            *checked,
            None::<&str>,
        )?),
        MenuEntry::Submenu { text, items } => {
            let built = items
                .iter()
                .map(|e| build_item(app, e))
                .collect::<tauri::Result<Vec<_>>>()?;
            let refs: Vec<&dyn tauri::menu::IsMenuItem<R>> =
                built.iter().map(|b| b.as_ref()).collect();
            Box::new(Submenu::with_items(app, text, true, &refs)?)
        }
        MenuEntry::Separator => Box::new(PredefinedMenuItem::separator(app)?),
    })
}

fn menu<R: Runtime>(app: &AppHandle<R>, state: TrayState) -> tauri::Result<Menu<R>> {
    let modes = app.state::<Modes>().snapshot();
    // La bandeja se crea antes de cargar los ajustes: hasta entonces, el atajo por defecto.
    let shortcut = app.try_state::<SettingsState>().map_or_else(
        || crate::hotkey::default_shortcut().to_string(),
        |s| s.get().hotkey.shortcut,
    );
    let built = menu_entries(state, &modes, &shortcut)
        .iter()
        .map(|e| build_item(app, e))
        .collect::<tauri::Result<Vec<_>>>()?;
    let refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = built.iter().map(|b| b.as_ref()).collect();
    Menu::with_items(app, &refs)
}

/// Id de menú → id de modo (`mode:email` → `email`).
pub fn mode_from_menu_id(id: &str) -> Option<&str> {
    id.strip_prefix(MODE_PREFIX)
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let state = TrayState::default();
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(state))
        .tooltip(state.tooltip())
        .menu(&menu(app, state)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => crate::hotkey::toggle_from_ui(app),
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

/// Reconstruye el menú cuando cambian los modos, el activo, el atajo o el estado.
pub fn rebuild_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let state = *app
            .state::<TrayStore>()
            .0
            .lock()
            .expect("tray state poisoned");
        tray.set_menu(Some(menu(app, state)?))?;
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
    drop(current);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon(next)));
        let _ = tray.set_tooltip(Some(next.tooltip()));
    }
    let _ = rebuild_menu(app);
    let _ = app.emit("tray-state", next);
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

    fn as_spec(entries: &[MenuEntry]) -> serde_json::Value {
        serde_json::Value::Array(
            entries
                .iter()
                .map(|e| match e {
                    MenuEntry::Item { text, enabled, .. } => {
                        serde_json::json!({ "kind": "item", "text": text, "enabled": enabled })
                    }
                    MenuEntry::Check { text, checked, .. } => {
                        serde_json::json!({ "kind": "check", "text": text, "checked": checked })
                    }
                    MenuEntry::Submenu { text, items } => {
                        serde_json::json!({ "kind": "submenu", "text": text, "items": as_spec(items) })
                    }
                    MenuEntry::Separator => serde_json::json!({ "kind": "separator" }),
                })
                .collect(),
        )
    }

    /// Oráculo de la bandeja (HU-16): el menú coincide con el diseño aprobado.
    #[test]
    fn menu_matches_the_approved_design() {
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/design/tray-spec.json")).unwrap();
        let modes = ModesState::initial();
        let entries = menu_entries(
            TrayState::Idle,
            &modes,
            spec["context"]["shortcut"].as_str().unwrap(),
        );
        assert_eq!(as_spec(&entries), spec["menu"]);
    }

    #[test]
    fn menu_follows_the_dictation_state() {
        let modes = ModesState::initial();
        let toggle = |s| {
            menu_entries(s, &modes, "Ctrl+Alt+Space")
                .into_iter()
                .find(|e| matches!(e, MenuEntry::Item { id, .. } if id == "toggle"))
                .unwrap()
        };
        assert_eq!(
            toggle(TrayState::Recording),
            item("toggle", "Terminar dictado", true)
        );
        assert_eq!(
            toggle(TrayState::Processing),
            item("toggle", "Empezar a dictar", false)
        );
        let first = menu_entries(TrayState::Recording, &modes, "x").remove(0);
        assert_eq!(first, item("status", "Grabando…", false));
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
