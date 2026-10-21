//! Post-procesado del texto con Claude (HU-08, HU-09, HU-10).

mod anthropic;

pub use anthropic::{AnthropicCleaner, CLEANUP_MODEL, DEFAULT_SYSTEM_PROMPT, cleanup_timeout};

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    /// El modelo declinó la petición (`stop_reason: "refusal"`).
    Refusal,
    /// Se alcanzó `max_tokens` antes de terminar.
    Truncated,
    Timeout,
    Auth,
    Http(u16),
    Network(String),
    InvalidResponse(String),
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmError::Refusal => f.write_str("Claude declinó procesar el texto"),
            LlmError::Truncated => f.write_str("La respuesta de Claude quedó incompleta"),
            LlmError::Timeout => f.write_str("Claude tardó demasiado"),
            LlmError::Auth => f.write_str("La API key de Anthropic no es válida"),
            LlmError::Http(s) => write!(f, "Anthropic respondió HTTP {s}"),
            LlmError::Network(m) => write!(f, "Sin conexión con Anthropic: {m}"),
            LlmError::InvalidResponse(m) => write!(f, "Respuesta inesperada de Anthropic: {m}"),
        }
    }
}

impl std::error::Error for LlmError {}

/// Tokens facturados de una llamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Usage {
    /// Coste en USD con la tarifa de Haiku 5.5 para prompts ≤ 100K tokens.
    pub fn cost_usd(&self) -> f64 {
        self.input_tokens as f64 * 0.10 / 1e6 + self.output_tokens as f64 * 0.50 / 1e6
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cleaned {
    pub text: String,
    pub latency: Duration,
    pub usage: Option<Usage>,
}

pub trait TextCleaner: Send + Sync {
    /// Limpia `raw` siguiendo `instructions` (el prompt del modo activo).
    fn clean(&self, raw: &str, instructions: &str) -> Result<Cleaned, LlmError>;
}

/// Resultado de la etapa de limpieza dentro del pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum CleanupOutcome {
    Cleaned(String),
    /// Se entrega el texto de Whisper: limpieza desactivada, sin key o error.
    Raw {
        text: String,
        reason: Option<LlmError>,
    },
}

impl CleanupOutcome {
    pub fn text(&self) -> &str {
        match self {
            CleanupOutcome::Cleaned(t) => t,
            CleanupOutcome::Raw { text, .. } => text,
        }
    }
}

/// Aplica la limpieza si hay limpiador; ante cualquier error devuelve el texto bruto.
pub fn clean_or_raw(
    cleaner: Option<&dyn TextCleaner>,
    raw: &str,
    instructions: &str,
) -> CleanupOutcome {
    let Some(cleaner) = cleaner else {
        return CleanupOutcome::Raw {
            text: raw.to_string(),
            reason: None,
        };
    };
    match cleaner.clean(raw, instructions) {
        Ok(c) if !c.text.trim().is_empty() => CleanupOutcome::Cleaned(c.text),
        Ok(_) => CleanupOutcome::Raw {
            text: raw.to_string(),
            reason: Some(LlmError::InvalidResponse("respuesta vacía".into())),
        },
        Err(e) => {
            tracing::warn!(error = %e, "limpieza fallida, se usa el texto bruto");
            CleanupOutcome::Raw {
                text: raw.to_string(),
                reason: Some(e),
            }
        }
    }
}

/// Interruptor de la limpieza; su persistencia llega con los ajustes (HU-12).
pub struct CleanupSettings {
    enabled: AtomicBool,
}

impl Default for CleanupSettings {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(true),
        }
    }
}

impl CleanupSettings {
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn set_cleanup_enabled(settings: tauri::State<'_, CleanupSettings>, enabled: bool) {
    settings.set_enabled(enabled);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Result<&'static str, LlmError>);
    impl TextCleaner for Fixed {
        fn clean(&self, _: &str, _: &str) -> Result<Cleaned, LlmError> {
            self.0.clone().map(|t| Cleaned {
                text: t.into(),
                latency: Duration::ZERO,
                usage: None,
            })
        }
    }

    #[test]
    fn without_cleaner_returns_raw_without_reason() {
        let out = clean_or_raw(None, "eh hola", "x");
        assert_eq!(
            out,
            CleanupOutcome::Raw {
                text: "eh hola".into(),
                reason: None
            }
        );
    }

    #[test]
    fn success_returns_cleaned_text() {
        let out = clean_or_raw(Some(&Fixed(Ok("Hola."))), "eh hola", "x");
        assert_eq!(out, CleanupOutcome::Cleaned("Hola.".into()));
        assert_eq!(out.text(), "Hola.");
    }

    #[test]
    fn every_error_falls_back_to_raw() {
        for e in [
            LlmError::Refusal,
            LlmError::Truncated,
            LlmError::Timeout,
            LlmError::Auth,
            LlmError::Http(529),
        ] {
            let out = clean_or_raw(Some(&Fixed(Err(e.clone()))), "texto bruto", "x");
            assert_eq!(out.text(), "texto bruto");
            assert!(matches!(out, CleanupOutcome::Raw { reason: Some(r), .. } if r == e));
        }
    }

    #[test]
    fn empty_answer_falls_back_to_raw() {
        let out = clean_or_raw(Some(&Fixed(Ok("   "))), "texto bruto", "x");
        assert_eq!(out.text(), "texto bruto");
    }

    #[test]
    fn cleanup_is_enabled_by_default_and_can_be_disabled() {
        let s = CleanupSettings::default();
        assert!(s.enabled());
        s.set_enabled(false);
        assert!(!s.enabled());
    }
}
