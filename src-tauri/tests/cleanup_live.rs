//! Evaluación de la limpieza con Claude real y comprobaciones deterministas.
//! `LIVE_API_TESTS=1 cargo test --test cleanup_live -- --ignored --nocapture`

use regex::Regex;
use voice_recorder_lib::llm::{AnthropicCleaner, TextCleaner};

#[derive(serde::Deserialize)]
struct Case {
    id: String,
    lang: String,
    input: String,
    keywords: Vec<String>,
    fillers: Vec<String>,
    #[serde(default)]
    question: bool,
    #[serde(default)]
    forbidden: Vec<String>,
}

fn contains_word(text: &str, word: &str) -> bool {
    Regex::new(&format!(
        r"(?i)(^|[^\p{{L}}]){}($|[^\p{{L}}])",
        regex::escape(word)
    ))
    .unwrap()
    .is_match(text)
}

/// Devuelve los fallos de un caso; vacío = pasa.
fn check(case: &Case, out: &str) -> Vec<String> {
    let mut fails = Vec::new();
    let preamble = Regex::new(
        r"(?i)^\s*(aquí|aqui|claro|por supuesto|texto (corregido|limpio)|here|sure|certainly|the (cleaned|corrected) text|<transcripcion>)",
    )
    .unwrap();
    if preamble.is_match(out) {
        fails.push("empieza con preámbulo".into());
    }
    for f in &case.fillers {
        if contains_word(out, f) {
            fails.push(format!("contiene la muletilla «{f}»"));
        }
    }
    for k in &case.keywords {
        if !out.to_lowercase().contains(&k.to_lowercase()) {
            fails.push(format!("perdió la palabra clave «{k}»"));
        }
    }
    for w in &case.forbidden {
        if contains_word(out, w) {
            fails.push(format!("contiene «{w}», que no debería"));
        }
    }
    match whatlang::detect_lang(out) {
        Some(l) if l.code() == case.lang => {}
        other => fails.push(format!("idioma {other:?}, se esperaba {}", case.lang)),
    }
    let ratio = out.chars().count() as f64 / case.input.chars().count() as f64;
    if !(0.5..=1.3).contains(&ratio) {
        fails.push(format!("longitud {ratio:.2}× la entrada"));
    }
    if case.question && !out.trim_end().ends_with('?') {
        fails.push("respondió o no conservó la pregunta".into());
    }
    fails
}

#[test]
#[ignore = "llama a la API de pago de Anthropic"]
fn cleanup_eval_passes_at_least_9_of_10() {
    if std::env::var("LIVE_API_TESTS").as_deref() != Ok("1") {
        eprintln!("omitido: LIVE_API_TESTS != 1");
        return;
    }
    let key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY");
    let cases: Vec<Case> = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/cleanup_cases.json").unwrap(),
    )
    .unwrap();
    // Timeout amplio en la evaluación: la latencia se mide aparte.
    let cleaner = AnthropicCleaner::with_base_url(
        "https://api.anthropic.com/v1",
        key,
        std::time::Duration::from_secs(30),
    );
    let (mut passed, mut cost, mut latencies) = (0, 0.0, Vec::new());
    for case in &cases {
        let result = cleaner.clean(&case.input, "");
        let (out, fails) = match &result {
            Ok(c) => {
                cost += c.usage.map(|u| u.cost_usd()).unwrap_or(0.0);
                latencies.push(c.latency.as_millis());
                (c.text.clone(), check(case, &c.text))
            }
            Err(e) => (String::new(), vec![format!("error: {e}")]),
        };
        if fails.is_empty() {
            passed += 1;
        }
        println!(
            "[{}] {} → «{}» {}",
            if fails.is_empty() { "OK" } else { "FALLA" },
            case.id,
            out,
            fails.join("; ")
        );
    }
    latencies.sort();
    println!(
        "LIVE cleanup: {passed}/{} cost_usd={cost:.6} latency_ms p50={} max={}",
        cases.len(),
        latencies.get(latencies.len() / 2).copied().unwrap_or(0),
        latencies.last().copied().unwrap_or(0)
    );
    assert!(passed >= 9, "solo {passed}/{} casos pasaron", cases.len());
}
