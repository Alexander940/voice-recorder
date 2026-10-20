pub mod audio;
pub mod dictation;
pub mod history;
pub mod hotkey;
pub mod llm;
pub mod output;
pub mod settings;
pub mod stt;
pub mod tray;

use tauri::{Manager, WindowEvent};

/// Versión de la app, expuesta al frontend.
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Micrófonos disponibles (para los ajustes, HU-12).
#[tauri::command]
fn list_input_devices() -> Vec<String> {
    audio::input_device_names()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(hotkey::plugin())
        .manage(tray::TrayStore::default())
        .manage(dictation::DictationState::default())
        .manage(settings::app_keys())
        .setup(|app| {
            // Solo bandeja / barra de menú: sin icono en el Dock.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::build(app.handle())?;
            hotkey::init(app.handle());
            // Primer arranque: sin key de Groq no se puede dictar.
            if !app.state::<settings::AppKeys>().status().groq {
                tray::show_window(app.handle(), tray::View::Onboarding);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                match tray::close_action(window.label()) {
                    tray::CloseAction::Hide => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            hotkey::set_hotkey,
            hotkey::set_hotkey_mode,
            list_input_devices,
            settings::get_key_status,
            settings::save_api_key,
            settings::delete_api_key
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_version_matches_cargo_manifest() {
        assert_eq!(app_version(), "0.1.0");
    }
}
