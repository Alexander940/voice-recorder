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

/// Destinos externos que la interfaz puede abrir. Son fijos: el frontend nunca
/// pasa una URL arbitraria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum LinkTarget {
    Groq,
    Anthropic,
    Accessibility,
}

fn link_url(target: LinkTarget) -> &'static str {
    match target {
        LinkTarget::Groq => "https://console.groq.com/keys",
        LinkTarget::Anthropic => "https://console.anthropic.com/settings/keys",
        LinkTarget::Accessibility => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
    }
}

/// Abre un enlace en el navegador o en Ajustes del Sistema.
#[tauri::command]
fn open_link(target: LinkTarget) -> Result<(), String> {
    let url = link_url(target);
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let spawned = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let spawned = std::process::Command::new("xdg-open").arg(url).spawn();
    spawned.map(|_| ()).map_err(|e| e.to_string())
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
            dictation::dictation_cancel,
            dictation::overlay_dismiss,
            dictation::overlay_fit,
            settings::get_key_hints,
            updater::update_status,
            updater::update_check_now,
            updater::update_install,
            open_link
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_fixed_and_safe() {
        for t in [LinkTarget::Groq, LinkTarget::Anthropic] {
            assert!(link_url(t).starts_with("https://console."));
        }
        assert!(link_url(LinkTarget::Accessibility).starts_with("x-apple.systempreferences:"));
        assert!(serde_json::from_str::<LinkTarget>("\"https://evil.example\"").is_err());
    }

    #[test]
    fn app_version_matches_cargo_manifest() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
    }
}
