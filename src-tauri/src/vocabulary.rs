//! Vocabulario personalizado (HU-10): términos para Whisper y Claude, y reemplazos.

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, Runtime, State};

/// Whisper acepta como mucho 224 tokens de `prompt`.
pub const WHISPER_PROMPT_MAX_TOKENS: usize = 224;

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Replacement {
    /// Lo que se oye/transcribe (sin distinguir mayúsculas, palabra completa).
    pub from: String,
    /// Lo que se escribe.
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Vocabulary {
    pub terms: Vec<String>,
    pub replacements: Vec<Replacement>,
}

/// Estimación conservadora de tokens de Whisper (~3 caracteres por token).
fn approx_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(3)
}

impl Vocabulary {
    /// Quita vacíos y duplicados conservando el orden.
    pub fn normalized(mut self) -> Self {
        let mut seen = std::collections::HashSet::new();
        self.terms = self
            .terms
            .into_iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty() && seen.insert(t.to_lowercase()))
            .collect();
        self.replacements
            .retain(|r| !r.from.trim().is_empty() && !r.to.trim().is_empty());
        for r in &mut self.replacements {
            r.from = r.from.trim().into();
            r.to = r.to.trim().into();
        }
        self
    }

    /// `prompt` para Whisper: términos completos hasta el límite de tokens.
    pub fn whisper_prompt(&self) -> Option<String> {
        let mut prompt = String::new();
        for term in &self.terms {
            let candidate = if prompt.is_empty() {
                term.clone()
            } else {
                format!("{prompt}, {term}")
            };
            if approx_tokens(&candidate) > WHISPER_PROMPT_MAX_TOKENS {
                break;
            }
            prompt = candidate;
        }
        (!prompt.is_empty()).then_some(prompt)
    }

    /// Instrucción extra para Claude con la ortografía preferida.
    pub fn llm_instructions(&self) -> String {
        if self.terms.is_empty() {
            return String::new();
        }
        format!(
            "Ortografía preferida para estos términos (respétala exactamente): {}.",
            self.terms.join(", ")
        )
    }

    /// Aplica los reemplazos sin distinguir mayúsculas y solo a palabras completas.
    pub fn apply_replacements(&self, text: &str) -> String {
        self.replacements.iter().fold(text.to_string(), |acc, r| {
            replace_whole_word(&acc, &r.from, &r.to)
        })
    }
}

fn is_word_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

fn replace_whole_word(text: &str, from: &str, to: &str) -> String {
    let Ok(re) = regex::Regex::new(&format!("(?i){}", regex::escape(from))) else {
        return text.to_string();
    };
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in re.find_iter(text) {
        let before = text[..m.start()].chars().next_back();
        let after = text[m.end()..].chars().next();
        if is_word_char(before) || is_word_char(after) {
            continue;
        }
        out.push_str(&text[last..m.start()]);
        out.push_str(to);
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

/// Vocabulario persistido en `vocabulary.json`.
pub struct VocabularyStore {
    path: Option<PathBuf>,
    current: Mutex<Vocabulary>,
}

impl VocabularyStore {
    pub fn load(path: Option<PathBuf>) -> Self {
        let current = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Vocabulary>(&t).ok())
            .map(Vocabulary::normalized)
            .unwrap_or_default();
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn open<R: Runtime>(app: &AppHandle<R>) -> Self {
        Self::load(
            app.path()
                .app_data_dir()
                .ok()
                .map(|d| d.join("vocabulary.json")),
        )
    }

    pub fn get(&self) -> Vocabulary {
        self.current.lock().expect("vocabulary poisoned").clone()
    }

    pub fn save(&self, vocabulary: Vocabulary) -> Result<Vocabulary, String> {
        let vocabulary = vocabulary.normalized();
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let tmp = path.with_extension("json.tmp");
            let json = serde_json::to_string_pretty(&vocabulary).map_err(|e| e.to_string())?;
            std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        }
        *self.current.lock().expect("vocabulary poisoned") = vocabulary.clone();
        Ok(vocabulary)
    }
}

#[tauri::command]
pub fn vocabulary_get(store: State<'_, VocabularyStore>) -> Vocabulary {
    store.get()
}

#[tauri::command]
pub fn vocabulary_save(
    store: State<'_, VocabularyStore>,
    vocabulary: Vocabulary,
) -> Result<Vocabulary, String> {
    store.save(vocabulary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocab(terms: &[&str], reps: &[(&str, &str)]) -> Vocabulary {
        Vocabulary {
            terms: terms.iter().map(|s| s.to_string()).collect(),
            replacements: reps
                .iter()
                .map(|(f, t)| Replacement {
                    from: f.to_string(),
                    to: t.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn whisper_prompt_joins_terms() {
        let v = vocab(&["Kubernetes", "Anthropic"], &[]);
        assert_eq!(v.whisper_prompt().as_deref(), Some("Kubernetes, Anthropic"));
        assert_eq!(vocab(&[], &[]).whisper_prompt(), None);
    }

    #[test]
    fn whisper_prompt_is_capped_without_cutting_terms() {
        let terms: Vec<String> = (0..500).map(|i| format!("Termino{i:03}")).collect();
        let v = Vocabulary {
            terms: terms.clone(),
            replacements: vec![],
        };
        let prompt = v.whisper_prompt().unwrap();
        assert!(approx_tokens(&prompt) <= WHISPER_PROMPT_MAX_TOKENS);
        // Cada trozo es un término completo y están en orden.
        let parts: Vec<&str> = prompt.split(", ").collect();
        assert!(parts.len() > 10 && parts.len() < terms.len());
        assert!(parts.iter().zip(&terms).all(|(p, t)| p == t));
    }

    #[test]
    fn llm_instructions_list_terms() {
        assert_eq!(vocab(&[], &[]).llm_instructions(), "");
        assert!(
            vocab(&["Bancolombia"], &[])
                .llm_instructions()
                .contains("Bancolombia")
        );
    }

    #[test]
    fn replacements_are_case_insensitive_whole_words() {
        let v = vocab(&[], &[("cubernetes", "Kubernetes"), ("ia", "IA")]);
        assert_eq!(
            v.apply_replacements("Desplegamos en Cubernetes y cubernetes."),
            "Desplegamos en Kubernetes y Kubernetes."
        );
        // No reemplaza dentro de otras palabras.
        assert_eq!(
            v.apply_replacements("La media de la ia es alta"),
            "La media de la IA es alta"
        );
        assert_eq!(v.apply_replacements("ia ia"), "IA IA");
    }

    #[test]
    fn replacements_handle_multiword_and_accents() {
        let v = vocab(
            &[],
            &[("anthropic", "Anthropic"), ("ban colombia", "Bancolombia")],
        );
        assert_eq!(
            v.apply_replacements("Llamé a Ban Colombia y a anthropic, señor."),
            "Llamé a Bancolombia y a Anthropic, señor."
        );
    }

    #[test]
    fn normalized_drops_empty_and_duplicate_terms() {
        let v = vocab(
            &[" Kubernetes ", "", "kubernetes", "Rust"],
            &[("", "x"), (" a ", " b ")],
        )
        .normalized();
        assert_eq!(v.terms, ["Kubernetes", "Rust"]);
        assert_eq!(
            v.replacements,
            [Replacement {
                from: "a".into(),
                to: "b".into()
            }]
        );
    }

    #[test]
    fn store_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vocabulary.json");
        let store = VocabularyStore::load(Some(path.clone()));
        assert_eq!(store.get(), Vocabulary::default());
        store
            .save(vocab(&["Tauri"], &[("tauri", "Tauri")]))
            .unwrap();
        assert_eq!(VocabularyStore::load(Some(path)).get().terms, ["Tauri"]);
    }
}
