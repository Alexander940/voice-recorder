//! Atajo global de dictado: mantener / alternar, Esc para cancelar (HU-03).
//!
//! Dos backends entregan las pulsaciones (issue #38):
//! - `tauri-plugin-global-shortcut` en Windows, macOS y Linux X11.
//! - El portal XDG `GlobalShortcuts` en Linux Wayland ([`portal`]).

mod controller;
#[cfg(target_os = "linux")]
pub mod portal;
mod registry;

pub use controller::{DEBOUNCE, DictationAction, HotkeyController, HotkeyInput, HotkeyMode};
pub use registry::{
    ESCAPE, HotkeyError, HotkeyRegistry, LEGACY_LINUX_SHORTCUT, ShortcutRegistrar,
    WAYLAND_CANCEL_SHORTCUT, default_shortcut, mode_shortcut,
};

use std::sync::Mutex;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

/// Qué atajo se ha pulsado, sea cual sea el backend que lo entregue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutKind {
    /// El atajo de dictado (mantener o alternar).
    Main,
    /// Cancela la grabación en curso.
    Cancel,
    /// Pasa al siguiente modo.
    Mode,
}

/// Quién registra los atajos globales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// `tauri-plugin-global-shortcut` (Windows, macOS, Linux X11).
    Plugin,
    /// Portal XDG `GlobalShortcuts` (Linux Wayland).
    Portal,
}

impl Backend {
    /// Texto del atajo que cancela, para el overlay.
    pub fn cancel_label(self) -> &'static str {
        match self {
            Backend::Plugin => "Esc",
            Backend::Portal => "Ctrl+Shift+Esc",
        }
    }
}

/// Backend según la plataforma y la sesión actuales.
pub fn select_backend() -> Backend {
    if crate::platform::uses_portals() {
        Backend::Portal
    } else {
        Backend::Plugin
    }
}

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

/// Registrador elegido en el arranque.
pub type Registrar = Box<dyn ShortcutRegistrar + Send>;

impl ShortcutRegistrar for Registrar {
    fn register(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        (**self).register(shortcut)
    }
    fn unregister(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        (**self).unregister(shortcut)
    }
}

pub struct HotkeyState {
    pub controller: Mutex<HotkeyController>,
    pub registry: Mutex<HotkeyRegistry<Registrar>>,
    pub backend: Backend,
}

/// Plugin con el handler que alimenta al controlador.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| on_shortcut(app, shortcut, event))
        .build()
}

fn make_registrar<R: Runtime>(app: &AppHandle<R>, backend: Backend) -> Registrar {
    match backend {
        Backend::Plugin => Box::new(PluginRegistrar(app.clone())),
        #[cfg(target_os = "linux")]
        Backend::Portal => Box::new(portal::PortalRegistrar::new(app.clone())),
        #[cfg(not(target_os = "linux"))]
        Backend::Portal => unreachable!("el portal solo existe en Linux"),
    }
}

/// Prepara el estado del atajo y registra el de modos. El atajo principal lo
/// registran los ajustes al arrancar. Un conflicto se notifica pero no impide
/// arrancar.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let backend = select_backend();
    tracing::info!("hotkey: backend={}", backend_name(backend));
    let mut registrar = make_registrar(app, backend);
    if let Err(e) = registrar.register(mode_shortcut()) {
        let _ = app.emit("hotkey-error", e.to_string());
    }
    app.manage(HotkeyState {
        controller: Mutex::new(HotkeyController::new(HotkeyMode::default())),
        registry: Mutex::new(HotkeyRegistry::new(registrar)),
        backend,
    });
}

fn backend_name(backend: Backend) -> &'static str {
    match backend {
        Backend::Plugin => "plugin",
        Backend::Portal => "portal",
    }
}

/// Texto del atajo de cancelar que aplica en esta sesión (overlay).
pub fn cancel_label<R: Runtime>(app: &AppHandle<R>) -> &'static str {
    app.try_state::<HotkeyState>()
        .map_or(Backend::Plugin, |s| s.backend)
        .cancel_label()
}

fn classify(shortcut: &Shortcut) -> ShortcutKind {
    if parse(mode_shortcut()).ok() == Some(*shortcut) {
        ShortcutKind::Mode
    } else if parse(ESCAPE).ok() == Some(*shortcut) {
        ShortcutKind::Cancel
    } else {
        ShortcutKind::Main
    }
}

fn on_shortcut<R: Runtime>(app: &AppHandle<R>, shortcut: &Shortcut, event: ShortcutEvent) {
    handle(
        app,
        classify(shortcut),
        event.state() == ShortcutState::Pressed,
    );
}

/// Entrada común de los dos backends: `pressed` es `true` al pulsar y `false`
/// al soltar.
pub fn handle<R: Runtime>(app: &AppHandle<R>, kind: ShortcutKind, pressed: bool) {
    let input = match (kind, pressed) {
        (ShortcutKind::Mode, true) => {
            crate::modes::cycle(app);
            return;
        }
        (ShortcutKind::Mode, false) | (ShortcutKind::Cancel, false) => return,
        (ShortcutKind::Cancel, true) => HotkeyInput::Escape,
        (ShortcutKind::Main, true) => HotkeyInput::Pressed(Instant::now()),
        (ShortcutKind::Main, false) => HotkeyInput::Released(Instant::now()),
    };
    let state = app.state::<HotkeyState>();
    let action = state
        .controller
        .lock()
        .expect("hotkey poisoned")
        .handle(input);
    if let Some(action) = action {
        dispatch(app, &state, action);
    }
}

fn dispatch<R: Runtime>(app: &AppHandle<R>, state: &HotkeyState, action: DictationAction) {
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
    let state = app.state::<HotkeyState>();
    let action = state
        .controller
        .lock()
        .expect("hotkey poisoned")
        .force_stop();
    if let Some(action) = action {
        dispatch(app, &state, action);
    }
}

/// Empieza o termina un dictado desde la interfaz (menú de la bandeja).
pub fn toggle_from_ui<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<HotkeyState>();
    let action = state
        .controller
        .lock()
        .expect("hotkey poisoned")
        .toggle_external(Instant::now());
    dispatch(app, &state, action);
}

/// Cancela la grabación en curso desde la interfaz (botón del overlay).
pub fn cancel_from_ui<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<HotkeyState>();
    let action = state
        .controller
        .lock()
        .expect("hotkey poisoned")
        .cancel_external();
    if let Some(action) = action {
        dispatch(app, &state, action);
    }
}

/// Vuelve a reposo sin acción y libera Esc (el micrófono no arrancó).
pub fn reset<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<HotkeyState>();
    state.controller.lock().expect("hotkey poisoned").reset();
    let _ = state
        .registry
        .lock()
        .expect("hotkey poisoned")
        .set_escape(false);
}

/// Cambia el atajo principal. Devuelve un mensaje legible si choca.
pub fn set_shortcut<R: Runtime>(app: &AppHandle<R>, shortcut: &str) -> Result<(), String> {
    let state = app.state::<HotkeyState>();
    let mut registry = state.registry.lock().expect("hotkey poisoned");
    registry.set_shortcut(shortcut).map_err(|e| e.to_string())
}

pub fn set_mode<R: Runtime>(app: &AppHandle<R>, mode: HotkeyMode) {
    let state = app.state::<HotkeyState>();
    let action = state
        .controller
        .lock()
        .expect("hotkey poisoned")
        .set_mode(mode);
    if let Some(action) = action {
        dispatch(app, &state, action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_escape_shortcuts_parse() {
        assert!(parse(default_shortcut()).is_ok());
        assert!(parse("Option+Space").is_ok());
        assert!(parse("Ctrl+Alt+Space").is_ok());
        assert!(parse("Ctrl+Shift+Space").is_ok());
        assert!(parse(ESCAPE).is_ok());
        assert!(parse(mode_shortcut()).is_ok());
        assert!(parse("Ctrl+Option+M").is_ok());
        assert!(parse(WAYLAND_CANCEL_SHORTCUT).is_ok());
    }

    #[test]
    fn invalid_shortcut_is_rejected() {
        assert_eq!(
            parse("Ctrl+NoExiste"),
            Err(HotkeyError::Invalid("Ctrl+NoExiste".into()))
        );
    }

    #[test]
    fn plugin_shortcuts_are_classified() {
        assert_eq!(
            classify(&parse(mode_shortcut()).unwrap()),
            ShortcutKind::Mode
        );
        assert_eq!(classify(&parse(ESCAPE).unwrap()), ShortcutKind::Cancel);
        assert_eq!(
            classify(&parse(default_shortcut()).unwrap()),
            ShortcutKind::Main
        );
        assert_eq!(classify(&parse("Ctrl+Alt+R").unwrap()), ShortcutKind::Main);
    }

    #[test]
    fn cancel_label_depends_on_backend() {
        assert_eq!(Backend::Plugin.cancel_label(), "Esc");
        assert_eq!(Backend::Portal.cancel_label(), "Ctrl+Shift+Esc");
    }
}
