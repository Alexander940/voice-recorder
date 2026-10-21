//! Configuración persistente y API keys en el keyring (HU-07, HU-12).

pub mod app;
pub mod keys;

pub use app::{AppSettings, SettingsEffects, apply_changes};

use std::sync::Mutex;

use keys::{
    HttpValidator, KEYRING_SERVICE, KeyHints, KeyManager, KeyStatus, KeyringStore, Provider,
};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

use crate::history::{History, HistorySettings};
use crate::hotkey::{self, HotkeyMode, default_shortcut};
use crate::llm::CleanupSettings;

const STORE_FILE: &str = "settings.json";
const STORE_KEY: &str = "settings";

/// Ajustes vigentes de la app en marcha.
#[derive(Default)]
pub struct SettingsState(pub Mutex<AppSettings>);

impl SettingsState {
    pub fn get(&self) -> AppSettings {
        self.0.lock().expect("settings poisoned").clone()
    }
}

/// Efectos reales sobre la app.
struct TauriEffects<'a, R: Runtime>(&'a AppHandle<R>);

impl<R: Runtime> SettingsEffects for TauriEffects<'_, R> {
    fn set_shortcut(&mut self, shortcut: &str) -> Result<(), String> {
        hotkey::set_shortcut(self.0, shortcut)
    }
    fn set_hotkey_mode(&mut self, mode: HotkeyMode) {
        hotkey::set_mode(self.0, mode);
    }
    fn set_cleanup(&mut self, enabled: bool) {
        self.0.state::<CleanupSettings>().set_enabled(enabled);
    }
    fn set_history(&mut self, settings: HistorySettings) {
        self.0.state::<History>().set_settings(settings);
    }
    fn set_autostart(&mut self, enabled: bool) -> Result<(), String> {
        let launcher = self.0.autolaunch();
        let current = launcher.is_enabled().unwrap_or(!enabled);
        if current == enabled {
            return Ok(());
        }
        if enabled {
            launcher.enable()
        } else {
            launcher.disable()
        }
        .map_err(|e| format!("No se pudo cambiar el inicio automático: {e}"))
    }
}

fn load<R: Runtime>(app: &AppHandle<R>) -> AppSettings {
    app.store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(STORE_KEY))
        .map(AppSettings::from_json)
        .unwrap_or_default()
}

fn persist<R: Runtime>(app: &AppHandle<R>, settings: &AppSettings) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    store.set(
        STORE_KEY,
        serde_json::to_value(settings).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())
}

/// Carga y aplica los ajustes al arrancar. Si el atajo guardado choca, se
/// intenta el predeterminado y se avisa.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let mut settings = load(app);
    if let Err(e) = apply_changes(&mut TauriEffects(app), None, &settings) {
        let _ = app.emit("hotkey-error", e);
        settings.hotkey.shortcut = default_shortcut().into();
        if let Err(e) = apply_changes(&mut TauriEffects(app), None, &settings) {
            let _ = app.emit("hotkey-error", e);
        }
    }
    app.manage(SettingsState(Mutex::new(settings)));
    // La bandeja se creó con el atajo por defecto: ahora muestra el guardado.
    let _ = crate::tray::rebuild_menu(app);
}

#[tauri::command]
pub fn settings_get(state: State<'_, SettingsState>) -> AppSettings {
    state.get()
}

/// Valida, aplica en caliente y persiste. Si algo falla, nada cambia.
#[tauri::command]
pub fn settings_update(app: AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    settings.validate()?;
    let state = app.state::<SettingsState>();
    let mut current = state.0.lock().expect("settings poisoned");
    apply_changes(&mut TauriEffects(&app), Some(&current), &settings)?;
    persist(&app, &settings)?;
    *current = settings.clone();
    drop(current);
    // El menú de la bandeja muestra el atajo.
    let _ = crate::tray::rebuild_menu(&app);
    let _ = app.emit("settings-changed", &settings);
    Ok(settings)
}

pub type AppKeys = KeyManager<KeyringStore, HttpValidator>;

pub fn app_keys() -> AppKeys {
    KeyManager::new(KeyringStore::new(KEYRING_SERVICE), HttpValidator::new())
}

#[tauri::command]
pub fn get_key_status(keys: State<'_, AppKeys>) -> KeyStatus {
    keys.status()
}

/// Prefijo y últimos 4 caracteres de cada key guardada (nunca la key entera).
#[tauri::command]
pub fn get_key_hints(keys: State<'_, AppKeys>) -> KeyHints {
    keys.hints()
}

/// La validación hace una petición de red bloqueante: va a un hilo de trabajo.
#[tauri::command]
pub async fn save_api_key(
    app: AppHandle,
    provider: Provider,
    key: String,
) -> Result<KeyStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppKeys>()
            .save(provider, &key)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn delete_api_key(keys: State<'_, AppKeys>, provider: Provider) -> Result<KeyStatus, String> {
    keys.delete(provider).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    /// Tipos de retorno permitidos en comandos de Tauri. Ninguno puede contener
    /// una API key: añadir un comando nuevo obliga a revisar esta lista.
    const ALLOWED_RETURNS: &[&str] = &[
        "&'static str",
        "Vec<String>",
        "Result<(), String>",
        "KeyStatus",
        "Result<KeyStatus, String>",
        "Vec<HistoryEntry>",
        "HistorySettings",
        "ModesState",
        "Result<ModesState, String>",
        "Vocabulary",
        "Result<Vocabulary, String>",
        "AppSettings",
        "Result<AppSettings, String>",
        "bool",
        "UpdateStatus",
        "Result<UpdateStatus, String>",
        "KeyHints",
    ];

    fn rust_sources(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push((
                    path.display().to_string(),
                    std::fs::read_to_string(&path).unwrap(),
                ));
            }
        }
    }

    #[test]
    fn no_tauri_command_can_return_an_api_key() {
        let mut files = Vec::new();
        rust_sources(std::path::Path::new("src"), &mut files);
        let re = Regex::new(r"#\[tauri::command\]\s*pub\s+(?:async\s+)?fn\s+(\w+)[^{]*?\)\s*(?:->\s*([^{]+?))?\s*\{|#\[tauri::command\]\s*(?:async\s+)?fn\s+(\w+)[^{]*?\)\s*(?:->\s*([^{]+?))?\s*\{").unwrap();
        let mut found = 0;
        for (path, src) in &files {
            for cap in re.captures_iter(src) {
                found += 1;
                let name = cap.get(1).or(cap.get(3)).unwrap().as_str();
                let ret = cap
                    .get(2)
                    .or(cap.get(4))
                    .map(|m| m.as_str().trim())
                    .unwrap_or("()");
                assert!(
                    ret == "()" || ALLOWED_RETURNS.contains(&ret),
                    "{path}: el comando `{name}` devuelve `{ret}`, que no está en la lista permitida"
                );
            }
        }
        assert!(
            found >= 6,
            "solo se encontraron {found} comandos; ¿cambió el formato?"
        );
    }
}
