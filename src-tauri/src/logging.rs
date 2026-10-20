//! Logs rotativos por tamaño, sin datos sensibles (HU-13).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use regex::Regex;

pub const MAX_LOG_BYTES: u64 = 1024 * 1024;
pub const MAX_LOG_FILES: usize = 3;
/// Cuántos textos dictados recientes se ocultan si aparecen en un log.
const MAX_REDACTIONS: usize = 20;

fn secret_patterns() -> &'static [(Regex, &'static str)] {
    static P: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    P.get_or_init(|| {
        vec![
            (Regex::new(r"gsk_[A-Za-z0-9]+").unwrap(), "[key]"),
            (Regex::new(r"sk-ant-[A-Za-z0-9_\-]+").unwrap(), "[key]"),
            (Regex::new(r"xai-[A-Za-z0-9]+").unwrap(), "[key]"),
            (
                Regex::new(r"(?i)([A-Z]:\\Users\\)[^\\\s]+").unwrap(),
                "${1}[usuario]",
            ),
            (
                Regex::new(r"(/Users/|/home/)[^/\s]+").unwrap(),
                "${1}[usuario]",
            ),
        ]
    })
}

/// Textos dictados recientes que nunca deben aparecer en los logs.
#[derive(Default, Clone)]
pub struct Redactions(Arc<Mutex<Vec<String>>>);

impl Redactions {
    pub fn add(&self, text: &str) {
        let text = text.trim();
        if text.len() < 4 {
            return;
        }
        let mut list = self.0.lock().expect("redactions poisoned");
        list.push(text.to_string());
        if list.len() > MAX_REDACTIONS {
            list.remove(0);
        }
    }

    fn snapshot(&self) -> Vec<String> {
        self.0.lock().expect("redactions poisoned").clone()
    }
}

/// Oculta keys, rutas de usuario y textos dictados registrados.
pub fn scrub(line: &str, redactions: &Redactions) -> String {
    let mut out = line.to_string();
    for text in redactions.snapshot() {
        out = out.replace(&text, "[texto]");
    }
    for (re, replacement) in secret_patterns() {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

/// Escritor que rota `app.log` → `app.log.1` → … al superar `max_bytes`.
pub struct RotatingWriter {
    path: PathBuf,
    max_bytes: u64,
    max_files: usize,
    file: Option<File>,
    written: u64,
    redactions: Redactions,
}

impl RotatingWriter {
    pub fn new(
        path: impl Into<PathBuf>,
        max_bytes: u64,
        max_files: usize,
        redactions: Redactions,
    ) -> Self {
        let path = path.into();
        let written = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Self {
            path,
            max_bytes,
            max_files,
            file: None,
            written,
            redactions,
        }
    }

    fn rotated(path: &Path, n: usize) -> PathBuf {
        PathBuf::from(format!("{}.{n}", path.display()))
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file = None;
        let _ = fs::remove_file(Self::rotated(&self.path, self.max_files));
        for n in (1..self.max_files).rev() {
            let from = Self::rotated(&self.path, n);
            if from.exists() {
                fs::rename(&from, Self::rotated(&self.path, n + 1))?;
            }
        }
        if self.path.exists() {
            fs::rename(&self.path, Self::rotated(&self.path, 1))?;
        }
        self.written = 0;
        Ok(())
    }

    fn file(&mut self) -> io::Result<&mut File> {
        if self.file.is_none() {
            if let Some(dir) = self.path.parent() {
                fs::create_dir_all(dir)?;
            }
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?,
            );
        }
        Ok(self.file.as_mut().expect("abierto"))
    }
}

impl Write for RotatingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let clean = scrub(&String::from_utf8_lossy(buf), &self.redactions);
        if self.written + clean.len() as u64 > self.max_bytes && self.written > 0 {
            self.rotate()?;
        }
        self.file()?.write_all(clean.as_bytes())?;
        self.written += clean.len() as u64;
        // Se informa la longitud original: el filtrado es transparente para tracing.
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.file {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

/// Instala el subscriber global de `tracing` escribiendo en `dir/app.log`.
pub fn init(dir: &Path, redactions: Redactions) {
    let writer = Arc::new(Mutex::new(RotatingWriter::new(
        dir.join("app.log"),
        MAX_LOG_BYTES,
        MAX_LOG_FILES,
        redactions,
    )));
    let make_writer = move || SharedWriter(writer.clone());
    let _ = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(make_writer)
        .try_init();
}

struct SharedWriter(Arc<Mutex<RotatingWriter>>);

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("log poisoned").write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().expect("log poisoned").flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_hides_keys_paths_and_dictated_text() {
        let r = Redactions::default();
        r.add("Mañana firmo el contrato con Juan");
        let line = r#"error key=gsk_ABC123def sk-ant-api03-XyZ_9 xai-QQQ path=C:\Users\alexander\AppData\x mac=/Users/maria/Library texto="Mañana firmo el contrato con Juan""#;
        let out = scrub(line, &r);
        for leaked in [
            "gsk_ABC",
            "sk-ant-api03",
            "xai-QQQ",
            "alexander",
            "maria",
            "contrato",
        ] {
            assert!(!out.contains(leaked), "{leaked} visible en: {out}");
        }
        assert!(out.contains(r"C:\Users\[usuario]\AppData"));
        assert!(out.contains("/Users/[usuario]/Library"));
        assert!(out.contains("[texto]"));
    }

    #[test]
    fn short_texts_are_not_registered_and_list_is_bounded() {
        let r = Redactions::default();
        r.add("ok");
        assert!(r.snapshot().is_empty());
        for i in 0..30 {
            r.add(&format!("dictado número {i}"));
        }
        let s = r.snapshot();
        assert_eq!(s.len(), MAX_REDACTIONS);
        assert_eq!(s[0], "dictado número 10");
    }

    #[test]
    fn writer_rotates_and_respects_max_size_and_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.log");
        let mut w = RotatingWriter::new(&path, 100, 2, Redactions::default());
        let line = format!("{}\n", "x".repeat(39)); // 40 bytes
        for _ in 0..10 {
            w.write_all(line.as_bytes()).unwrap();
        }
        w.flush().unwrap();
        for p in [
            path.clone(),
            RotatingWriter::rotated(&path, 1),
            RotatingWriter::rotated(&path, 2),
        ] {
            assert!(
                fs::metadata(&p).unwrap().len() <= 100,
                "{} demasiado grande",
                p.display()
            );
        }
        assert!(
            !RotatingWriter::rotated(&path, 3).exists(),
            "se superó el número de archivos"
        );
    }

    #[test]
    fn writer_scrubs_before_writing_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.log");
        let r = Redactions::default();
        r.add("texto secreto del usuario");
        let mut w = RotatingWriter::new(&path, MAX_LOG_BYTES, 3, r);
        w.write_all(b"key=gsk_SECRET texto secreto del usuario\n")
            .unwrap();
        w.flush().unwrap();
        let disk = fs::read_to_string(&path).unwrap();
        assert!(!disk.contains("gsk_SECRET"));
        assert!(!disk.contains("secreto"));
    }
}
