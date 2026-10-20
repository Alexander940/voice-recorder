//! Valida las keys reales con los proveedores (listar modelos: sin coste).
//! `LIVE_API_TESTS=1 cargo test --test keys_live -- --ignored`

use voice_recorder_lib::settings::keys::{HttpValidator, KeyValidator, Provider};

#[test]
#[ignore = "llama a las APIs de Groq y Anthropic"]
fn real_keys_from_env_are_valid() {
    if std::env::var("LIVE_API_TESTS").as_deref() != Ok("1") {
        eprintln!("omitido: LIVE_API_TESTS != 1");
        return;
    }
    let v = HttpValidator::new();
    for p in Provider::ALL {
        let key = std::env::var(p.env_var()).unwrap_or_else(|_| panic!("{}", p.env_var()));
        v.validate(p, &key)
            .unwrap_or_else(|e| panic!("{}: {e}", p.name()));
        println!("LIVE keys: {} válida", p.name());
    }
}
