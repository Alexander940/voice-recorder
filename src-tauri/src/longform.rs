//! Dictados largos por trozos: unión de transcripciones solapadas.
//!
//! Cada trozo se graba con unos segundos de solape con el anterior, así que su
//! transcripción repite las últimas palabras del trozo previo. Whisper puede cortar
//! distinto la primera o la última palabra en el borde, por eso la coincidencia
//! tolera unas pocas palabras sueltas en cada extremo.

use std::time::{Duration, Instant};

use crate::errors::AppError;
use crate::llm::{CleanupOutcome, TextCleaner, clean_or_raw};
use crate::stt::{SttError, TranscribeOptions, Transcriber};
use crate::vocabulary::Vocabulary;

/// Cada trozo dura 10 minutos: unos 19 MB en WAV, por debajo de los 25 MB que
/// acepta Groq en el plan gratuito.
pub const CHUNK_LEN: Duration = Duration::from_secs(10 * 60);
/// Solape entre trozos para no cortar palabras en el borde.
pub const CHUNK_OVERLAP: Duration = Duration::from_secs(5);

/// Palabras que como mucho se ignoran en cada borde del solape.
const EDGE_SLACK: usize = 3;
/// Coincidencia mínima para considerar que dos trozos se solapan.
const MIN_MATCH: usize = 3;
/// El solape de 5 s tiene unas 15 palabras; se busca con margen.
const MAX_MATCH: usize = 40;

/// Instrucción añadida a la limpieza de cada trozo de un dictado largo.
pub const FRAGMENT_INSTRUCTIONS: &str = "\
Este texto es un fragmento consecutivo de un dictado largo que se procesa por partes. \
Límpialo sin añadir títulos, saludos, despedidas ni resúmenes, y sin completar ni \
cerrar frases que empiecen o terminen a medias en los bordes.";

fn normalize(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Devuelve `next` sin la parte que repite el final de `prev`.
///
/// Busca la coincidencia más larga entre las últimas palabras de `prev` y las
/// primeras de `next`, admitiendo hasta `EDGE_SLACK` palabras distintas en cada
/// borde (una palabra cortada por el solape). Sin coincidencia, devuelve `next`.
pub fn strip_overlap(prev: &str, next: &str) -> String {
    let a: Vec<String> = prev.split_whitespace().map(normalize).collect();
    let b_raw: Vec<&str> = next.split_whitespace().collect();
    let b: Vec<String> = b_raw.iter().map(|w| normalize(w)).collect();
    let mut best: Option<(usize, usize)> = None; // (longitud, fin del solape en `next`)
    for trim_a in 0..=EDGE_SLACK.min(a.len()) {
        let a_end = a.len() - trim_a;
        for skip_b in 0..=EDGE_SLACK.min(b.len()) {
            let max_k = MAX_MATCH.min(a_end).min(b.len() - skip_b);
            for k in (MIN_MATCH..=max_k).rev() {
                if best.is_some_and(|(len, _)| len >= k) {
                    break;
                }
                let tail = &a[a_end - k..a_end];
                let head = &b[skip_b..skip_b + k];
                if tail.iter().all(|w| !w.is_empty()) && tail == head {
                    best = Some((k, skip_b + k));
                    break;
                }
            }
        }
    }
    match best {
        Some((_, end)) => b_raw[end..].join(" "),
        None => next.trim().to_string(),
    }
}

/// Une los textos ya limpios de cada trozo.
pub fn join_parts(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Espera recomendada ante un «demasiadas peticiones» de Groq: la que indique el
/// servidor (acotada) o un retroceso creciente.
pub fn rate_limit_wait(retry_after: Option<Duration>, attempt: u32) -> Duration {
    retry_after
        .unwrap_or_else(|| Duration::from_secs(15 * 2u64.pow(attempt.min(4))))
        .clamp(Duration::from_secs(1), Duration::from_secs(5 * 60))
}

/// Reintentos ante límites de Groq antes de dar el trozo por fallido.
pub const RATE_LIMIT_RETRIES: u32 = 6;

/// Texto de un dictado largo que se va acumulando trozo a trozo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LongText {
    /// Transcripción de cada trozo sin el solape (lo que se limpió).
    pub raw_parts: Vec<String>,
    /// Texto final de cada trozo (limpio y con el vocabulario aplicado).
    pub parts: Vec<String>,
    /// Transcripción completa del último trozo, para quitar el solape del siguiente.
    last_raw: String,
    pub chunks: usize,
    pub cleaned_any: bool,
    pub stt_ms: i64,
    pub llm_ms: Option<i64>,
}

impl LongText {
    pub fn raw(&self) -> String {
        join_parts(&self.raw_parts)
    }

    pub fn text(&self) -> String {
        join_parts(&self.parts)
    }
}

/// Transcribe esperando y reintentando cuando Groq responde «demasiadas peticiones».
pub fn transcribe_patiently(
    transcriber: &dyn Transcriber,
    wav: &[u8],
    options: &TranscribeOptions,
    wait: &dyn Fn(Duration),
) -> Result<crate::stt::Transcript, SttError> {
    let mut attempt = 0;
    loop {
        match transcriber.transcribe(wav, options) {
            Err(SttError::RateLimited(after)) if attempt < RATE_LIMIT_RETRIES => {
                let pause = rate_limit_wait(after, attempt);
                tracing::info!(secs = pause.as_secs(), attempt, "límite de Groq: esperando");
                wait(pause);
                attempt += 1;
            }
            other => return other,
        }
    }
}

/// Procesa un trozo: lo transcribe (con paciencia ante los límites de Groq), quita
/// lo que repite del anterior, limpia solo lo nuevo y lo añade a `acc`.
#[allow(clippy::too_many_arguments)]
pub fn process_chunk(
    transcriber: &dyn Transcriber,
    cleaner: Option<&dyn TextCleaner>,
    wav: &[u8],
    options: &TranscribeOptions,
    instructions: &str,
    vocabulary: &Vocabulary,
    acc: &mut LongText,
    wait: &dyn Fn(Duration),
) -> Result<(), AppError> {
    let transcript = transcribe_patiently(transcriber, wav, options, wait)?;
    let new_raw = strip_overlap(&acc.last_raw, &transcript.text);
    acc.last_raw = transcript.text;
    acc.chunks += 1;
    acc.stt_ms += transcript.latency.as_millis() as i64;
    if new_raw.trim().is_empty() {
        return Ok(());
    }
    let instructions = format!("{}\n{FRAGMENT_INSTRUCTIONS}", instructions.trim());
    let started = Instant::now();
    let outcome = clean_or_raw(cleaner, &new_raw, instructions.trim());
    if cleaner.is_some() {
        *acc.llm_ms.get_or_insert(0) += started.elapsed().as_millis() as i64;
    }
    acc.cleaned_any |= matches!(outcome, CleanupOutcome::Cleaned(_));
    acc.parts
        .push(vocabulary.apply_replacements(outcome.text()));
    acc.raw_parts.push(new_raw);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{Cleaned, LlmError};
    use crate::stt::Transcript;
    use std::sync::Mutex;

    /// Devuelve las respuestas en orden.
    struct Scripted(Mutex<Vec<Result<&'static str, SttError>>>);

    impl Transcriber for Scripted {
        fn transcribe(&self, _: &[u8], _: &TranscribeOptions) -> Result<Transcript, SttError> {
            self.0.lock().unwrap().remove(0).map(|t| Transcript {
                text: t.into(),
                latency: Duration::from_millis(10),
            })
        }
    }

    /// Limpia en mayúsculas y comprueba que recibe la instrucción de fragmento.
    struct Upper;
    impl TextCleaner for Upper {
        fn clean(&self, raw: &str, instructions: &str) -> Result<Cleaned, LlmError> {
            assert!(instructions.contains("fragmento consecutivo"));
            Ok(Cleaned {
                text: raw.to_uppercase(),
                latency: Duration::ZERO,
                usage: None,
            })
        }
    }

    fn run(t: &Scripted, c: Option<&dyn TextCleaner>, acc: &mut LongText) -> Result<(), AppError> {
        process_chunk(
            t,
            c,
            &[0; 4],
            &TranscribeOptions::default(),
            "modo",
            &Vocabulary::default(),
            acc,
            &|_| {},
        )
    }

    #[test]
    fn chunks_are_joined_without_the_overlap_and_cleaned_once() {
        let t = Scripted(Mutex::new(vec![
            Ok("empezamos la reunión con el presupuesto del trimestre"),
            Ok("presupuesto del trimestre y seguimos con el equipo"),
            Ok("seguimos con el equipo nuevo"),
        ]));
        let mut acc = LongText::default();
        for _ in 0..3 {
            run(&t, Some(&Upper), &mut acc).unwrap();
        }
        assert_eq!(
            acc.text(),
            "EMPEZAMOS LA REUNIÓN CON EL PRESUPUESTO DEL TRIMESTRE Y SEGUIMOS CON EL EQUIPO NUEVO"
        );
        assert_eq!(
            acc.raw(),
            "empezamos la reunión con el presupuesto del trimestre y seguimos con el equipo nuevo"
        );
        assert_eq!(acc.chunks, 3);
        assert!(acc.cleaned_any);
        assert_eq!(acc.stt_ms, 30);
    }

    #[test]
    fn rate_limit_waits_and_retries_the_same_chunk() {
        let t = Scripted(Mutex::new(vec![
            Err(SttError::RateLimited(Some(Duration::from_secs(9)))),
            Err(SttError::RateLimited(None)),
            Ok("hola mundo"),
        ]));
        let waits = Mutex::new(Vec::new());
        let mut acc = LongText::default();
        process_chunk(
            &t,
            None,
            &[0; 4],
            &TranscribeOptions::default(),
            "",
            &Vocabulary::default(),
            &mut acc,
            &|d| waits.lock().unwrap().push(d),
        )
        .unwrap();
        assert_eq!(acc.text(), "hola mundo");
        assert_eq!(
            *waits.lock().unwrap(),
            [Duration::from_secs(9), Duration::from_secs(30)]
        );
    }

    #[test]
    fn persistent_rate_limit_gives_up_and_keeps_previous_text() {
        let mut answers = vec![Ok("primer trozo dictado aquí")];
        answers.extend((0..=RATE_LIMIT_RETRIES).map(|_| Err(SttError::RateLimited(None))));
        let t = Scripted(Mutex::new(answers));
        let mut acc = LongText::default();
        run(&t, None, &mut acc).unwrap();
        let err = run(&t, None, &mut acc).unwrap_err();
        assert_eq!(
            err,
            AppError::RateLimited(crate::settings::keys::Provider::Groq)
        );
        assert!(err.retryable());
        assert_eq!(acc.text(), "primer trozo dictado aquí");
        assert_eq!(acc.chunks, 1);
    }

    #[test]
    fn failed_cleanup_keeps_the_raw_chunk() {
        struct Broken;
        impl TextCleaner for Broken {
            fn clean(&self, _: &str, _: &str) -> Result<Cleaned, LlmError> {
                Err(LlmError::Timeout)
            }
        }
        let t = Scripted(Mutex::new(vec![Ok("texto sin limpiar")]));
        let mut acc = LongText::default();
        run(&t, Some(&Broken), &mut acc).unwrap();
        assert_eq!(acc.text(), "texto sin limpiar");
        assert!(!acc.cleaned_any);
    }

    /// «Transcribe» un audio en el que cada segundo tiene un nivel constante n/100:
    /// devuelve una palabra `sN` por segundo, como si cada segundo dijera su número.
    struct SecondsTranscriber;
    impl Transcriber for SecondsTranscriber {
        fn transcribe(&self, wav: &[u8], _: &TranscribeOptions) -> Result<Transcript, SttError> {
            let mut reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
            let samples: Vec<f32> = reader
                .samples::<i16>()
                .map(|s| s.unwrap() as f32 / i16::MAX as f32)
                .collect();
            let words: Vec<String> = samples
                .chunks(crate::audio::TARGET_RATE as usize)
                .filter(|block| block.len() == crate::audio::TARGET_RATE as usize)
                .map(|block| {
                    let mean = block.iter().sum::<f32>() / block.len() as f32;
                    format!("s{}", (mean * 100.0).round() as i32)
                })
                .collect();
            Ok(Transcript {
                text: words.join(" "),
                latency: Duration::ZERO,
            })
        }
    }

    /// De extremo a extremo: grabación de 27 s cortada en trozos de 10 s con 5 s
    /// de solape (como en la app, a escala) → cada segundo aparece una sola vez.
    #[test]
    fn long_recording_is_transcribed_once_in_order() {
        use crate::audio::Recording;
        let rate = 48_000;
        let mut rec = Recording::new(rate, 1, Duration::from_secs(60));
        let mut acc = LongText::default();
        let transcribe = |segment: crate::audio::Segment, acc: &mut LongText| {
            process_chunk(
                &SecondsTranscriber,
                None,
                &segment.encode().unwrap(),
                &TranscribeOptions::default(),
                "",
                &Vocabulary::default(),
                acc,
                &|_| {},
            )
            .unwrap();
        };
        // Se graba segundo a segundo y se sacan trozos mientras tanto.
        for second in 0..27 {
            rec.push(&vec![second as f32 / 100.0; rate as usize]);
            if let Some(chunk) = rec.take_chunk(Duration::from_secs(10), Duration::from_secs(5)) {
                transcribe(chunk, &mut acc);
            }
        }
        assert_eq!(acc.chunks, 2, "trozos tomados durante la grabación");
        transcribe(rec.finish_segment().unwrap(), &mut acc);
        let expected: Vec<String> = (0..27).map(|n| format!("s{n}")).collect();
        assert_eq!(acc.text(), expected.join(" "));
    }

    #[test]
    fn silent_chunk_adds_nothing() {
        let t = Scripted(Mutex::new(vec![Ok("algo dicho antes"), Ok("")]));
        let mut acc = LongText::default();
        run(&t, None, &mut acc).unwrap();
        run(&t, None, &mut acc).unwrap();
        assert_eq!(acc.text(), "algo dicho antes");
        assert_eq!(acc.chunks, 2);
    }

    #[test]
    fn removes_exact_overlap() {
        let prev = "hoy revisamos el presupuesto del trimestre y luego hablamos del equipo";
        let next = "luego hablamos del equipo nuevo que llega en marzo";
        assert_eq!(strip_overlap(prev, next), "nuevo que llega en marzo");
    }

    #[test]
    fn tolerates_punctuation_case_and_cut_words_at_the_edges() {
        // Whisper corta distinto en el borde: «equi» al final y «po.» al principio.
        // La palabra cortada del final de `prev` ya se entregó: se conserva la
        // completa de `next` en lugar de perderla.
        let prev = "Hoy revisamos el presupuesto, y luego hablamos del equi";
        let next = "po. Luego hablamos del equipo nuevo, que llega en marzo.";
        assert_eq!(
            strip_overlap(prev, next),
            "equipo nuevo, que llega en marzo."
        );
    }

    #[test]
    fn keeps_next_when_there_is_no_overlap() {
        assert_eq!(
            strip_overlap("uno dos tres cuatro", "cinco seis siete"),
            "cinco seis siete"
        );
        assert_eq!(strip_overlap("", "hola mundo"), "hola mundo");
    }

    #[test]
    fn a_short_coincidence_is_not_an_overlap() {
        // Solo «de la» coincide: no basta para cortar.
        assert_eq!(
            strip_overlap("la casa de la", "de la montaña es grande"),
            "de la montaña es grande"
        );
    }

    #[test]
    fn prefers_the_longest_match() {
        let prev = "a b c d e f g a b c";
        let next = "a b c d e f g h i";
        assert_eq!(strip_overlap(prev, next), "h i");
    }

    #[test]
    fn whole_next_inside_overlap_leaves_nothing() {
        assert_eq!(strip_overlap("x y z uno dos tres", "uno dos tres"), "");
    }

    #[test]
    fn join_skips_empty_parts() {
        let parts = vec!["Hola.".to_string(), "  ".into(), "Adiós.".into()];
        assert_eq!(join_parts(&parts), "Hola. Adiós.");
    }

    #[test]
    fn chunk_wav_fits_groq_free_tier() {
        // 16 kHz mono 16 bits = 32 000 B/s; Groq gratuito acepta 25 MB.
        let bytes = (CHUNK_LEN + CHUNK_OVERLAP).as_secs() * 32_000 + 44;
        assert!(bytes < 25 * 1024 * 1024, "{bytes} B");
    }

    #[test]
    fn rate_limit_wait_uses_server_hint_or_backs_off() {
        assert_eq!(
            rate_limit_wait(Some(Duration::from_secs(7)), 0),
            Duration::from_secs(7)
        );
        assert_eq!(
            rate_limit_wait(Some(Duration::from_secs(3600)), 0),
            Duration::from_secs(300)
        );
        assert_eq!(rate_limit_wait(None, 0), Duration::from_secs(15));
        assert_eq!(rate_limit_wait(None, 2), Duration::from_secs(60));
        assert_eq!(rate_limit_wait(None, 9), Duration::from_secs(240));
    }
}
