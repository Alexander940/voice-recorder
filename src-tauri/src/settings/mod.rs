//! Configuración persistente y API keys en el keyring (HU-07, HU-12).

pub mod keys;

use keys::{HttpValidator, KEYRING_SERVICE, KeyManager, KeyStatus, KeyringStore, Provider};
use tauri::{AppHandle, Manager, State};

pub type AppKeys = KeyManager<KeyringStore, HttpValidator>;

pub fn app_keys() -> AppKeys {
    KeyManager::new(KeyringStore::new(KEYRING_SERVICE), HttpValidator::new())
}

#[tauri::command]
pub fn get_key_status(keys: State<'_, AppKeys>) -> KeyStatus {
    keys.status()
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
