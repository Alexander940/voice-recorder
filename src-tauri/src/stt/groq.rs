//! Cliente de Groq Whisper (`/openai/v1/audio/transcriptions`).

use std::fmt;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use reqwest::blocking::{Client, multipart};

use super::{SttError, TranscribeOptions, Transcriber, Transcript};

pub const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
pub const DEFAULT_MODEL: &str = "whisper-large-v3-turbo";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// Un único reintento ante 5xx o errores de red.
const RETRIES: u32 = 1;

pub struct GroqTranscriber {
    base_url: String,
    api_key: String,
    client: Client,
    timeout: Duration,
}

/// Subida mínima supuesta (≈ 0,8 Mbit/s): un trozo de 19 MB tiene unos 3 min.
const UPLOAD_BYTES_PER_SEC: u64 = 100_000;

/// Tiempo máximo de una petición: base más lo que tarda en subir el audio.
pub fn request_timeout(base: Duration, wav_len: usize) -> Duration {
    base + Duration::from_secs(wav_len as u64 / UPLOAD_BYTES_PER_SEC)
}

/// Segundos de `Retry-After` (Groq usa segundos, no fechas).
fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|s| s.is_finite() && *s >= 0.0)
        .map(Duration::from_secs_f64)
}

// La key nunca debe aparecer en logs ni en mensajes de depuración.
impl fmt::Debug for GroqTranscriber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GroqTranscriber")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

#[derive(serde::Deserialize)]
struct TranscriptionResponse {
    text: String,
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(serde::Deserialize)]
struct ErrorDetail {
    message: String,
}

enum Attempt {
    Done(Result<String, SttError>),
    Retry(SttError),
}

impl GroqTranscriber {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(GROQ_BASE_URL, api_key, DEFAULT_TIMEOUT)
    }

    pub fn with_base_url(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        timeout: Duration,
    ) -> Self {
        let client = Client::builder().build().expect("cliente HTTP");
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client,
            timeout,
        }
    }

    fn form(wav: &[u8], options: &TranscribeOptions) -> multipart::Form {
        let file = multipart::Part::bytes(wav.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .expect("mime válido");
        let mut form = multipart::Form::new()
            .part("file", file)
            .text("model", options.model.clone())
            .text("response_format", "json")
            .text("temperature", "0");
        if let Some(lang) = &options.language {
            form = form.text("language", lang.clone());
        }
        if let Some(prompt) = &options.prompt {
            form = form.text("prompt", prompt.clone());
        }
        form
    }

    fn attempt(&self, wav: &[u8], options: &TranscribeOptions) -> Attempt {
        let response = self
            .client
            .post(format!("{}/audio/transcriptions", self.base_url))
            .bearer_auth(&self.api_key)
            .timeout(request_timeout(self.timeout, wav.len()))
            .multipart(Self::form(wav, options))
            .send();
        let response = match response {
            Ok(r) => r,
            Err(e) => return Attempt::Retry(SttError::Network(network_reason(&e))),
        };
        let status = response.status();
        if status.is_success() {
            return Attempt::Done(
                response
                    .json::<TranscriptionResponse>()
                    .map(|r| r.text.trim().to_string())
                    .map_err(|e| SttError::InvalidResponse(e.to_string())),
            );
        }
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Attempt::Done(Err(SttError::Auth)),
            StatusCode::TOO_MANY_REQUESTS => {
                Attempt::Done(Err(SttError::RateLimited(retry_after(response.headers()))))
            }
            s if s.is_server_error() => Attempt::Retry(SttError::Server(s.as_u16())),
            s => {
                let message = response
                    .json::<ErrorBody>()
                    .map(|b| b.error.message)
                    .unwrap_or_else(|_| format!("HTTP {}", s.as_u16()));
                Attempt::Done(Err(SttError::BadRequest(message)))
            }
        }
    }
}

/// Describe el error de red sin incluir la URL ni cabeceras.
fn network_reason(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "tiempo de espera agotado".into()
    } else if e.is_connect() {
        "no se pudo conectar".into()
    } else {
        "error de red".into()
    }
}

impl Transcriber for GroqTranscriber {
    fn transcribe(&self, wav: &[u8], options: &TranscribeOptions) -> Result<Transcript, SttError> {
        let started = Instant::now();
        let mut attempts = 0;
        let result = loop {
            attempts += 1;
            match self.attempt(wav, options) {
                Attempt::Done(r) => break r,
                Attempt::Retry(e) if attempts > RETRIES => break Err(e),
                Attempt::Retry(e) => {
                    tracing::warn!(error = %e, "transcripción fallida, reintentando");
                }
            }
        };
        let latency = started.elapsed();
        match &result {
            Ok(text) => tracing::info!(
                latency_ms = latency.as_millis() as u64,
                model = %options.model,
                attempts,
                chars = text.len(),
                "transcripción completada"
            ),
            Err(e) => tracing::warn!(
                latency_ms = latency.as_millis() as u64,
                attempts,
                error = %e,
                "transcripción fallida"
            ),
        }
        result.map(|text| Transcript { text, latency })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;
    use tracing_test::traced_test;

    const KEY: &str = "gsk_test_secret_key_123";
    const WAV: &[u8] = b"RIFF....WAVEfmt fake";

    fn client(server: &mockito::Server) -> GroqTranscriber {
        GroqTranscriber::with_base_url(server.url(), KEY, Duration::from_millis(500))
    }

    fn es() -> TranscribeOptions {
        TranscribeOptions {
            language: Some("es".into()),
            ..Default::default()
        }
    }

    #[test]
    fn sends_multipart_with_file_model_language_and_auth() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/audio/transcriptions")
            .match_header("authorization", format!("Bearer {KEY}").as_str())
            .match_header(
                "content-type",
                Matcher::Regex("multipart/form-data; boundary=.*".into()),
            )
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex(r#"name="file"; filename="audio.wav""#.into()),
                Matcher::Regex(r#"name="model"\r\n\r\nwhisper-large-v3-turbo"#.into()),
                Matcher::Regex(r#"name="language"\r\n\r\nes"#.into()),
                Matcher::Regex("RIFF....WAVEfmt fake".into()),
            ]))
            .with_status(200)
            .with_body(r#"{"text":"  hola mundo  "}"#)
            .create();
        let t = client(&server).transcribe(WAV, &es()).unwrap();
        assert_eq!(t.text, "hola mundo");
        m.assert();
    }

    #[test]
    fn language_is_omitted_for_auto_detection() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/audio/transcriptions")
            .match_body(Matcher::Regex(r#"name="language""#.into()))
            .expect(0)
            .create();
        let ok = server
            .mock("POST", "/audio/transcriptions")
            .with_body(r#"{"text":"ok"}"#)
            .create();
        client(&server)
            .transcribe(WAV, &TranscribeOptions::default())
            .unwrap();
        m.assert();
        ok.assert();
    }

    /// Servidor HTTP mínimo que responde, en orden, las respuestas dadas.
    fn scripted_server(responses: Vec<(u16, &'static str)>) -> String {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                    if line.trim_end().is_empty() {
                        break;
                    }
                }
                let mut body_in = vec![0; len];
                reader.read_exact(&mut body_in).unwrap();
                let crlf = "\r\n";
                let head = [
                    format!("HTTP/1.1 {status} X"),
                    "Content-Type: application/json".into(),
                    format!("Content-Length: {}", body.len()),
                    "Connection: close".into(),
                    String::new(),
                ]
                .join(crlf);
                write!(stream, "{head}{crlf}{body}").unwrap();
            }
        });
        url
    }

    #[test]
    fn server_error_is_retried_once_then_succeeds() {
        let url = scripted_server(vec![(503, "{}"), (200, r#"{"text":"segundo intento"}"#)]);
        let t = GroqTranscriber::with_base_url(url, KEY, Duration::from_secs(2));
        assert_eq!(t.transcribe(WAV, &es()).unwrap().text, "segundo intento");
    }

    #[test]
    fn persistent_server_error_gives_up_after_one_retry() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(500)
            .expect(2)
            .create();
        assert_eq!(
            client(&server).transcribe(WAV, &es()).unwrap_err(),
            SttError::Server(500)
        );
        m.assert();
    }

    #[test]
    fn unauthorized_maps_to_auth_without_retry() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(401)
            .expect(1)
            .create();
        let err = client(&server).transcribe(WAV, &es()).unwrap_err();
        assert_eq!(err, SttError::Auth);
        assert!(err.to_string().contains("API key"));
        m.assert();
    }

    #[test]
    fn rate_limit_maps_to_rate_limited_without_retry() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(429)
            .with_header("retry-after", "12")
            .expect(1)
            .create();
        assert_eq!(
            client(&server).transcribe(WAV, &es()).unwrap_err(),
            SttError::RateLimited(Some(Duration::from_secs(12)))
        );
        m.assert();
    }

    #[test]
    fn rate_limit_without_hint_has_no_wait() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/audio/transcriptions")
            .with_status(429)
            .create();
        assert_eq!(
            client(&server).transcribe(WAV, &es()).unwrap_err(),
            SttError::RateLimited(None)
        );
    }

    #[test]
    fn timeout_grows_with_the_audio_size() {
        let base = Duration::from_secs(30);
        assert_eq!(request_timeout(base, 0), base);
        // Un trozo de 10 min (≈ 19 MB) tiene unos 3 minutos más para subir.
        let chunk = request_timeout(base, 19_360_000);
        assert!(chunk >= Duration::from_secs(30 + 190), "{chunk:?}");
    }

    #[test]
    fn bad_request_surfaces_api_message() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/audio/transcriptions")
            .with_status(400)
            .with_body(r#"{"error":{"message":"file is too short"}}"#)
            .create();
        assert_eq!(
            client(&server).transcribe(WAV, &es()).unwrap_err(),
            SttError::BadRequest("file is too short".into())
        );
    }

    #[test]
    fn timeout_is_retried_then_reported_as_network() {
        // Puerto sin servidor: falla al conectar en ambos intentos.
        let t =
            GroqTranscriber::with_base_url("http://127.0.0.1:9", KEY, Duration::from_millis(300));
        assert!(matches!(
            t.transcribe(WAV, &es()).unwrap_err(),
            SttError::Network(_)
        ));
    }

    #[test]
    #[traced_test]
    fn logs_latency_and_never_the_api_key() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/audio/transcriptions")
            .with_body(r#"{"text":"hola"}"#)
            .create();
        let t = client(&server);
        t.transcribe(WAV, &es()).unwrap();
        server.reset();
        server
            .mock("POST", "/audio/transcriptions")
            .with_status(401)
            .create();
        let _ = t.transcribe(WAV, &es());
        assert!(logs_contain("latency_ms"));
        assert!(!logs_contain(KEY));
        assert!(!format!("{t:?}").contains(KEY));
    }
}
