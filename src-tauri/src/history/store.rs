//! Repositorio SQLite del historial.

use std::path::Path;

use rusqlite::{Connection, params};

/// Versión actual del esquema (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    /// Milisegundos desde la época Unix.
    pub created_at: i64,
    pub raw_text: String,
    pub final_text: String,
    pub mode: String,
    pub duration_ms: i64,
    pub stt_latency_ms: i64,
    pub llm_latency_ms: Option<i64>,
}

/// Datos de una entrada nueva (sin id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntry {
    pub created_at: i64,
    pub raw_text: String,
    pub final_text: String,
    pub mode: String,
    pub duration_ms: i64,
    pub stt_latency_ms: i64,
    pub llm_latency_ms: Option<i64>,
}

pub struct HistoryStore {
    conn: Connection,
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS entries (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 created_at INTEGER NOT NULL,
                 raw_text TEXT NOT NULL,
                 final_text TEXT NOT NULL,
                 mode TEXT NOT NULL,
                 duration_ms INTEGER NOT NULL,
                 stt_latency_ms INTEGER NOT NULL,
                 llm_latency_ms INTEGER
             );
             CREATE INDEX IF NOT EXISTS entries_created_at ON entries(created_at);
             PRAGMA user_version = 1;
             COMMIT;",
        )?;
    }
    Ok(())
}

fn row_to_entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: r.get(0)?,
        created_at: r.get(1)?,
        raw_text: r.get(2)?,
        final_text: r.get(3)?,
        mode: r.get(4)?,
        duration_ms: r.get(5)?,
        stt_latency_ms: r.get(6)?,
        llm_latency_ms: r.get(7)?,
    })
}

const COLUMNS: &str =
    "id, created_at, raw_text, final_text, mode, duration_ms, stt_latency_ms, llm_latency_ms";

impl HistoryStore {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> rusqlite::Result<Self> {
        migrate(&conn)?;
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> rusqlite::Result<i64> {
        self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))
    }

    pub fn insert(&self, e: &NewEntry) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO entries (created_at, raw_text, final_text, mode, duration_ms, stt_latency_ms, llm_latency_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                e.created_at,
                e.raw_text,
                e.final_text,
                e.mode,
                e.duration_ms,
                e.stt_latency_ms,
                e.llm_latency_ms
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get(&self, id: i64) -> rusqlite::Result<Option<HistoryEntry>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {COLUMNS} FROM entries WHERE id = ?1"))?;
        let mut rows = stmt.query_map([id], row_to_entry)?;
        rows.next().transpose()
    }

    /// Más recientes primero. `query` busca en el texto final y en el bruto.
    pub fn search(&self, query: &str, limit: u32) -> rusqlite::Result<Vec<HistoryEntry>> {
        let pattern = format!(
            "%{}%",
            query
                .trim()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM entries
             WHERE final_text LIKE ?1 ESCAPE '\\' OR raw_text LIKE ?1 ESCAPE '\\'
             ORDER BY created_at DESC, id DESC LIMIT ?2"
        ))?;
        stmt.query_map(params![pattern, limit], row_to_entry)?
            .collect()
    }

    pub fn delete(&self, id: i64) -> rusqlite::Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM entries WHERE id = ?1", [id])?
            > 0)
    }

    /// Borra las entradas anteriores a `cutoff_ms`. Devuelve cuántas.
    pub fn purge_older_than(&self, cutoff_ms: i64) -> rusqlite::Result<usize> {
        self.conn
            .execute("DELETE FROM entries WHERE created_at < ?1", [cutoff_ms])
    }

    pub fn count(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000;

    fn entry(created_at: i64, text: &str) -> NewEntry {
        NewEntry {
            created_at,
            raw_text: format!("eh {text}"),
            final_text: text.into(),
            mode: "clean".into(),
            duration_ms: 1500,
            stt_latency_ms: 900,
            llm_latency_ms: Some(700),
        }
    }

    #[test]
    fn empty_database_migrates_to_current_schema() {
        let s = HistoryStore::in_memory().unwrap();
        assert_eq!(s.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(s.count().unwrap(), 0);
    }

    #[test]
    fn reopening_a_migrated_file_keeps_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("h.sqlite3");
        let id = HistoryStore::open(&path)
            .unwrap()
            .insert(&entry(1, "hola"))
            .unwrap();
        let s = HistoryStore::open(&path).unwrap();
        assert_eq!(s.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(s.get(id).unwrap().unwrap().final_text, "hola");
    }

    #[test]
    fn insert_and_get_roundtrip() {
        let s = HistoryStore::in_memory().unwrap();
        let id = s.insert(&entry(10, "Hola mundo.")).unwrap();
        let e = s.get(id).unwrap().unwrap();
        assert_eq!(e.final_text, "Hola mundo.");
        assert_eq!(e.raw_text, "eh Hola mundo.");
        assert_eq!(e.llm_latency_ms, Some(700));
        assert_eq!(s.get(id + 99).unwrap(), None);
    }

    #[test]
    fn search_matches_final_and_raw_text_newest_first() {
        let s = HistoryStore::in_memory().unwrap();
        s.insert(&entry(1, "Reunión con ventas")).unwrap();
        s.insert(&entry(2, "Comprar pan")).unwrap();
        s.insert(&entry(3, "Revisar ventas Q3")).unwrap();
        let found: Vec<_> = s
            .search("VENTAS", 50)
            .unwrap()
            .into_iter()
            .map(|e| e.final_text)
            .collect();
        assert_eq!(found, ["Revisar ventas Q3", "Reunión con ventas"]);
        assert_eq!(s.search("eh", 50).unwrap().len(), 3);
        assert_eq!(s.search("", 2).unwrap().len(), 2);
    }

    #[test]
    fn search_treats_wildcards_literally() {
        let s = HistoryStore::in_memory().unwrap();
        s.insert(&entry(1, "100% listo")).unwrap();
        s.insert(&entry(2, "nada")).unwrap();
        assert_eq!(s.search("%", 50).unwrap().len(), 1);
        assert_eq!(s.search("_", 50).unwrap().len(), 0);
    }

    #[test]
    fn delete_removes_one_entry() {
        let s = HistoryStore::in_memory().unwrap();
        let a = s.insert(&entry(1, "a")).unwrap();
        s.insert(&entry(2, "b")).unwrap();
        assert!(s.delete(a).unwrap());
        assert!(!s.delete(a).unwrap());
        assert_eq!(s.count().unwrap(), 1);
    }

    #[test]
    fn purge_respects_retention_with_fixed_clock() {
        let s = HistoryStore::in_memory().unwrap();
        let now = 100 * DAY;
        s.insert(&entry(now - 31 * DAY, "viejo")).unwrap();
        s.insert(&entry(now - 29 * DAY, "reciente")).unwrap();
        s.insert(&entry(now, "hoy")).unwrap();
        assert_eq!(s.purge_older_than(now - 30 * DAY).unwrap(), 1);
        let left: Vec<_> = s
            .search("", 50)
            .unwrap()
            .into_iter()
            .map(|e| e.final_text)
            .collect();
        assert_eq!(left, ["hoy", "reciente"]);
    }
}
