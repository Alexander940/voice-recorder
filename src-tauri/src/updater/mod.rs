//! Actualizaciones automáticas (HU-15): comprueba al iniciar y una vez al día,
//! avisa y solo instala cuando el usuario lo confirma.

mod core;

pub use core::{
    Available, CHECK_INTERVAL, Manifest, UpdateChecker, UpdateError, evaluate, platform_key,
    should_check, verify_artifact,
};

use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_updater::UpdaterExt;

use crate::output::{Notifier, TauriNotifier};
use crate::settings::SettingsState;

/// Cada cuánto se despierta el hilo para ver si toca comprobar.
const TICK: Duration = Duration::from_secs(60 * 60);

#[derive(Default)]
pub struct UpdateState {
    last_check: Mutex<Option<SystemTime>>,
    available: Mutex<Option<Available>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct UpdateStatus {
    /// Hay clave pública y endpoint en la configuración.
    pub configured: bool,
    pub enabled: bool,
    pub available: Option<Available>,
}

/// Endpoint y clave pública de `plugins.updater` en tauri.conf.json.
fn checker<R: Runtime>(app: &AppHandle<R>) -> UpdateChecker {
    let conf = app.config().plugins.0.get("updater");
    let pubkey = conf
        .and_then(|c| c.get("pubkey"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let endpoint = conf
        .and_then(|c| c.get("endpoints"))
        .and_then(|v| v.get(0))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    UpdateChecker::new(endpoint, pubkey)
}

fn tick<R: Runtime>(app: &AppHandle<R>) {
    let enabled = app.state::<SettingsState>().get().updates.enabled;
    let checker = checker(app);
    let state = app.state::<UpdateState>();
    let last = *state.last_check.lock().expect("update poisoned");
    if !checker.configured() || !should_check(enabled, last, SystemTime::now()) {
        return;
    }
    *state.last_check.lock().expect("update poisoned") = Some(SystemTime::now());
    match checker.check(&app.package_info().version.to_string()) {
        Ok(Some(available)) => {
            tracing::info!(version = %available.version, "actualización disponible");
            TauriNotifier(app.clone()).notify(
                "Actualización disponible",
                &format!(
                    "Voice Recorder {} está lista. Instálala desde Ajustes.",
                    available.version
                ),
            );
            let _ = app.emit("update-available", &available);
            *state.available.lock().expect("update poisoned") = Some(available);
        }
        Ok(None) => tracing::info!("sin actualizaciones"),
        Err(e) => tracing::warn!(error = %e, "no se pudo comprobar actualizaciones"),
    }
}

/// Lanza la comprobación al iniciar y cada hora mira si toca la diaria.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            tick(&app);
            std::thread::sleep(TICK);
        }
    });
}

#[tauri::command]
pub fn update_status(app: AppHandle, state: State<'_, UpdateState>) -> UpdateStatus {
    UpdateStatus {
        configured: checker(&app).configured(),
        enabled: app.state::<SettingsState>().get().updates.enabled,
        available: state.available.lock().expect("update poisoned").clone(),
    }
}

/// Descarga, verifica la firma e instala; después reinicia la app.
/// Solo se llama cuando el usuario lo confirma.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let pubkey = {
        let conf = app.config().plugins.0.get("updater");
        conf.and_then(|c| c.get("pubkey"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    if pubkey.trim().is_empty() {
        return Err(UpdateError::NotConfigured.to_string());
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No hay ninguna actualización disponible".to_string())?;
    // El plugin ya verifica la firma; se comprueba de nuevo con nuestra lógica probada.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    verify_artifact(&bytes, &update.signature, &pubkey).map_err(|e| e.to_string())?;
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}
