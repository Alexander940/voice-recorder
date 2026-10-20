//! Word Error Rate para evaluar transcripciones contra una referencia.

/// Minúsculas, sin puntuación, separado en palabras. Conserva los acentos.
pub fn normalize_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// (sustituciones + inserciones + borrados) / palabras de la referencia.
pub fn word_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let r = normalize_words(reference);
    let h = normalize_words(hypothesis);
    if r.is_empty() {
        return if h.is_empty() { 0.0 } else { 1.0 };
    }
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    for (i, rw) in r.iter().enumerate() {
        let mut row = vec![i + 1; h.len() + 1];
        for (j, hw) in h.iter().enumerate() {
            let cost = usize::from(rw != hw);
            row[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1);
        }
        prev = row;
    }
    prev[h.len()] as f64 / r.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_ignoring_case_and_punctuation_is_zero() {
        assert_eq!(word_error_rate("Hola, mundo.", "hola mundo"), 0.0);
    }

    #[test]
    fn one_substitution_in_four_words() {
        assert_eq!(word_error_rate("a b c d", "a x c d"), 0.25);
    }

    #[test]
    fn insertions_and_deletions_count() {
        assert_eq!(word_error_rate("a b c d", "a b c d e"), 0.25);
        assert_eq!(word_error_rate("a b c d", "a c d"), 0.25);
    }

    #[test]
    fn accents_matter() {
        assert_eq!(word_error_rate("reunión", "reunion"), 1.0);
    }

    #[test]
    fn empty_reference() {
        assert_eq!(word_error_rate("", ""), 0.0);
        assert_eq!(word_error_rate("", "algo"), 1.0);
    }
}
