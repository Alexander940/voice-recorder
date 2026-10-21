//! Errores de la app y su notificación al usuario (HU-13).

use crate::audio::AudioError;
use crate::output::OutputError;
use crate::settings::keys::Provider;
use crate::stt::SttError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

impl Lang {
    /// Español si el sistema está en español; inglés en otro caso.
    pub fn from_locale(locale: Option<&str>) -> Self {
        match locale {
            Some(l) if l.to_lowercase().starts_with("es") => Lang::Es,
            Some(_) => Lang::En,
            None => Lang::Es,
        }
    }

    pub fn system() -> Self {
        Self::from_locale(sys_locale::get_locale().as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    NoNetwork,
    MissingKey(Provider),
    InvalidKey(Provider),
    RateLimited(Provider),
    NoMicrophone,
    /// Fallo de transcripción no clasificado (servidor, respuesta inválida…).
    Transcription(String),
    Paste(String),
}

impl From<SttError> for AppError {
    fn from(e: SttError) -> Self {
        match e {
            SttError::Auth => AppError::InvalidKey(Provider::Groq),
            SttError::RateLimited => AppError::RateLimited(Provider::Groq),
            SttError::Network(_) => AppError::NoNetwork,
            other => AppError::Transcription(other.to_string()),
        }
    }
}

impl From<AudioError> for AppError {
    fn from(_: AudioError) -> Self {
        AppError::NoMicrophone
    }
}

impl From<OutputError> for AppError {
    fn from(e: OutputError) -> Self {
        AppError::Paste(e.0)
    }
}

impl AppError {
    /// Si el audio merece conservarse para reintentar.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            AppError::NoNetwork
                | AppError::RateLimited(_)
                | AppError::Transcription(_)
                | AppError::InvalidKey(_)
                | AppError::MissingKey(_)
        )
    }

    /// Título y texto de la notificación nativa.
    pub fn notification(&self, lang: Lang) -> (String, String) {
        use AppError::*;
        let (es, en): ((&str, String), (&str, String)) = match self {
            NoNetwork => (
                ("Sin conexión", "No se pudo contactar con el servicio. Tu dictado se guardó: reinténtalo desde el Historial.".into()),
                ("No connection", "Couldn't reach the service. Your dictation was kept: retry it from History.".into()),
            ),
            MissingKey(p) => (
                ("Falta una API key", format!("Configura la key de {} en Ajustes → Cuentas.", p.name())),
                ("API key missing", format!("Set your {} key in Settings → Accounts.", p.name())),
            ),
            InvalidKey(p) => (
                ("API key no válida", format!("{} rechazó la key. Revísala en Ajustes → Cuentas.", p.name())),
                ("Invalid API key", format!("{} rejected the key. Check it in Settings → Accounts.", p.name())),
            ),
            RateLimited(p) => (
                ("Demasiadas peticiones", format!("{} pide esperar unos segundos. Reinténtalo desde el Historial.", p.name())),
                ("Too many requests", format!("{} asks to wait a few seconds. Retry from History.", p.name())),
            ),
            NoMicrophone => (
                ("Sin micrófono", "No se encontró un micrófono disponible. Revisa Ajustes → Audio.".into()),
                ("No microphone", "No microphone available. Check Settings → Audio.".into()),
            ),
            Transcription(m) => (
                ("Error al transcribir", format!("{m}. Reinténtalo desde el Historial.")),
                ("Transcription failed", format!("{m}. Retry from History.")),
            ),
            Paste(m) => (
                ("No se pudo pegar", format!("El texto está en el portapapeles. ({m})")),
                ("Couldn't paste", format!("The text is on your clipboard. ({m})")),
            ),
        };
        let (title, body) = match lang {
            Lang::Es => es,
            Lang::En => en,
        };
        (title.to_string(), body)
    }
}

impl AppError {
    /// Título corto y detalle para el overlay (una sola línea cada uno).
    pub fn overlay(&self, lang: Lang) -> (String, String) {
        use AppError::*;
        let es = lang == Lang::Es;
        let title = match self {
            NoNetwork if es => "Sin conexión con Groq".to_string(),
            NoNetwork => "No connection to Groq".to_string(),
            MissingKey(p) if es => format!("Falta la key de {}", p.name()),
            MissingKey(p) => format!("{} key missing", p.name()),
            InvalidKey(p) if es => format!("{} rechazó la key", p.name()),
            InvalidKey(p) => format!("{} rejected the key", p.name()),
            RateLimited(p) if es => format!("Demasiadas peticiones a {}", p.name()),
            RateLimited(p) => format!("Too many requests to {}", p.name()),
            NoMicrophone if es => "Sin micrófono".to_string(),
            NoMicrophone => "No microphone".to_string(),
            Transcription(_) if es => "Error al transcribir".to_string(),
            Transcription(_) => "Transcription failed".to_string(),
            Paste(_) if es => "No se pudo pegar".to_string(),
            Paste(_) => "Couldn't paste".to_string(),
        };
        let detail = match (self, es) {
            (Paste(_), true) => "El texto está en el portapapeles",
            (Paste(_), false) => "The text is on your clipboard",
            (e, true) if e.retryable() => "El audio sigue guardado",
            (e, false) if e.retryable() => "Your audio was kept",
            _ => "",
        };
        (title, detail.to_string())
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (title, body) = self.notification(Lang::Es);
        write!(f, "{title}: {body}")
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<AppError> {
        vec![
            AppError::NoNetwork,
            AppError::MissingKey(Provider::Groq),
            AppError::InvalidKey(Provider::Anthropic),
            AppError::RateLimited(Provider::Groq),
            AppError::NoMicrophone,
            AppError::Transcription("Groq no está disponible (HTTP 503)".into()),
            AppError::Paste("acceso denegado".into()),
        ]
    }

    #[test]
    fn every_variant_has_distinct_es_and_en_notifications() {
        let es: Vec<_> = all().iter().map(|e| e.notification(Lang::Es)).collect();
        let en: Vec<_> = all().iter().map(|e| e.notification(Lang::En)).collect();
        for ((t_es, b_es), (t_en, b_en)) in es.iter().zip(&en) {
            assert!(!t_es.is_empty() && !b_es.is_empty() && !t_en.is_empty() && !b_en.is_empty());
            assert_ne!(t_es, t_en, "falta traducir {t_es}");
        }
        let titles: std::collections::HashSet<_> = es.iter().map(|(t, _)| t).collect();
        assert_eq!(titles.len(), es.len(), "títulos repetidos");
    }

    #[test]
    fn overlay_texts_are_short_and_mention_the_kept_audio() {
        let (t, d) = AppError::NoNetwork.overlay(Lang::Es);
        assert_eq!(t, "Sin conexión con Groq");
        assert_eq!(d, "El audio sigue guardado");
        let (t, d) = AppError::NoMicrophone.overlay(Lang::En);
        assert_eq!(t, "No microphone");
        assert_eq!(d, "");
        for e in all() {
            for lang in [Lang::Es, Lang::En] {
                let (t, d) = e.overlay(lang);
                assert!(t.chars().count() <= 32, "{t}");
                assert!(d.chars().count() <= 34, "{d}");
            }
        }
    }

    #[test]
    fn notification_table() {
        let cases = [
            (AppError::NoNetwork, "Sin conexión", "Historial"),
            (
                AppError::InvalidKey(Provider::Groq),
                "API key no válida",
                "Groq",
            ),
            (
                AppError::RateLimited(Provider::Groq),
                "Demasiadas peticiones",
                "Groq",
            ),
            (AppError::NoMicrophone, "Sin micrófono", "Audio"),
            (
                AppError::MissingKey(Provider::Anthropic),
                "Falta una API key",
                "Anthropic",
            ),
            (
                AppError::Paste("x".into()),
                "No se pudo pegar",
                "portapapeles",
            ),
        ];
        for (err, title, body_has) in cases {
            let (t, b) = err.notification(Lang::Es);
            assert_eq!(t, title);
            assert!(b.contains(body_has), "{b}");
        }
        assert_eq!(
            AppError::NoMicrophone.notification(Lang::En).0,
            "No microphone"
        );
    }

    #[test]
    fn module_errors_map_to_app_errors() {
        assert_eq!(
            AppError::from(SttError::Auth),
            AppError::InvalidKey(Provider::Groq)
        );
        assert_eq!(
            AppError::from(SttError::RateLimited),
            AppError::RateLimited(Provider::Groq)
        );
        assert_eq!(
            AppError::from(SttError::Network("x".into())),
            AppError::NoNetwork
        );
        assert!(matches!(
            AppError::from(SttError::Server(503)),
            AppError::Transcription(_)
        ));
        assert_eq!(
            AppError::from(AudioError("sin dispositivo".into())),
            AppError::NoMicrophone
        );
    }

    #[test]
    fn only_service_failures_keep_audio_for_retry() {
        assert!(AppError::NoNetwork.retryable());
        assert!(AppError::RateLimited(Provider::Groq).retryable());
        assert!(!AppError::NoMicrophone.retryable());
        assert!(!AppError::Paste("x".into()).retryable());
    }

    #[test]
    fn language_from_locale() {
        assert_eq!(Lang::from_locale(Some("es-CO")), Lang::Es);
        assert_eq!(Lang::from_locale(Some("en-US")), Lang::En);
        assert_eq!(Lang::from_locale(None), Lang::Es);
    }
}
