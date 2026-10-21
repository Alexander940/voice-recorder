//! Voz a texto: trait `Transcriber` e implementación Groq Whisper (HU-05).

mod groq;
mod wer;

pub use groq::{DEFAULT_MODEL, GROQ_BASE_URL, GroqTranscriber};
pub use wer::{normalize_words, word_error_rate};

use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscribeOptions {
    /// `whisper-large-v3-turbo` o `whisper-large-v3`.
    pub model: String,
    /// Código ISO-639-1 (`es`, `en`…). `None` = detección automática.
    pub language: Option<String>,
    /// Vocabulario o contexto para guiar a Whisper (HU-10).
    pub prompt: Option<String>,
}

impl Default for TranscribeOptions {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.into(),
            language: None,
            prompt: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transcript {
    pub text: String,
    pub latency: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SttError {
    /// API key ausente, inválida o sin permisos (401/403).
    Auth,
    /// Límite de peticiones (429), con la espera que indique `Retry-After`.
    RateLimited(Option<Duration>),
    /// La API rechazó la petición (4xx), con su mensaje.
    BadRequest(String),
    /// Error del servidor (5xx) tras reintentar.
    Server(u16),
    /// Sin conexión o tiempo agotado tras reintentar.
    Network(String),
    /// Respuesta inesperada.
    InvalidResponse(String),
}

impl fmt::Display for SttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SttError::Auth => f.write_str("La API key de Groq no es válida o falta"),
            SttError::RateLimited(_) => {
                f.write_str("Groq: demasiadas peticiones, espera unos segundos")
            }
            SttError::BadRequest(m) => write!(f, "Groq rechazó el audio: {m}"),
            SttError::Server(s) => write!(f, "Groq no está disponible (HTTP {s})"),
            SttError::Network(m) => write!(f, "Sin conexión con Groq: {m}"),
            SttError::InvalidResponse(m) => write!(f, "Respuesta inesperada de Groq: {m}"),
        }
    }
}

impl std::error::Error for SttError {}

pub trait Transcriber: Send + Sync {
    fn transcribe(&self, wav: &[u8], options: &TranscribeOptions) -> Result<Transcript, SttError>;
}
