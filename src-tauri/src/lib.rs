pub mod audio;
pub mod dictation;
pub mod errors;
pub mod history;
pub mod hotkey;
pub mod llm;
pub mod logging;
pub mod modes;
pub mod output;
pub mod settings;
pub mod sounds;
pub mod stt;
pub mod tray;
pub mod updater;
pub mod vocabulary;

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
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(tray::TrayStore::default())
        .manage(dictation::DictationState::default())
        .manage(logging::Redactions::default())
        .manage(updater::UpdateState::default())
        .manage(settings::app_keys())
        .manage(llm::CleanupSettings::default())
        .setup(|app| {
            if let Ok(dir) = app.path().app_log_dir() {
                logging::init(&dir, app.state::<logging::Redactions>().inner().clone());
            }
            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                "Voice Recorder iniciado"
            );
            // Solo bandeja / barra de menú: sin icono en el Dock.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            app.manage(history::History::open(app.handle()));
            app.manage(modes::Modes::open(app.handle()));
            app.manage(vocabulary::VocabularyStore::open(app.handle()));
            tray::build(app.handle())?;
            hotkey::init(app.handle());
            // Después del atajo y el historial: los ajustes los configuran.
            settings::init(app.handle());
            updater::start(app.handle());
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
            list_input_devices,
            settings::get_key_status,
            settings::save_api_key,
            settings::delete_api_key,
            llm::set_cleanup_enabled,
            history::history_list,
            history::history_delete,
            history::history_get_settings,
            history::history_set_settings,
            history::history_deliver,
            modes::modes_list,
            modes::modes_set_active,
            modes::modes_save,
            modes::modes_duplicate,
            modes::modes_delete,
            vocabulary::vocabulary_get,
            vocabulary::vocabulary_save,
            settings::settings_get,
            settings::settings_update,
            dictation::dictation_retry_available,
            dictation::dictation_retry,
            updater::update_status,
            updater::update_install
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
