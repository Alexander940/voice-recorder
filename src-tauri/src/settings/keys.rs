//! API keys en el almacén seguro del SO. Las keys entran desde el frontend pero
//! nunca vuelven a él: solo se expone `KeyStatus`.

use std::fmt;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;

pub const KEYRING_SERVICE: &str = "com.alexander940.voicerecorder";
pub const GROQ_API_BASE: &str = "https://api.groq.com/openai/v1";
pub const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Groq,
    Anthropic,
}

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::Groq, Provider::Anthropic];

    fn account(self) -> &'static str {
        match self {
            Provider::Groq => "groq-api-key",
            Provider::Anthropic => "anthropic-api-key",
        }
    }

    /// Variable de entorno de respaldo (desarrollo y tests en vivo).
    pub fn env_var(self) -> &'static str {
        match self {
            Provider::Groq => "GROQ_API_KEY",
            Provider::Anthropic => "ANTHROPIC_API_KEY",
        }
    }

    pub fn prefix(self) -> &'static str {
        match self {
            Provider::Groq => "gsk_",
            Provider::Anthropic => "sk-ant-",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Provider::Groq => "Groq",
            Provider::Anthropic => "Anthropic",
        }
    }
}

/// Las keys reales son mucho más largas: algo más corto es un error al copiarla.
const MIN_KEY_LEN: usize = 40;

/// Pista para reconocer una key guardada: prefijo y últimos 4 caracteres.
/// Es todo lo que el frontend llega a ver de una key.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct KeyHints {
    pub groq: Option<String>,
    pub anthropic: Option<String>,
}

pub fn mask(provider: Provider, key: &str) -> String {
    let tail: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}••••{tail}", provider.prefix())
}

/// Lo único que el frontend sabe de las keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct KeyStatus {
    pub groq: bool,
    pub anthropic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    /// No tiene el prefijo esperado (p. ej. una key de xAI en lugar de Groq).
    WrongFormat(Provider),
    /// El proveedor la rechazó.
    Rejected(Provider),
    Network(Provider),
    Storage(String),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::WrongFormat(p) => write!(
                f,
                "No parece una key de {}: debe empezar por «{}»",
                p.name(),
                p.prefix()
            ),
            KeyError::Rejected(p) => write!(f, "{} rechazó la key", p.name()),
            KeyError::Network(p) => write!(f, "No se pudo contactar con {} para validar", p.name()),
            KeyError::Storage(e) => write!(f, "No se pudo guardar en el almacén seguro: {e}"),
        }
    }
}

impl std::error::Error for KeyError {}

pub trait SecretStore: Send + Sync {
    fn get(&self, provider: Provider) -> Option<String>;
    fn set(&self, provider: Provider, key: &str) -> Result<(), KeyError>;
    fn delete(&self, provider: Provider) -> Result<(), KeyError>;
}

/// Administrador de credenciales de Windows, Llavero de macOS o Secret Service.
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, provider: Provider) -> Result<keyring::Entry, KeyError> {
        keyring::Entry::new(&self.service, provider.account())
            .map_err(|e| KeyError::Storage(e.to_string()))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, provider: Provider) -> Option<String> {
        self.entry(provider).ok()?.get_password().ok()
    }

    fn set(&self, provider: Provider, key: &str) -> Result<(), KeyError> {
        self.entry(provider)?
            .set_password(key)
            .map_err(|e| KeyError::Storage(e.to_string()))
    }

    fn delete(&self, provider: Provider) -> Result<(), KeyError> {
        match self.entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(KeyError::Storage(e.to_string())),
        }
    }
}

pub trait KeyValidator: Send + Sync {
    fn validate(&self, provider: Provider, key: &str) -> Result<(), KeyError>;
}

/// Valida con una llamada ligera y gratuita: listar modelos.
pub struct HttpValidator {
    groq_base: String,
    anthropic_base: String,
    client: Client,
}

impl HttpValidator {
    pub fn new() -> Self {
        Self::with_bases(GROQ_API_BASE, ANTHROPIC_API_BASE)
    }

    pub fn with_bases(groq_base: impl Into<String>, anthropic_base: impl Into<String>) -> Self {
        Self {
            groq_base: groq_base.into(),
            anthropic_base: anthropic_base.into(),
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("cliente HTTP"),
        }
    }
}

impl Default for HttpValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyValidator for HttpValidator {
    fn validate(&self, provider: Provider, key: &str) -> Result<(), KeyError> {
        let request = match provider {
            Provider::Groq => self
                .client
                .get(format!("{}/models", self.groq_base))
                .bearer_auth(key),
            Provider::Anthropic => self
                .client
                .get(format!("{}/models", self.anthropic_base))
                .header("x-api-key", key)
                .header("anthropic-version", ANTHROPIC_VERSION),
        };
        let status = request
            .send()
            .map_err(|_| KeyError::Network(provider))?
            .status();
        tracing::info!(
            provider = provider.name(),
            status = status.as_u16(),
            "validación de key"
        );
        match status {
            s if s.is_success() => Ok(()),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(KeyError::Rejected(provider)),
            _ => Err(KeyError::Network(provider)),
        }
    }
}

pub struct KeyManager<S, V> {
    store: S,
    validator: V,
}

impl<S: SecretStore, V: KeyValidator> KeyManager<S, V> {
    pub fn new(store: S, validator: V) -> Self {
        Self { store, validator }
    }

    /// Key guardada, o la variable de entorno como respaldo.
    pub fn get(&self, provider: Provider) -> Option<String> {
        self.store
            .get(provider)
            .or_else(|| std::env::var(provider.env_var()).ok())
            .filter(|k| !k.trim().is_empty())
    }

    pub fn hints(&self) -> KeyHints {
        let hint = |p| self.get(p).map(|k| mask(p, k.trim()));
        KeyHints {
            groq: hint(Provider::Groq),
            anthropic: hint(Provider::Anthropic),
        }
    }

    pub fn status(&self) -> KeyStatus {
        KeyStatus {
            groq: self.get(Provider::Groq).is_some(),
            anthropic: self.get(Provider::Anthropic).is_some(),
        }
    }

    /// Comprueba formato, valida con el proveedor y solo entonces guarda.
    pub fn save(&self, provider: Provider, key: &str) -> Result<KeyStatus, KeyError> {
        let key = key.trim();
        if !key.starts_with(provider.prefix()) || key.len() < MIN_KEY_LEN {
            return Err(KeyError::WrongFormat(provider));
        }
        self.validator.validate(provider, key)?;
        self.store.set(provider, key)?;
        Ok(self.status())
    }

    pub fn delete(&self, provider: Provider) -> Result<KeyStatus, KeyError> {
        self.store.delete(provider)?;
        Ok(self.status())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tracing_test::traced_test;

    pub const FAKE_GROQ: &str = "gsk_FAKEtestkey0123456789abcdefghijklmnopqrstuvwxyzABCD";
    pub const FAKE_ANTHROPIC: &str = "sk-ant-api03-FAKEtestkey0123456789abcdefghij";

    #[derive(Default)]
    pub struct MemoryStore(Mutex<HashMap<Provider, String>>);

    impl SecretStore for MemoryStore {
        fn get(&self, p: Provider) -> Option<String> {
            self.0.lock().unwrap().get(&p).cloned()
        }
        fn set(&self, p: Provider, k: &str) -> Result<(), KeyError> {
            self.0.lock().unwrap().insert(p, k.into());
            Ok(())
        }
        fn delete(&self, p: Provider) -> Result<(), KeyError> {
            self.0.lock().unwrap().remove(&p);
            Ok(())
        }
    }

    struct AcceptAll;
    impl KeyValidator for AcceptAll {
        fn validate(&self, _: Provider, _: &str) -> Result<(), KeyError> {
            Ok(())
        }
    }

    struct RejectAll;
    impl KeyValidator for RejectAll {
        fn validate(&self, p: Provider, _: &str) -> Result<(), KeyError> {
            Err(KeyError::Rejected(p))
        }
    }

    // Los tests de estado no deben depender de las variables del entorno real.
    fn without_env() {
        for p in Provider::ALL {
            // SAFETY: los tests que tocan el entorno solo lo borran, nunca lo leen en paralelo con valores distintos.
            unsafe { std::env::remove_var(p.env_var()) };
        }
    }

    #[test]
    fn save_valid_key_stores_it_and_reports_status() {
        without_env();
        let m = KeyManager::new(MemoryStore::default(), AcceptAll);
        assert_eq!(m.status(), KeyStatus::default());
        let s = m.save(Provider::Groq, &format!("  {FAKE_GROQ}\n")).unwrap();
        assert_eq!(
            s,
            KeyStatus {
                groq: true,
                anthropic: false
            }
        );
        assert_eq!(m.get(Provider::Groq).as_deref(), Some(FAKE_GROQ));
    }

    #[test]
    fn hints_show_only_prefix_and_last_four() {
        without_env();
        let m = KeyManager::new(MemoryStore::default(), AcceptAll);
        assert_eq!(m.hints(), KeyHints::default());
        m.save(Provider::Groq, FAKE_GROQ).unwrap();
        let hint = m.hints().groq.unwrap();
        assert_eq!(hint, "gsk_••••ABCD");
        assert!(!hint.contains("FAKE"));
        assert_eq!(m.hints().anthropic, None);
    }

    #[test]
    fn too_short_key_is_a_format_error() {
        let m = KeyManager::new(MemoryStore::default(), RejectAll);
        let err = m
            .save(Provider::Groq, "gsk_xxxxxxxxxxxxxxxxxxxx")
            .unwrap_err();
        assert_eq!(err, KeyError::WrongFormat(Provider::Groq));
    }

    #[test]
    fn wrong_prefix_is_rejected_before_calling_provider() {
        let m = KeyManager::new(MemoryStore::default(), RejectAll);
        let err = m.save(Provider::Groq, "xai-something").unwrap_err();
        assert_eq!(err, KeyError::WrongFormat(Provider::Groq));
        assert!(err.to_string().contains("gsk_"));
    }

    #[test]
    fn rejected_key_is_not_stored() {
        without_env();
        let m = KeyManager::new(MemoryStore::default(), RejectAll);
        assert_eq!(
            m.save(Provider::Anthropic, FAKE_ANTHROPIC).unwrap_err(),
            KeyError::Rejected(Provider::Anthropic)
        );
        assert!(m.store.get(Provider::Anthropic).is_none());
    }

    #[test]
    fn delete_removes_key() {
        without_env();
        let m = KeyManager::new(MemoryStore::default(), AcceptAll);
        m.save(Provider::Anthropic, FAKE_ANTHROPIC).unwrap();
        assert!(!m.delete(Provider::Anthropic).unwrap().anthropic);
    }

    #[test]
    fn key_status_serializes_only_booleans() {
        let json = serde_json::to_value(KeyStatus {
            groq: true,
            anthropic: false,
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({"groq": true, "anthropic": false}));
    }

    fn validator(server: &mockito::Server) -> HttpValidator {
        HttpValidator::with_bases(server.url(), server.url())
    }

    #[test]
    #[traced_test]
    fn http_validator_groq_valid_and_invalid() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/models")
            .match_header("authorization", format!("Bearer {FAKE_GROQ}").as_str())
            .with_status(200)
            .with_body(r#"{"data":[]}"#)
            .create();
        server.mock("GET", "/models").with_status(401).create();
        let v = validator(&server);
        assert!(v.validate(Provider::Groq, FAKE_GROQ).is_ok());
        assert_eq!(
            v.validate(Provider::Groq, "gsk_wrong"),
            Err(KeyError::Rejected(Provider::Groq))
        );
        assert!(!logs_contain("gsk_"));
    }

    #[test]
    #[traced_test]
    fn http_validator_anthropic_sends_version_and_key_headers() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/models")
            .match_header("x-api-key", FAKE_ANTHROPIC)
            .match_header("anthropic-version", ANTHROPIC_VERSION)
            .with_status(200)
            .with_body(r#"{"data":[]}"#)
            .create();
        server.mock("GET", "/models").with_status(401).create();
        let v = validator(&server);
        assert!(v.validate(Provider::Anthropic, FAKE_ANTHROPIC).is_ok());
        assert_eq!(
            v.validate(Provider::Anthropic, "sk-ant-wrong"),
            Err(KeyError::Rejected(Provider::Anthropic))
        );
        assert!(!logs_contain("sk-ant-"));
    }

    #[test]
    fn http_validator_unreachable_is_network_error() {
        let v = HttpValidator::with_bases("http://127.0.0.1:9", "http://127.0.0.1:9");
        assert_eq!(
            v.validate(Provider::Groq, FAKE_GROQ),
            Err(KeyError::Network(Provider::Groq))
        );
    }

    /// Ida y vuelta con el Administrador de credenciales real de Windows,
    /// bajo un servicio de pruebas que se borra al terminar.
    #[cfg(windows)]
    #[test]
    fn keyring_roundtrip_windows_credential_manager() {
        let store = KeyringStore::new(format!("{KEYRING_SERVICE}.test"));
        store.delete(Provider::Groq).unwrap();
        assert_eq!(store.get(Provider::Groq), None);
        store.set(Provider::Groq, FAKE_GROQ).unwrap();
        assert_eq!(store.get(Provider::Groq).as_deref(), Some(FAKE_GROQ));
        store.delete(Provider::Groq).unwrap();
        assert_eq!(store.get(Provider::Groq), None);
    }
}
