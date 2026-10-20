//! El vocabulario guía a Whisper hacia la ortografía exacta de términos propios.
//! `LIVE_API_TESTS=1 cargo test --test vocabulary_live -- --ignored --nocapture`

use voice_recorder_lib::stt::{GroqTranscriber, TranscribeOptions, Transcriber};
use voice_recorder_lib::vocabulary::Vocabulary;

const TERMS: [&str; 3] = ["Kubernetes", "Anthropic", "Bancolombia"];

fn hits(text: &str) -> usize {
    TERMS.iter().filter(|t| text.contains(*t)).count()
}

#[test]
#[ignore = "llama a la API de pago de Groq"]
fn vocabulary_prompt_gets_at_least_2_of_3_terms_exact() {
    if std::env::var("LIVE_API_TESTS").as_deref() != Ok("1") {
        eprintln!("omitido: LIVE_API_TESTS != 1");
        return;
    }
    let groq = GroqTranscriber::new(std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY"));
    let wav = std::fs::read("tests/fixtures/vocab_sample.wav").unwrap();
    let vocabulary = Vocabulary {
        terms: TERMS.iter().map(|s| s.to_string()).collect(),
        replacements: vec![],
    };

    let base = TranscribeOptions {
        language: Some("es".into()),
        ..Default::default()
    };
    let without = groq.transcribe(&wav, &base).unwrap().text;
    let with = groq
        .transcribe(
            &wav,
            &TranscribeOptions {
                prompt: vocabulary.whisper_prompt(),
                ..base
            },
        )
        .unwrap()
        .text;

    let secs = (wav.len() - 44) as f64 / 32_000.0;
    println!("sin vocabulario ({}/3): {without}", hits(&without));
    println!("con vocabulario ({}/3): {with}", hits(&with));

    // Informativo: el pipeline completo añade las instrucciones de ortografía a Claude.
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        use voice_recorder_lib::llm::{AnthropicCleaner, TextCleaner};
        let cleaner = AnthropicCleaner::with_base_url(
            "https://api.anthropic.com/v1",
            key,
            std::time::Duration::from_secs(30),
        );
        if let Ok(c) = cleaner.clean(&with, &vocabulary.llm_instructions()) {
            println!("pipeline Whisper+Claude ({}/3): {}", hits(&c.text), c.text);
        }
    }
    println!(
        "LIVE vocabulary: {}/3 est_cost_usd={:.6}",
        hits(&with),
        2.0 * secs / 3600.0 * 0.04
    );
    assert!(
        hits(&with) >= 2,
        "solo {}/3 términos exactos: {with}",
        hits(&with)
    );
}
