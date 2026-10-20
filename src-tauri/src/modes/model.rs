//! Modos de dictado: lógica pura, sin Tauri.

pub const DEFAULT_MODE: &str = "clean";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Mode {
    pub id: String,
    pub name: String,
    /// Instrucciones que se añaden al prompt de sistema de la limpieza.
    pub prompt: String,
    /// Idioma de salida forzado (p. ej. "inglés"); `None` conserva el del dictado.
    pub output_language: Option<String>,
    /// `false` = Literal: se pega la transcripción sin pasar por Claude.
    pub use_llm: bool,
    /// Los predefinidos se pueden editar pero no borrar.
    pub builtin: bool,
}

/// Lo que ve el frontend: los modos y cuál está activo.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModesState {
    pub modes: Vec<Mode>,
    pub active: String,
}

fn builtin(id: &str, name: &str, prompt: &str, lang: Option<&str>, use_llm: bool) -> Mode {
    Mode {
        id: id.into(),
        name: name.into(),
        prompt: prompt.into(),
        output_language: lang.map(String::from),
        use_llm,
        builtin: true,
    }
}

pub fn builtin_modes() -> Vec<Mode> {
    vec![
        builtin("literal", "Literal", "", None, false),
        builtin(DEFAULT_MODE, "Limpio", "", None, true),
        builtin(
            "email",
            "Email",
            "Da formato de correo electrónico: saludo, cuerpo en párrafos y despedida. \
             Tono profesional y cordial. No inventes nombres: si no se menciona al \
             destinatario, usa un saludo genérico.",
            None,
            true,
        ),
        builtin(
            "message",
            "Mensaje",
            "Formato de mensaje de chat: breve, natural y directo, sin saludo formal ni firma.",
            None,
            true,
        ),
        builtin(
            "prompt",
            "Prompt técnico",
            "Reescribe el dictado como una instrucción clara y precisa para un asistente de IA \
             o de programación: en imperativo, sin relleno conversacional ni cortesías. \
             Conserva todos los requisitos técnicos y los nombres exactos de archivos, \
             funciones y valores. Usa viñetas si hay varios requisitos.",
            None,
            true,
        ),
        builtin(
            "translate-en",
            "Traducir al inglés",
            "Traduce el texto al inglés de forma natural y fiel.",
            Some("inglés"),
            true,
        ),
    ]
}

impl Mode {
    /// ¿Se pasa este dictado por Claude? Literal nunca; el resto, si la limpieza está activa.
    pub fn wants_llm(&self, cleanup_enabled: bool) -> bool {
        self.use_llm && cleanup_enabled
    }

    /// Instrucciones del modo para el limpiador.
    pub fn instructions(&self) -> String {
        let mut s = self.prompt.trim().to_string();
        if let Some(lang) = self
            .output_language
            .as_deref()
            .filter(|l| !l.trim().is_empty())
        {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(&format!("Escribe el resultado final en {}.", lang.trim()));
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeError {
    NotFound(String),
    BuiltinCannotBeDeleted,
    EmptyName,
}

impl std::fmt::Display for ModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModeError::NotFound(id) => write!(f, "El modo «{id}» no existe"),
            ModeError::BuiltinCannotBeDeleted => {
                f.write_str("Los modos predefinidos no se pueden borrar")
            }
            ModeError::EmptyName => f.write_str("El modo necesita un nombre"),
        }
    }
}

impl std::error::Error for ModeError {}

impl ModesState {
    pub fn initial() -> Self {
        Self {
            modes: builtin_modes(),
            active: DEFAULT_MODE.into(),
        }
    }

    /// Repara un estado cargado de disco: añade predefinidos que falten y un activo válido.
    pub fn normalized(mut self) -> Self {
        for b in builtin_modes() {
            if !self.modes.iter().any(|m| m.id == b.id) {
                self.modes.push(b);
            }
        }
        if !self.modes.iter().any(|m| m.id == self.active) {
            self.active = DEFAULT_MODE.into();
        }
        self
    }

    pub fn get(&self, id: &str) -> Option<&Mode> {
        self.modes.iter().find(|m| m.id == id)
    }

    pub fn active_mode(&self) -> &Mode {
        self.get(&self.active)
            .or_else(|| self.get(DEFAULT_MODE))
            .expect("el modo por defecto siempre existe")
    }

    pub fn set_active(&mut self, id: &str) -> Result<(), ModeError> {
        self.get(id).ok_or_else(|| ModeError::NotFound(id.into()))?;
        self.active = id.into();
        Ok(())
    }

    /// Pasa al siguiente modo (atajo secundario).
    pub fn cycle(&mut self) -> &Mode {
        let i = self
            .modes
            .iter()
            .position(|m| m.id == self.active)
            .unwrap_or(0);
        self.active = self.modes[(i + 1) % self.modes.len()].id.clone();
        self.active_mode()
    }

    fn new_id(&self) -> String {
        (1..)
            .map(|n| format!("custom-{n}"))
            .find(|id| self.get(id).is_none())
            .expect("ids infinitos")
    }

    /// Crea (id vacío) o actualiza un modo. Conserva `builtin` del existente.
    pub fn save(&mut self, mut mode: Mode) -> Result<String, ModeError> {
        if mode.name.trim().is_empty() {
            return Err(ModeError::EmptyName);
        }
        mode.name = mode.name.trim().into();
        if mode.id.is_empty() {
            mode.id = self.new_id();
            mode.builtin = false;
            let id = mode.id.clone();
            self.modes.push(mode);
            return Ok(id);
        }
        let existing = self
            .modes
            .iter_mut()
            .find(|m| m.id == mode.id)
            .ok_or_else(|| ModeError::NotFound(mode.id.clone()))?;
        mode.builtin = existing.builtin;
        *existing = mode;
        Ok(existing.id.clone())
    }

    pub fn duplicate(&mut self, id: &str) -> Result<String, ModeError> {
        let mut copy = self
            .get(id)
            .cloned()
            .ok_or_else(|| ModeError::NotFound(id.into()))?;
        copy.id = String::new();
        copy.name = format!("{} (copia)", copy.name);
        self.save(copy)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), ModeError> {
        let mode = self.get(id).ok_or_else(|| ModeError::NotFound(id.into()))?;
        if mode.builtin {
            return Err(ModeError::BuiltinCannotBeDeleted);
        }
        self.modes.retain(|m| m.id != id);
        if self.active == id {
            self.active = DEFAULT_MODE.into();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(name: &str) -> Mode {
        Mode {
            id: String::new(),
            name: name.into(),
            prompt: "Hazlo breve.".into(),
            output_language: None,
            use_llm: true,
            builtin: true, // se ignora al crear
        }
    }

    #[test]
    fn six_builtins_with_literal_without_llm() {
        let s = ModesState::initial();
        let ids: Vec<_> = s.modes.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "literal",
                "clean",
                "email",
                "message",
                "prompt",
                "translate-en"
            ]
        );
        assert!(!s.get("literal").unwrap().use_llm);
        assert!(
            s.modes
                .iter()
                .filter(|m| m.id != "literal")
                .all(|m| m.use_llm)
        );
        assert_eq!(s.active_mode().id, DEFAULT_MODE);
    }

    #[test]
    fn literal_never_calls_llm_and_global_switch_disables_all() {
        let s = ModesState::initial();
        assert!(!s.get("literal").unwrap().wants_llm(true));
        assert!(s.get("email").unwrap().wants_llm(true));
        assert!(s.modes.iter().all(|m| !m.wants_llm(false)));
    }

    #[test]
    fn instructions_include_prompt_and_output_language() {
        let s = ModesState::initial();
        assert_eq!(s.get("clean").unwrap().instructions(), "");
        let t = s.get("translate-en").unwrap().instructions();
        assert!(t.starts_with("Traduce el texto al inglés"));
        assert!(t.ends_with("Escribe el resultado final en inglés."));
        assert!(s.get("email").unwrap().instructions().contains("correo"));
    }

    #[test]
    fn create_update_duplicate_delete() {
        let mut s = ModesState::initial();
        let id = s.save(custom("  Notas  ")).unwrap();
        let m = s.get(&id).unwrap().clone();
        assert_eq!(m.name, "Notas");
        assert!(!m.builtin);

        s.save(Mode {
            prompt: "Otro".into(),
            ..m.clone()
        })
        .unwrap();
        assert_eq!(s.get(&id).unwrap().prompt, "Otro");

        let dup = s.duplicate(&id).unwrap();
        assert_ne!(dup, id);
        assert_eq!(s.get(&dup).unwrap().name, "Notas (copia)");

        s.set_active(&id).unwrap();
        s.delete(&id).unwrap();
        assert!(s.get(&id).is_none());
        assert_eq!(s.active, DEFAULT_MODE);
    }

    #[test]
    fn builtins_can_be_edited_but_not_deleted() {
        let mut s = ModesState::initial();
        let mut email = s.get("email").unwrap().clone();
        email.prompt = "Muy formal.".into();
        email.builtin = false; // el frontend no puede quitar la marca
        s.save(email).unwrap();
        assert!(s.get("email").unwrap().builtin);
        assert_eq!(s.get("email").unwrap().prompt, "Muy formal.");
        assert_eq!(s.delete("email"), Err(ModeError::BuiltinCannotBeDeleted));
    }

    #[test]
    fn validation_errors() {
        let mut s = ModesState::initial();
        assert_eq!(s.save(custom("   ")), Err(ModeError::EmptyName));
        assert_eq!(
            s.set_active("nope"),
            Err(ModeError::NotFound("nope".into()))
        );
        assert!(matches!(s.duplicate("nope"), Err(ModeError::NotFound(_))));
    }

    #[test]
    fn cycle_wraps_around() {
        let mut s = ModesState::initial();
        s.set_active("translate-en").unwrap();
        assert_eq!(s.cycle().id, "literal");
        assert_eq!(s.cycle().id, "clean");
    }

    #[test]
    fn normalized_restores_missing_builtins_and_active() {
        let s = ModesState {
            modes: vec![],
            active: "borrado".into(),
        }
        .normalized();
        assert_eq!(s.modes.len(), 6);
        assert_eq!(s.active, DEFAULT_MODE);
    }
}
