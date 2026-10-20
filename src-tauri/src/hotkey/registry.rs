//! Registro de atajos con detección de conflictos.

use std::fmt;

pub const ESCAPE: &str = "Escape";

/// Atajo secundario para pasar al siguiente modo.
pub const fn mode_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "Ctrl+Option+M"
    } else {
        "Ctrl+Alt+M"
    }
}

/// Atajo por defecto de la plataforma.
pub const fn default_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "Option+Space"
    } else {
        "Ctrl+Alt+Space"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyError {
    /// El texto no es un atajo válido.
    Invalid(String),
    /// Otra app (o el SO) ya usa el atajo.
    Conflict(String),
    Other(String),
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HotkeyError::Invalid(s) => write!(f, "«{s}» no es un atajo válido"),
            HotkeyError::Conflict(s) => {
                write!(f, "El atajo «{s}» ya está en uso por otra aplicación")
            }
            HotkeyError::Other(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for HotkeyError {}

/// Abstracción sobre el registro de atajos globales del SO.
pub trait ShortcutRegistrar {
    fn register(&mut self, shortcut: &str) -> Result<(), HotkeyError>;
    fn unregister(&mut self, shortcut: &str) -> Result<(), HotkeyError>;
}

/// Mantiene registrado el atajo principal y gestiona Esc solo durante la grabación,
/// para no robarle Esc al resto de apps.
pub struct HotkeyRegistry<R> {
    registrar: R,
    current: Option<String>,
    escape_registered: bool,
}

impl<R: ShortcutRegistrar> HotkeyRegistry<R> {
    pub fn new(registrar: R) -> Self {
        Self {
            registrar,
            current: None,
            escape_registered: false,
        }
    }

    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Cambia el atajo principal. Si el nuevo choca, se conserva el anterior.
    pub fn set_shortcut(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        if self.current.as_deref() == Some(shortcut) {
            return Ok(());
        }
        self.registrar.register(shortcut)?;
        if let Some(old) = self.current.replace(shortcut.to_string()) {
            self.registrar.unregister(&old)?;
        }
        Ok(())
    }

    pub fn set_escape(&mut self, active: bool) -> Result<(), HotkeyError> {
        if active == self.escape_registered {
            return Ok(());
        }
        if active {
            self.registrar.register(ESCAPE)?;
        } else {
            self.registrar.unregister(ESCAPE)?;
        }
        self.escape_registered = active;
        Ok(())
    }

    #[cfg(test)]
    pub fn registrar(&self) -> &R {
        &self.registrar
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Simula el SO: `taken` son atajos usados por otras apps.
    #[derive(Default)]
    struct FakeRegistrar {
        registered: HashSet<String>,
        taken: HashSet<String>,
    }

    impl ShortcutRegistrar for FakeRegistrar {
        fn register(&mut self, s: &str) -> Result<(), HotkeyError> {
            if s.is_empty() {
                return Err(HotkeyError::Invalid(s.into()));
            }
            if self.taken.contains(s) || !self.registered.insert(s.into()) {
                return Err(HotkeyError::Conflict(s.into()));
            }
            Ok(())
        }
        fn unregister(&mut self, s: &str) -> Result<(), HotkeyError> {
            self.registered.remove(s);
            Ok(())
        }
    }

    fn registry(taken: &[&str]) -> HotkeyRegistry<FakeRegistrar> {
        HotkeyRegistry::new(FakeRegistrar {
            taken: taken.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        })
    }

    #[test]
    fn registers_default_shortcut() {
        let mut r = registry(&[]);
        r.set_shortcut(default_shortcut()).unwrap();
        assert_eq!(r.current(), Some(default_shortcut()));
        assert!(r.registrar().registered.contains(default_shortcut()));
    }

    #[test]
    fn conflicting_shortcut_returns_conflict_and_keeps_previous() {
        let mut r = registry(&["Ctrl+Shift+D"]);
        r.set_shortcut("Ctrl+Alt+Space").unwrap();
        let err = r.set_shortcut("Ctrl+Shift+D").unwrap_err();
        assert_eq!(err, HotkeyError::Conflict("Ctrl+Shift+D".into()));
        assert!(err.to_string().contains("ya está en uso"));
        assert_eq!(r.current(), Some("Ctrl+Alt+Space"));
        assert!(r.registrar().registered.contains("Ctrl+Alt+Space"));
    }

    #[test]
    fn changing_shortcut_unregisters_old_one() {
        let mut r = registry(&[]);
        r.set_shortcut("Ctrl+Alt+Space").unwrap();
        r.set_shortcut("Ctrl+Alt+R").unwrap();
        assert!(!r.registrar().registered.contains("Ctrl+Alt+Space"));
        assert!(r.registrar().registered.contains("Ctrl+Alt+R"));
    }

    #[test]
    fn same_shortcut_twice_is_noop() {
        let mut r = registry(&[]);
        r.set_shortcut("Ctrl+Alt+Space").unwrap();
        assert!(r.set_shortcut("Ctrl+Alt+Space").is_ok());
    }

    #[test]
    fn escape_registered_only_while_active() {
        let mut r = registry(&[]);
        r.set_escape(true).unwrap();
        assert!(r.registrar().registered.contains(ESCAPE));
        r.set_escape(true).unwrap(); // idempotente, sin conflicto consigo mismo
        r.set_escape(false).unwrap();
        assert!(!r.registrar().registered.contains(ESCAPE));
    }

    #[test]
    fn default_shortcut_matches_platform() {
        #[cfg(target_os = "macos")]
        assert_eq!(default_shortcut(), "Option+Space");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(default_shortcut(), "Ctrl+Alt+Space");
    }
}
