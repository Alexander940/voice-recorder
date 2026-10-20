//! Atajo global de dictado: mantener / alternar, Esc para cancelar (HU-03).

mod controller;
mod registry;

pub use controller::{DEBOUNCE, DictationAction, HotkeyController, HotkeyInput, HotkeyMode};
pub use registry::{ESCAPE, HotkeyError, HotkeyRegistry, ShortcutRegistrar, default_shortcut};

use std::sync::Mutex;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

/// Registrador real sobre `tauri-plugin-global-shortcut`.
pub struct PluginRegistrar<R: Runtime>(pub AppHandle<R>);

fn parse(shortcut: &str) -> Result<Shortcut, HotkeyError> {
    shortcut
        .parse::<Shortcut>()
        .map_err(|_| HotkeyError::Invalid(shortcut.into()))
}

impl<R: Runtime> ShortcutRegistrar for PluginRegistrar<R> {
    fn register(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        let parsed = parse(shortcut)?;
        // El SO rechaza los atajos que ya usa otra app.
        self.0
            .global_shortcut()
            .register(parsed)
            .map_err(|_| HotkeyError::Conflict(shortcut.into()))
    }

    fn unregister(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        let parsed = parse(shortcut)?;
        self.0
            .global_shortcut()
            .unregister(parsed)
            .map_err(|e| HotkeyError::Other(e.to_string()))
    }
}

pub struct HotkeyState<R: Runtime> {
    pub controller: Mutex<HotkeyController>,
    pub registry: Mutex<HotkeyRegistry<PluginRegistrar<R>>>,
}

/// Plugin con el handler que alimenta al controlador.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| on_shortcut(app, shortcut, event))
        .build()
}

/// Registra el atajo por defecto. Un conflicto se notifica pero no impide arrancar.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let mut registry = HotkeyRegistry::new(PluginRegistrar(app.clone()));
    if let Err(e) = registry.set_shortcut(default_shortcut()) {
        let _ = app.emit("hotkey-error", e.to_string());
    }
    app.manage(HotkeyState {
        controller: Mutex::new(HotkeyController::new(HotkeyMode::default())),
        registry: Mutex::new(registry),
    });
}

fn on_shortcut<R: Runtime>(app: &AppHandle<R>, shortcut: &Shortcut, event: ShortcutEvent) {
    let state = app.state::<HotkeyState<R>>();
    let input = match (parse(ESCAPE).ok() == Some(*shortcut), event.state()) {
        (true, ShortcutState::Pressed) => HotkeyInput::Escape,
        (true, ShortcutState::Released) => return,
        (false, ShortcutState::Pressed) => HotkeyInput::Pressed(Instant::now()),
        (false, ShortcutState::Released) => HotkeyInput::Released(Instant::now()),
    };
    let action = state
        .controller
        .lock()
        .expect("hotkey poisoned")
        .handle(input);
    if let Some(action) = action {
        dispatch(app, &state, action);
    }
}

fn dispatch<R: Runtime>(app: &AppHandle<R>, state: &HotkeyState<R>, action: DictationAction) {
    let recording = action == DictationAction::Start;
    let _ = state
        .registry
        .lock()
        .expect("hotkey poisoned")
        .set_escape(recording);
    crate::dictation::on_action(app, action);
    let _ = app.emit("dictation", action);
}

/// Termina la grabación en curso desde fuera del atajo (límite de duración).
pub fn force_stop<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<HotkeyState<R>>();
    let action = state
        .controller
        .lock()
        .expect("hotkey poisoned")
        .force_stop();
    if let Some(action) = action {
        dispatch(app, &state, action);
    }
}

/// Vuelve a reposo sin acción y libera Esc (el micrófono no arrancó).
pub fn reset<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<HotkeyState<R>>();
    state.controller.lock().expect("hotkey poisoned").reset();
    let _ = state
        .registry
        .lock()
        .expect("hotkey poisoned")
        .set_escape(false);
}

/// Cambia el atajo principal. Devuelve un mensaje legible si choca.
#[tauri::command]
pub fn set_hotkey(app: AppHandle, shortcut: String) -> Result<(), String> {
    let state = app.state::<HotkeyState<tauri::Wry>>();
    let mut registry = state.registry.lock().expect("hotkey poisoned");
    registry.set_shortcut(&shortcut).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_hotkey_mode(app: AppHandle, mode: HotkeyMode) {
    let state = app.state::<HotkeyState<tauri::Wry>>();
    let action = state
        .controller
        .lock()
        .expect("hotkey poisoned")
        .set_mode(mode);
    if let Some(action) = action {
        dispatch(&app, &state, action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_escape_shortcuts_parse() {
        assert!(parse(default_shortcut()).is_ok());
        assert!(parse("Cmd+Option+Space").is_ok());
        assert!(parse("Ctrl+Alt+Space").is_ok());
        assert!(parse(ESCAPE).is_ok());
    }

    #[test]
    fn invalid_shortcut_is_rejected() {
        assert_eq!(
            parse("Ctrl+NoExiste"),
            Err(HotkeyError::Invalid("Ctrl+NoExiste".into()))
        );
    }
}
