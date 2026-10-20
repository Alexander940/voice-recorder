//! Lógica del updater sin Tauri: manifiesto, versiones, firma y programación.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::blocking::Client;

pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    /// Falta la clave pública del updater en la configuración.
    NotConfigured,
    Network(String),
    InvalidManifest(String),
    InvalidSignature,
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::NotConfigured => f.write_str("Las actualizaciones no están configuradas"),
            UpdateError::Network(m) => {
                write!(f, "No se pudo comprobar si hay actualizaciones: {m}")
            }
            UpdateError::InvalidManifest(m) => {
                write!(f, "Manifiesto de actualización inválido: {m}")
            }
            UpdateError::InvalidSignature => {
                f.write_str("La firma de la actualización no es válida; no se instalará")
            }
        }
    }
}

impl std::error::Error for UpdateError {}

/// `latest.json` tal como lo publica tauri-action.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub platforms: HashMap<String, PlatformEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct PlatformEntry {
    pub signature: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Available {
    pub version: String,
    pub notes: Option<String>,
    #[serde(skip)]
    pub url: String,
    #[serde(skip)]
    pub signature: String,
}

/// Clave de plataforma de Tauri para el binario actual.
pub fn platform_key() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "aarch64") => "windows-aarch64",
        ("windows", _) => "windows-x86_64",
        ("macos", "aarch64") => "darwin-aarch64",
        ("macos", _) => "darwin-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        _ => "linux-x86_64",
    }
}

/// ¿Ofrece el manifiesto una versión mayor para esta plataforma?
pub fn evaluate(
    manifest: &Manifest,
    current: &str,
    platform: &str,
) -> Result<Option<Available>, UpdateError> {
    let parse = |v: &str| {
        semver::Version::parse(v.trim_start_matches('v'))
            .map_err(|e| UpdateError::InvalidManifest(format!("versión «{v}»: {e}")))
    };
    if parse(&manifest.version)? <= parse(current)? {
        return Ok(None);
    }
    Ok(manifest.platforms.get(platform).map(|p| Available {
        version: manifest.version.trim_start_matches('v').to_string(),
        notes: manifest.notes.clone(),
        url: p.url.clone(),
        signature: p.signature.clone(),
    }))
}

fn b64_text(s: &str) -> Result<String, UpdateError> {
    let bytes = STANDARD
        .decode(s.trim())
        .map_err(|_| UpdateError::InvalidSignature)?;
    String::from_utf8(bytes).map_err(|_| UpdateError::InvalidSignature)
}

/// Verifica la firma minisign del artefacto como lo hace tauri-plugin-updater:
/// clave pública y firma en base64 del texto minisign.
pub fn verify_artifact(
    data: &[u8],
    signature_b64: &str,
    pubkey_b64: &str,
) -> Result<(), UpdateError> {
    if pubkey_b64.trim().is_empty() {
        return Err(UpdateError::NotConfigured);
    }
    let pk = minisign_verify::PublicKey::decode(&b64_text(pubkey_b64)?)
        .map_err(|_| UpdateError::InvalidSignature)?;
    let sig = minisign_verify::Signature::decode(&b64_text(signature_b64)?)
        .map_err(|_| UpdateError::InvalidSignature)?;
    pk.verify(data, &sig, true)
        .map_err(|_| UpdateError::InvalidSignature)
}

/// Como mucho una comprobación por intervalo, y nunca si están desactivadas.
pub fn should_check(enabled: bool, last: Option<SystemTime>, now: SystemTime) -> bool {
    enabled && last.is_none_or(|l| now.duration_since(l).map_or(true, |d| d >= CHECK_INTERVAL))
}

pub struct UpdateChecker {
    endpoint: String,
    pubkey: String,
    client: Client,
}

impl UpdateChecker {
    pub fn new(endpoint: impl Into<String>, pubkey: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            pubkey: pubkey.into(),
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("cliente HTTP"),
        }
    }

    pub fn configured(&self) -> bool {
        !self.pubkey.trim().is_empty() && !self.endpoint.is_empty()
    }

    pub fn fetch_manifest(&self) -> Result<Manifest, UpdateError> {
        let response = self
            .client
            .get(&self.endpoint)
            .send()
            .map_err(|e| UpdateError::Network(e.to_string()))?;
        if !response.status().is_success() {
            return Err(UpdateError::Network(format!(
                "HTTP {}",
                response.status().as_u16()
            )));
        }
        response
            .json()
            .map_err(|e| UpdateError::InvalidManifest(e.to_string()))
    }

    pub fn check(&self, current: &str) -> Result<Option<Available>, UpdateError> {
        if !self.configured() {
            return Err(UpdateError::NotConfigured);
        }
        evaluate(&self.fetch_manifest()?, current, platform_key())
    }

    /// Descarga el artefacto anunciado y comprueba su firma (sin instalar).
    pub fn download_verified(&self, available: &Available) -> Result<Vec<u8>, UpdateError> {
        let bytes = self
            .client
            .get(&available.url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes())
            .map_err(|e| UpdateError::Network(e.to_string()))?;
        verify_artifact(&bytes, &available.signature, &self.pubkey)?;
        Ok(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestKeys {
        pubkey_b64: String,
        kp: minisign::KeyPair,
    }

    /// Par de claves de prueba generado en cada ejecución (nunca la clave real).
    fn test_keys() -> TestKeys {
        let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let pubkey_b64 = STANDARD.encode(kp.pk.to_box().unwrap().into_string());
        TestKeys { pubkey_b64, kp }
    }

    fn sign(keys: &TestKeys, data: &[u8]) -> String {
        let sig = minisign::sign(
            Some(&keys.kp.pk),
            &keys.kp.sk,
            std::io::Cursor::new(data),
            None,
            None,
        )
        .unwrap();
        STANDARD.encode(sig.into_string())
    }

    fn manifest_json(version: &str, url: &str, signature: &str) -> String {
        format!(
            r#"{{"version":"{version}","notes":"Mejoras","pub_date":"2026-10-21T00:00:00Z",
               "platforms":{{"{}":{{"signature":"{signature}","url":"{url}"}}}}}}"#,
            platform_key()
        )
    }

    #[test]
    fn newer_version_in_local_manifest_is_offered() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/latest.json")
            .with_body(manifest_json("v0.2.0", "https://x/setup.exe", "sig"))
            .expect(1)
            .create();
        let checker = UpdateChecker::new(format!("{}/latest.json", server.url()), "cGs=");
        let available = checker.check("0.1.0").unwrap().unwrap();
        assert_eq!(available.version, "0.2.0");
        assert_eq!(available.notes.as_deref(), Some("Mejoras"));
        m.assert();
    }

    #[test]
    fn same_or_older_version_or_missing_platform_offers_nothing() {
        let m = |v: &str, platform: &str| Manifest {
            version: v.into(),
            notes: None,
            platforms: HashMap::from([(
                platform.into(),
                PlatformEntry {
                    signature: "s".into(),
                    url: "u".into(),
                },
            )]),
        };
        assert_eq!(
            evaluate(&m("0.1.0", platform_key()), "0.1.0", platform_key()),
            Ok(None)
        );
        assert_eq!(
            evaluate(&m("0.0.9", platform_key()), "0.1.0", platform_key()),
            Ok(None)
        );
        assert_eq!(
            evaluate(&m("0.2.0", "otra-plataforma"), "0.1.0", platform_key()),
            Ok(None)
        );
        assert!(matches!(
            evaluate(&m("no-semver", platform_key()), "0.1.0", platform_key()),
            Err(UpdateError::InvalidManifest(_))
        ));
    }

    #[test]
    fn valid_signature_downloads_and_invalid_is_rejected() {
        let keys = test_keys();
        let artifact = b"instalador de prueba".to_vec();
        let good = sign(&keys, &artifact);
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/setup.exe")
            .with_body(artifact.clone())
            .create();
        let checker = UpdateChecker::new(format!("{}/latest.json", server.url()), &keys.pubkey_b64);
        let ok = Available {
            version: "0.2.0".into(),
            notes: None,
            url: format!("{}/setup.exe", server.url()),
            signature: good,
        };
        assert_eq!(checker.download_verified(&ok).unwrap(), artifact);

        // Firma de otro contenido: se rechaza.
        let tampered = Available {
            signature: sign(&keys, b"otro binario"),
            ..ok.clone()
        };
        assert_eq!(
            checker.download_verified(&tampered),
            Err(UpdateError::InvalidSignature)
        );

        // Firma con otra clave: se rechaza.
        let other = test_keys();
        let foreign = Available {
            signature: sign(&other, &artifact),
            ..ok
        };
        assert_eq!(
            checker.download_verified(&foreign),
            Err(UpdateError::InvalidSignature)
        );
    }

    #[test]
    fn garbage_signature_is_rejected() {
        let keys = test_keys();
        assert_eq!(
            verify_artifact(b"x", "no es base64!!", &keys.pubkey_b64),
            Err(UpdateError::InvalidSignature)
        );
    }

    #[test]
    fn without_pubkey_nothing_is_checked() {
        let mut server = mockito::Server::new();
        let m = server.mock("GET", "/latest.json").expect(0).create();
        let checker = UpdateChecker::new(format!("{}/latest.json", server.url()), "");
        assert!(!checker.configured());
        assert_eq!(checker.check("0.1.0"), Err(UpdateError::NotConfigured));
        assert_eq!(
            verify_artifact(b"x", "s", ""),
            Err(UpdateError::NotConfigured)
        );
        m.assert();
    }

    #[test]
    fn invalid_manifest_and_http_errors() {
        let mut server = mockito::Server::new();
        server.mock("GET", "/bad.json").with_body("{nope").create();
        server
            .mock("GET", "/missing.json")
            .with_status(404)
            .create();
        let bad = UpdateChecker::new(format!("{}/bad.json", server.url()), "cGs=");
        assert!(matches!(
            bad.check("0.1.0"),
            Err(UpdateError::InvalidManifest(_))
        ));
        let missing = UpdateChecker::new(format!("{}/missing.json", server.url()), "cGs=");
        assert_eq!(
            missing.check("0.1.0"),
            Err(UpdateError::Network("HTTP 404".into()))
        );
    }

    #[test]
    fn at_most_one_check_per_day_with_simulated_clock() {
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert!(should_check(true, None, t0));
        assert!(!should_check(
            true,
            Some(t0),
            t0 + Duration::from_secs(23 * 3600)
        ));
        assert!(should_check(
            true,
            Some(t0),
            t0 + Duration::from_secs(24 * 3600)
        ));
        assert!(
            !should_check(false, None, t0),
            "desactivado nunca comprueba"
        );
        // Reloj que retrocede: se permite comprobar.
        assert!(should_check(true, Some(t0), t0 - Duration::from_secs(10)));
    }
}
