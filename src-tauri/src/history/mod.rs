//! Historial de transcripciones en SQLite (HU-11). El audio no se guarda.

mod store;

pub use store::{HistoryEntry, HistoryStore, NewEntry, SCHEMA_VERSION};

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, Runtime, State};

use crate::output::{EnigoKeys, OutputMode, Paster, SystemClipboard, TauriNotifier};
use crate::tray::SETTINGS_WINDOW;

pub const DEFAULT_RETENTION_DAYS: u32 = 30;
const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HistorySettings {
    /// `false` = no guardar historial.
    pub enabled: bool,
    /// 0 = conservar para siempre.
    pub retention_days: u32,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: DEFAULT_RETENTION_DAYS,
        }
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Historial de la app: almacén (si se pudo abrir) y ajustes.
pub struct History {
    store: Option<Mutex<HistoryStore>>,
    settings: Mutex<HistorySettings>,
}

impl History {
    pub fn new(store: Option<HistoryStore>) -> Self {
        Self {
            store: store.map(Mutex::new),
            settings: Mutex::new(HistorySettings::default()),
        }
    }

    pub fn open<R: Runtime>(app: &AppHandle<R>) -> Self {
        let store = app.path().app_data_dir().ok().and_then(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            HistoryStore::open(&dir.join("history.sqlite3"))
                .map_err(|e| tracing::warn!(error = %e, "no se pudo abrir el historial"))
                .ok()
        });
        let history = Self::new(store);
        history.purge(now_ms());
        history
    }

    pub fn settings(&self) -> HistorySettings {
        *self.settings.lock().expect("history poisoned")
    }

    pub fn set_settings(&self, s: HistorySettings) {
        *self.settings.lock().expect("history poisoned") = s;
        self.purge(now_ms());
    }

    fn with_store<T>(&self, f: impl FnOnce(&HistoryStore) -> rusqlite::Result<T>) -> Option<T> {
        let store = self.store.as_ref()?.lock().expect("history poisoned");
        f(&store)
            .map_err(|e| tracing::warn!(error = %e, "error de historial"))
            .ok()
    }

    /// Registra un dictado salvo que el historial esté desactivado.
    pub fn record(&self, entry: &NewEntry) -> Option<i64> {
        if !self.settings().enabled {
            return None;
        }
        self.with_store(|s| s.insert(entry))
    }

    pub fn purge(&self, now: i64) -> usize {
        let days = self.settings().retention_days;
        if days == 0 {
            return 0;
        }
        self.with_store(|s| s.purge_older_than(now - days as i64 * DAY_MS))
            .unwrap_or(0)
    }

    pub fn search(&self, query: &str) -> Vec<HistoryEntry> {
        self.with_store(|s| s.search(query, 200))
            .unwrap_or_default()
    }

    pub fn get(&self, id: i64) -> Option<HistoryEntry> {
        self.with_store(|s| s.get(id)).flatten()
    }

    pub fn delete(&self, id: i64) -> bool {
        self.with_store(|s| s.delete(id)).unwrap_or(false)
    }
}

#[tauri::command]
pub fn history_list(history: State<'_, History>, query: String) -> Vec<HistoryEntry> {
    history.search(&query)
}

#[tauri::command]
pub fn history_delete(history: State<'_, History>, id: i64) -> Result<(), String> {
    history
        .delete(id)
        .then_some(())
        .ok_or_else(|| "La entrada no existe".to_string())
}

#[tauri::command]
pub fn history_get_settings(history: State<'_, History>) -> HistorySettings {
    history.settings()
}

#[tauri::command]
pub fn history_set_settings(history: State<'_, History>, settings: HistorySettings) {
    history.set_settings(settings);
}

/// Copia (o vuelve a pegar en la app anterior) el texto final de una entrada.
#[tauri::command]
pub fn history_deliver(app: AppHandle, id: i64, paste: bool) -> Result<(), String> {
    let entry = app
        .state::<History>()
        .get(id)
        .ok_or_else(|| "La entrada no existe".to_string())?;
    let mode = if paste {
        // Al ocultar la ventana el foco vuelve a la app anterior.
        if let Some(w) = app.get_webview_window(SETTINGS_WINDOW) {
            let _ = w.hide();
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        OutputMode::Paste
    } else {
        OutputMode::CopyOnly
    };
    let clipboard = SystemClipboard::new().map_err(|e| e.to_string())?;
    let keys = EnigoKeys::new().map_err(|e| e.to_string())?;
    Paster::new(clipboard, keys, TauriNotifier(app.clone()))
        .deliver(&entry.final_text, mode)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(created_at: i64) -> NewEntry {
        NewEntry {
            created_at,
            raw_text: "raw".into(),
            final_text: "final".into(),
            mode: "clean".into(),
            duration_ms: 1,
            stt_latency_ms: 1,
            llm_latency_ms: None,
        }
    }

    #[test]
    fn disabled_history_does_not_write() {
        let h = History::new(Some(HistoryStore::in_memory().unwrap()));
        h.set_settings(HistorySettings {
            enabled: false,
            retention_days: 30,
        });
        assert_eq!(h.record(&entry(now_ms())), None);
        assert!(h.search("").is_empty());
    }

    #[test]
    fn enabled_history_records_and_lists() {
        let h = History::new(Some(HistoryStore::in_memory().unwrap()));
        let id = h.record(&entry(now_ms())).unwrap();
        assert_eq!(h.search("final")[0].id, id);
        assert!(h.delete(id));
        assert!(h.get(id).is_none());
    }

    #[test]
    fn purge_uses_retention_days_and_zero_keeps_forever() {
        let h = History::new(Some(HistoryStore::in_memory().unwrap()));
        let now = 1_000 * DAY_MS;
        h.record(&entry(now - 10 * DAY_MS)).unwrap();
        h.record(&entry(now)).unwrap();
        *h.settings.lock().unwrap() = HistorySettings {
            enabled: true,
            retention_days: 0,
        };
        assert_eq!(h.purge(now), 0);
        *h.settings.lock().unwrap() = HistorySettings {
            enabled: true,
            retention_days: 7,
        };
        assert_eq!(h.purge(now), 1);
        assert_eq!(h.search("").len(), 1);
    }

    #[test]
    fn missing_store_degrades_gracefully() {
        let h = History::new(None);
        assert_eq!(h.record(&entry(0)), None);
        assert!(h.search("").is_empty());
        assert!(!h.delete(1));
    }

    #[test]
    fn audio_is_not_part_of_the_schema() {
        let entry_fields = serde_json::to_value(HistoryEntry {
            id: 1,
            created_at: 0,
            raw_text: String::new(),
            final_text: String::new(),
            mode: String::new(),
            duration_ms: 0,
            stt_latency_ms: 0,
            llm_latency_ms: None,
        })
        .unwrap();
        assert!(
            entry_fields
                .as_object()
                .unwrap()
                .keys()
                .all(|k| !k.contains("audio") && !k.contains("wav"))
        );
    }
}
