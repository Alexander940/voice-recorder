//! Ajustes de la app: modelo, migración, validación y aplicación de cambios.

use serde_json::Value;

use crate::history::HistorySettings;
use crate::hotkey::{HotkeyMode, default_shortcut};
use crate::stt::DEFAULT_MODEL;

pub const SETTINGS_VERSION: u32 = 1;
pub const TRANSCRIPTION_MODELS: [&str; 2] = ["whisper-large-v3-turbo", "whisper-large-v3"];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct GeneralSettings {
    pub autostart: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HotkeySettings {
    pub shortcut: String,
    pub mode: HotkeyMode,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            shortcut: default_shortcut().into(),
            mode: HotkeyMode::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// `None` = micrófono predeterminado del sistema.
    pub device: Option<String>,
    pub max_duration_secs: u64,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            device: None,
            max_duration_secs: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TranscriptionSettings {
    pub model: String,
    /// `None` = detección automática.
    pub language: Option<String>,
}

impl Default for TranscriptionSettings {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.into(),
            language: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub cleanup: bool,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self { cleanup: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct OutputSettings {
    /// Solo copiar al portapapeles, sin pegar.
    pub copy_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub version: u32,
    pub general: GeneralSettings,
    pub hotkey: HotkeySettings,
    pub audio: AudioSettings,
    pub transcription: TranscriptionSettings,
    pub ai: AiSettings,
    pub output: OutputSettings,
    pub privacy: HistorySettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            general: GeneralSettings::default(),
            hotkey: HotkeySettings::default(),
            audio: AudioSettings::default(),
            transcription: TranscriptionSettings::default(),
            ai: AiSettings::default(),
            output: OutputSettings::default(),
            privacy: HistorySettings::default(),
        }
    }
}

impl AppSettings {
    /// Carga desde JSON tolerando campos ausentes o desconocidos y migrando
    /// el formato plano v0 (sin `version`).
    pub fn from_json(value: Value) -> Self {
        let migrated = if value.get("version").is_none() {
            migrate_v0(&value)
        } else {
            value
        };
        let mut s: AppSettings = serde_json::from_value(migrated).unwrap_or_default();
        s.version = SETTINGS_VERSION;
        s
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.hotkey.shortcut.trim().is_empty() {
            return Err("El atajo no puede estar vacío".into());
        }
        if !(10..=1800).contains(&self.audio.max_duration_secs) {
            return Err("La duración máxima debe estar entre 10 s y 30 min".into());
        }
        if !TRANSCRIPTION_MODELS.contains(&self.transcription.model.as_str()) {
            return Err(format!("Modelo desconocido: {}", self.transcription.model));
        }
        if let Some(lang) = &self.transcription.language
            && (lang.len() != 2 || !lang.chars().all(|c| c.is_ascii_lowercase()))
        {
            return Err("El idioma debe ser un código de dos letras (es, en…)".into());
        }
        Ok(())
    }
}

/// v0: claves planas de una versión previa del formato.
fn migrate_v0(v: &Value) -> Value {
    let get = |k: &str| v.get(k).cloned();
    let mut out = serde_json::json!({ "version": SETTINGS_VERSION });
    let mut put = |section: &str, key: &str, value: Option<Value>| {
        if let Some(value) = value {
            out[section][key] = value;
        }
    };
    put("hotkey", "shortcut", get("shortcut"));
    put("hotkey", "mode", get("hotkey_mode"));
    put("ai", "cleanup", get("cleanup_enabled"));
    put("transcription", "language", get("language"));
    put("audio", "device", get("microphone"));
    out
}

/// Efectos de aplicar ajustes sobre la app en marcha.
pub trait SettingsEffects {
    fn set_shortcut(&mut self, shortcut: &str) -> Result<(), String>;
    fn set_hotkey_mode(&mut self, mode: HotkeyMode);
    fn set_cleanup(&mut self, enabled: bool);
    fn set_history(&mut self, settings: HistorySettings);
    fn set_autostart(&mut self, enabled: bool) -> Result<(), String>;
}

/// Aplica solo lo que cambió (`old = None` = arranque: aplica todo).
/// El atajo va primero: si falla, no se aplica nada más.
pub fn apply_changes(
    effects: &mut impl SettingsEffects,
    old: Option<&AppSettings>,
    new: &AppSettings,
) -> Result<(), String> {
    if old.is_none_or(|o| o.hotkey.shortcut != new.hotkey.shortcut) {
        effects.set_shortcut(&new.hotkey.shortcut)?;
    }
    if old.is_none_or(|o| o.hotkey.mode != new.hotkey.mode) {
        effects.set_hotkey_mode(new.hotkey.mode);
    }
    if old.is_none_or(|o| o.ai.cleanup != new.ai.cleanup) {
        effects.set_cleanup(new.ai.cleanup);
    }
    if old.is_none_or(|o| o.privacy != new.privacy) {
        effects.set_history(new.privacy);
    }
    if old.is_none_or(|o| o.general.autostart != new.general.autostart) {
        effects.set_autostart(new.general.autostart)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Default)]
    struct Recorder {
        calls: Vec<String>,
        taken: Vec<String>,
    }

    impl SettingsEffects for Recorder {
        fn set_shortcut(&mut self, s: &str) -> Result<(), String> {
            if self.taken.iter().any(|t| t == s) {
                return Err(format!("El atajo «{s}» ya está en uso"));
            }
            self.calls.push(format!("shortcut:{s}"));
            Ok(())
        }
        fn set_hotkey_mode(&mut self, m: HotkeyMode) {
            self.calls.push(format!("mode:{m:?}"));
        }
        fn set_cleanup(&mut self, e: bool) {
            self.calls.push(format!("cleanup:{e}"));
        }
        fn set_history(&mut self, h: HistorySettings) {
            self.calls
                .push(format!("history:{}:{}", h.enabled, h.retention_days));
        }
        fn set_autostart(&mut self, e: bool) -> Result<(), String> {
            self.calls.push(format!("autostart:{e}"));
            Ok(())
        }
    }

    #[test]
    fn defaults_roundtrip_through_json() {
        let s = AppSettings::default();
        let back = AppSettings::from_json(serde_json::to_value(&s).unwrap());
        assert_eq!(back, s);
        assert_eq!(s.hotkey.shortcut, default_shortcut());
        assert!(s.ai.cleanup);
        assert_eq!(s.audio.max_duration_secs, 300);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn partial_file_fills_missing_fields_and_ignores_unknown() {
        let s = AppSettings::from_json(json!({
            "version": 1,
            "audio": { "device": "USB Mic" },
            "futuro": true
        }));
        assert_eq!(s.audio.device.as_deref(), Some("USB Mic"));
        assert_eq!(s.audio.max_duration_secs, 300);
        assert_eq!(s.hotkey, HotkeySettings::default());
    }

    #[test]
    fn legacy_flat_v0_file_migrates() {
        let s = AppSettings::from_json(json!({
            "shortcut": "Ctrl+Shift+Space",
            "hotkey_mode": "toggle",
            "cleanup_enabled": false,
            "language": "es",
            "microphone": "Headset"
        }));
        assert_eq!(s.version, SETTINGS_VERSION);
        assert_eq!(s.hotkey.shortcut, "Ctrl+Shift+Space");
        assert_eq!(s.hotkey.mode, HotkeyMode::Toggle);
        assert!(!s.ai.cleanup);
        assert_eq!(s.transcription.language.as_deref(), Some("es"));
        assert_eq!(s.audio.device.as_deref(), Some("Headset"));
    }

    #[test]
    fn garbage_falls_back_to_defaults() {
        assert_eq!(
            AppSettings::from_json(json!({"version": 1, "audio": "no"})),
            AppSettings::default()
        );
    }

    #[test]
    fn validation_rejects_bad_values() {
        let mut s = AppSettings::default();
        s.audio.max_duration_secs = 5;
        assert!(s.validate().is_err());
        let mut s = AppSettings::default();
        s.transcription.model = "gpt".into();
        assert!(s.validate().is_err());
        let mut s = AppSettings::default();
        s.transcription.language = Some("ES".into());
        assert!(s.validate().is_err());
        let mut s = AppSettings::default();
        s.hotkey.shortcut = " ".into();
        assert!(s.validate().is_err());
    }

    #[test]
    fn startup_applies_everything() {
        let mut fx = Recorder::default();
        apply_changes(&mut fx, None, &AppSettings::default()).unwrap();
        assert_eq!(fx.calls.len(), 5);
        assert!(fx.calls[0].starts_with("shortcut:"));
    }

    #[test]
    fn only_changed_settings_are_applied_and_hotkey_is_reregistered() {
        let old = AppSettings::default();
        let mut new = old.clone();
        new.hotkey.shortcut = "Ctrl+Alt+R".into();
        new.ai.cleanup = false;
        new.audio.device = Some("USB".into()); // se lee al dictar: sin efecto inmediato
        let mut fx = Recorder::default();
        apply_changes(&mut fx, Some(&old), &new).unwrap();
        assert_eq!(fx.calls, ["shortcut:Ctrl+Alt+R", "cleanup:false"]);
    }

    #[test]
    fn conflicting_shortcut_aborts_before_other_effects() {
        let old = AppSettings::default();
        let mut new = old.clone();
        new.hotkey.shortcut = "Ctrl+Shift+D".into();
        new.general.autostart = true;
        let mut fx = Recorder {
            taken: vec!["Ctrl+Shift+D".into()],
            ..Default::default()
        };
        let err = apply_changes(&mut fx, Some(&old), &new).unwrap_err();
        assert!(err.contains("ya está en uso"));
        assert!(fx.calls.is_empty());
    }

    #[test]
    fn history_and_autostart_changes_propagate() {
        let old = AppSettings::default();
        let mut new = old.clone();
        new.privacy.enabled = false;
        new.general.autostart = true;
        new.hotkey.mode = HotkeyMode::Toggle;
        let mut fx = Recorder::default();
        apply_changes(&mut fx, Some(&old), &new).unwrap();
        assert_eq!(
            fx.calls,
            ["mode:Toggle", "history:false:30", "autostart:true"]
        );
    }
}
