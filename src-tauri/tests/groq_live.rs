//! Test contra la API real de Groq. Solo corre con `LIVE_API_TESTS=1` y `GROQ_API_KEY`:
//! `LIVE_API_TESTS=1 cargo test --test groq_live -- --ignored`

use voice_recorder_lib::stt::{GroqTranscriber, TranscribeOptions, Transcriber, word_error_rate};

/// Tarifa de whisper-large-v3-turbo usada para estimar el gasto en el log del loop.
const USD_PER_HOUR: f64 = 0.04;
const MAX_WER: f64 = 0.15;

#[test]
#[ignore = "llama a la API de pago de Groq"]
fn spanish_fixture_wer_is_below_threshold() {
    if std::env::var("LIVE_API_TESTS").as_deref() != Ok("1") {
        eprintln!("omitido: LIVE_API_TESTS != 1");
        return;
    }
    let key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY");
    let wav = std::fs::read("tests/fixtures/es_sample.wav").unwrap();
    let reference = std::fs::read_to_string("tests/fixtures/es_sample.txt").unwrap();

    let options = TranscribeOptions {
        language: Some("es".into()),
        ..Default::default()
    };
    let t = GroqTranscriber::new(key)
        .transcribe(&wav, &options)
        .unwrap();

    let secs = (wav.len() - 44) as f64 / 32_000.0;
    let wer = word_error_rate(&reference, &t.text);
    println!(
        "LIVE groq: wer={wer:.3} latency_ms={} audio_s={secs:.1} est_cost_usd={:.6}\ntexto: {}",
        t.latency.as_millis(),
        secs / 3600.0 * USD_PER_HOUR,
        t.text
    );
    assert!(wer <= MAX_WER, "WER {wer:.3} > {MAX_WER}: {}", t.text);
}
