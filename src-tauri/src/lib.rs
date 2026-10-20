pub mod audio;
pub mod history;
pub mod llm;
pub mod output;
pub mod settings;
pub mod stt;
pub mod tray;

use tauri::WindowEvent;

/// Versión de la app, expuesta al frontend.
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(tray::TrayStore::default())
        .setup(|app| {
            // Solo bandeja / barra de menú: sin icono en el Dock.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::build(app.handle())?;
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
        .invoke_handler(tauri::generate_handler![app_version])
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
