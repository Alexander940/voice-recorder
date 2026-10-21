//! Oracle de HU-14: configuración de empaquetado y permisos declarados.

use serde_json::Value;

fn conf() -> Value {
    serde_json::from_str(&std::fs::read_to_string("tauri.conf.json").unwrap()).unwrap()
}

#[test]
fn bundle_targets_cover_the_three_platforms() {
    let c = conf();
    let targets: Vec<&str> = c["bundle"]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    for t in ["msi", "nsis", "dmg", "appimage", "deb"] {
        assert!(targets.contains(&t), "falta el target {t}");
    }
    assert_eq!(c["bundle"]["active"], true);
}

#[test]
fn macos_info_plist_declares_permissions_and_is_referenced() {
    let c = conf();
    assert_eq!(c["bundle"]["macOS"]["infoPlist"], "Info.plist");
    let plist = std::fs::read_to_string("Info.plist").unwrap();
    for key in [
        "NSMicrophoneUsageDescription",
        "NSAppleEventsUsageDescription",
    ] {
        let pos = plist
            .find(&format!("<key>{key}</key>"))
            .unwrap_or_else(|| panic!("falta {key}"));
        let rest = &plist[pos..];
        let text_start = rest.find("<string>").unwrap() + "<string>".len();
        let text_end = rest.find("</string>").unwrap();
        assert!(text_end - text_start > 20, "{key} sin descripción útil");
    }
}

#[test]
fn release_workflow_never_publishes_on_dispatch_and_only_drafts_on_tags() {
    let wf = std::fs::read_to_string("../.github/workflows/release.yml").unwrap();
    assert!(wf.contains("tags: [\"v*\"]"));
    assert!(wf.contains("workflow_dispatch"));
    assert!(wf.contains("tagName: ${{ github.event_name == 'push' && github.ref_name || '' }}"));
    assert!(wf.contains("releaseDraft: true"));
    assert!(
        !wf.contains("pull_request"),
        "el release no debe correr en PRs"
    );
}

/// Primer release: el updater tiene una clave pública minisign válida y el build
/// genera los artefactos firmados (la clave privada solo está en GitHub Secrets).
#[test]
fn updater_has_a_valid_public_key_and_signed_artifacts() {
    use base64::Engine;
    let c = conf();
    // En la base no: un build local sin la clave privada terminaría con error.
    assert!(c["bundle"]["createUpdaterArtifacts"].is_null());
    let release: Value =
        serde_json::from_str(&std::fs::read_to_string("tauri.release.conf.json").unwrap()).unwrap();
    assert_eq!(release["bundle"]["createUpdaterArtifacts"], true);
    let wf = std::fs::read_to_string("../.github/workflows/release.yml").unwrap();
    assert!(wf.contains("--config src-tauri/tauri.release.conf.json"));
    let pubkey = c["plugins"]["updater"]["pubkey"].as_str().unwrap();
    let text = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(pubkey.trim())
            .unwrap(),
    )
    .unwrap();
    assert!(text.starts_with("untrusted comment: minisign public key"));
    assert!(minisign_verify::PublicKey::decode(&text).is_ok());
    // Nunca una clave privada en el repo.
    assert!(!text.contains("secret key"));
    let endpoint = c["plugins"]["updater"]["endpoints"][0].as_str().unwrap();
    assert!(endpoint.starts_with("https://github.com/Alexander940/voice-recorder/releases/"));
}
