//! Modos personalizados (HU-09): predefinidos + propios, persistidos en JSON.

mod model;

pub use model::{DEFAULT_MODE, Mode, ModeError, ModesState, builtin_modes};

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// Dónde se guardan los modos.
pub trait ModeStore: Send + Sync {
    fn load(&self) -> Option<ModesState>;
    fn save(&self, state: &ModesState) -> Result<(), String>;
}

pub struct JsonFileStore(pub PathBuf);

impl ModeStore for JsonFileStore {
    fn load(&self) -> Option<ModesState> {
        let text = std::fs::read_to_string(&self.0).ok()?;
        serde_json::from_str(&text)
            .map_err(
                |e| tracing::warn!(error = %e, "modes.json inválido, se usan los predefinidos"),
            )
            .ok()
    }

    fn save(&self, state: &ModesState) -> Result<(), String> {
        if let Some(dir) = self.0.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
        // Escritura atómica: archivo temporal + renombrado.
        let tmp = self.0.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.0).map_err(|e| e.to_string())
    }
}

pub struct Modes {
    state: Mutex<ModesState>,
    store: Box<dyn ModeStore>,
}

impl Modes {
    pub fn new(store: Box<dyn ModeStore>) -> Self {
        let state = store
            .load()
            .map(ModesState::normalized)
            .unwrap_or_else(ModesState::initial);
        Self {
            state: Mutex::new(state),
            store,
        }
    }

    pub fn open<R: Runtime>(app: &AppHandle<R>) -> Self {
        let path = app
            .path()
            .app_data_dir()
            .map(|d| d.join("modes.json"))
            .unwrap_or_else(|_| PathBuf::from("modes.json"));
        Self::new(Box::new(JsonFileStore(path)))
    }

    pub fn snapshot(&self) -> ModesState {
        self.state.lock().expect("modes poisoned").clone()
    }

    pub fn active(&self) -> Mode {
        self.state
            .lock()
            .expect("modes poisoned")
            .active_mode()
            .clone()
    }

    /// Aplica un cambio y lo persiste.
    pub fn update<T>(
        &self,
        f: impl FnOnce(&mut ModesState) -> Result<T, ModeError>,
    ) -> Result<ModesState, String> {
        let mut state = self.state.lock().expect("modes poisoned");
        f(&mut state).map_err(|e| e.to_string())?;
        self.store.save(&state)?;
        Ok(state.clone())
    }
}

/// Avisa a la bandeja y al overlay de que cambió el modo o la lista.
pub fn notify_changed<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<Modes>().snapshot();
    let _ = crate::tray::rebuild_menu(app);
    let _ = app.emit("mode-changed", state.active_mode().name.clone());
}

fn changed<R: Runtime>(
    app: &AppHandle<R>,
    result: Result<ModesState, String>,
) -> Result<ModesState, String> {
    if result.is_ok() {
        notify_changed(app);
    }
    result
}

#[tauri::command]
pub fn modes_list(modes: State<'_, Modes>) -> ModesState {
    modes.snapshot()
}

#[tauri::command]
pub fn modes_set_active(app: AppHandle, id: String) -> Result<ModesState, String> {
    let r = app.state::<Modes>().update(|s| s.set_active(&id));
    changed(&app, r)
}

#[tauri::command]
pub fn modes_save(app: AppHandle, mode: Mode) -> Result<ModesState, String> {
    let r = app.state::<Modes>().update(|s| s.save(mode));
    changed(&app, r)
}

#[tauri::command]
pub fn modes_duplicate(app: AppHandle, id: String) -> Result<ModesState, String> {
    let r = app.state::<Modes>().update(|s| s.duplicate(&id));
    changed(&app, r)
}

#[tauri::command]
pub fn modes_delete(app: AppHandle, id: String) -> Result<ModesState, String> {
    let r = app.state::<Modes>().update(|s| s.delete(&id));
    changed(&app, r)
}

/// Atajo secundario: siguiente modo.
pub fn cycle<R: Runtime>(app: &AppHandle<R>) {
    let r = app.state::<Modes>().update(|s| {
        s.cycle();
        Ok(())
    });
    let _ = changed(app, r);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemoryStore(Mutex<Option<ModesState>>);
    impl ModeStore for MemoryStore {
        fn load(&self) -> Option<ModesState> {
            self.0.lock().unwrap().clone()
        }
        fn save(&self, s: &ModesState) -> Result<(), String> {
            *self.0.lock().unwrap() = Some(s.clone());
            Ok(())
        }
    }

    #[test]
    fn changes_are_persisted_and_reloaded_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("modes.json");
        let modes = Modes::new(Box::new(JsonFileStore(path.clone())));
        modes.update(|s| s.set_active("email")).unwrap();
        modes
            .update(|s| {
                s.save(Mode {
                    id: String::new(),
                    name: "Tuits".into(),
                    prompt: "Máximo 280 caracteres.".into(),
                    output_language: None,
                    use_llm: true,
                    builtin: false,
                })
            })
            .unwrap();

        let reloaded = Modes::new(Box::new(JsonFileStore(path)));
        let s = reloaded.snapshot();
        assert_eq!(s.active, "email");
        assert!(s.modes.iter().any(|m| m.name == "Tuits"));
    }

    #[test]
    fn missing_or_corrupt_file_falls_back_to_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("modes.json");
        std::fs::write(&path, "{ no es json").unwrap();
        let modes = Modes::new(Box::new(JsonFileStore(path)));
        assert_eq!(modes.snapshot(), ModesState::initial());
    }

    #[test]
    fn failed_change_is_not_persisted() {
        let modes = Modes::new(Box::new(MemoryStore::default()));
        assert!(modes.update(|s| s.delete("literal")).is_err());
        assert!(modes.store.load().is_none());
    }

    #[test]
    fn active_mode_defaults_to_clean() {
        let modes = Modes::new(Box::new(MemoryStore::default()));
        assert_eq!(modes.active().id, DEFAULT_MODE);
    }
}
