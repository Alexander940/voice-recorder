//! Entrega del texto: portapapeles y pegado en la app activa (HU-06).

mod system;

pub use system::{EnigoKeys, SystemClipboard, TauriNotifier};

use std::thread;
use std::time::Duration;

/// Tiempo que se deja a la app destino para leer el portapapeles antes de restaurarlo.
pub const RESTORE_DELAY: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputError(pub String);

impl std::fmt::Display for OutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OutputError {}

pub trait Clipboard {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str) -> Result<(), OutputError>;
    fn clear(&mut self) -> Result<(), OutputError>;
}

pub trait KeySender {
    /// Simula el atajo de pegar de la plataforma (Ctrl+V / Cmd+V).
    fn paste(&mut self) -> Result<(), OutputError>;
}

pub trait Notifier {
    fn notify(&self, title: &str, body: &str);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// Pegar en la app activa y restaurar el portapapeles.
    #[default]
    Paste,
    /// Solo dejar el texto en el portapapeles.
    CopyOnly,
}

/// Resultado de una entrega.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Pasted,
    /// El texto quedó en el portapapeles (por modo o por fallo del pegado).
    InClipboard,
}

/// Modificador del atajo de pegar en la plataforma actual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteModifier {
    Control,
    Command,
}

pub const fn paste_modifier() -> PasteModifier {
    if cfg!(target_os = "macos") {
        PasteModifier::Command
    } else {
        PasteModifier::Control
    }
}

pub struct Paster<C, K, N> {
    pub clipboard: C,
    pub keys: K,
    pub notifier: N,
    pub restore_delay: Duration,
}

impl<C: Clipboard, K: KeySender, N: Notifier> Paster<C, K, N> {
    pub fn new(clipboard: C, keys: K, notifier: N) -> Self {
        Self {
            clipboard,
            keys,
            notifier,
            restore_delay: RESTORE_DELAY,
        }
    }

    /// Entrega `text` según `mode`. Si el pegado falla, el texto se queda en el
    /// portapapeles (sin restaurar) y se avisa al usuario.
    pub fn deliver(&mut self, text: &str, mode: OutputMode) -> Result<Delivery, OutputError> {
        if mode == OutputMode::CopyOnly {
            self.clipboard.set_text(text)?;
            return Ok(Delivery::InClipboard);
        }

        let previous = self.clipboard.get_text();
        self.clipboard.set_text(text)?;

        if let Err(e) = self.keys.paste() {
            self.notifier.notify(
                "No se pudo pegar",
                &format!("El texto está en el portapapeles. ({e})"),
            );
            return Ok(Delivery::InClipboard);
        }

        thread::sleep(self.restore_delay);
        match previous {
            Some(prev) => self.clipboard.set_text(&prev)?,
            None => self.clipboard.clear()?,
        }
        Ok(Delivery::Pasted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<String>>>;

    struct FakeClipboard {
        log: Log,
        content: Option<String>,
    }

    impl Clipboard for FakeClipboard {
        fn get_text(&mut self) -> Option<String> {
            self.log.borrow_mut().push("get".into());
            self.content.clone()
        }
        fn set_text(&mut self, text: &str) -> Result<(), OutputError> {
            self.log.borrow_mut().push(format!("set:{text}"));
            self.content = Some(text.into());
            Ok(())
        }
        fn clear(&mut self) -> Result<(), OutputError> {
            self.log.borrow_mut().push("clear".into());
            self.content = None;
            Ok(())
        }
    }

    struct FakeKeys {
        log: Log,
        fail: bool,
    }

    impl KeySender for FakeKeys {
        fn paste(&mut self) -> Result<(), OutputError> {
            self.log.borrow_mut().push("paste".into());
            if self.fail {
                Err(OutputError("acceso denegado".into()))
            } else {
                Ok(())
            }
        }
    }

    struct FakeNotifier {
        log: Log,
    }

    impl Notifier for FakeNotifier {
        fn notify(&self, title: &str, _body: &str) {
            self.log.borrow_mut().push(format!("notify:{title}"));
        }
    }

    fn paster(
        previous: Option<&str>,
        paste_fails: bool,
    ) -> (Paster<FakeClipboard, FakeKeys, FakeNotifier>, Log) {
        let log: Log = Rc::default();
        let mut p = Paster::new(
            FakeClipboard {
                log: log.clone(),
                content: previous.map(String::from),
            },
            FakeKeys {
                log: log.clone(),
                fail: paste_fails,
            },
            FakeNotifier { log: log.clone() },
        );
        p.restore_delay = Duration::ZERO;
        (p, log)
    }

    #[test]
    fn paste_saves_sets_pastes_then_restores_in_order() {
        let (mut p, log) = paster(Some("anterior"), false);
        assert_eq!(p.deliver("hola", OutputMode::Paste), Ok(Delivery::Pasted));
        assert_eq!(*log.borrow(), ["get", "set:hola", "paste", "set:anterior"]);
        assert_eq!(p.clipboard.content.as_deref(), Some("anterior"));
    }

    #[test]
    fn empty_previous_clipboard_is_cleared_after_paste() {
        let (mut p, log) = paster(None, false);
        p.deliver("hola", OutputMode::Paste).unwrap();
        assert_eq!(*log.borrow(), ["get", "set:hola", "paste", "clear"]);
    }

    #[test]
    fn failed_paste_keeps_text_and_notifies_without_restoring() {
        let (mut p, log) = paster(Some("anterior"), true);
        assert_eq!(
            p.deliver("hola", OutputMode::Paste),
            Ok(Delivery::InClipboard)
        );
        assert_eq!(
            *log.borrow(),
            ["get", "set:hola", "paste", "notify:No se pudo pegar"]
        );
        assert_eq!(p.clipboard.content.as_deref(), Some("hola"));
    }

    #[test]
    fn copy_only_sets_clipboard_without_pasting() {
        let (mut p, log) = paster(Some("anterior"), false);
        assert_eq!(
            p.deliver("hola", OutputMode::CopyOnly),
            Ok(Delivery::InClipboard)
        );
        assert_eq!(*log.borrow(), ["set:hola"]);
    }

    #[test]
    fn paste_modifier_matches_platform() {
        #[cfg(target_os = "macos")]
        assert_eq!(paste_modifier(), PasteModifier::Command);
        #[cfg(not(target_os = "macos"))]
        assert_eq!(paste_modifier(), PasteModifier::Control);
    }
}
