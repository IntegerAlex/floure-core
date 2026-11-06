//! Transcript history: schema, FTS index, and queries.
//!
//! The database is the same file the Tauri app used, so existing installs keep
//! their history. Every migration here must be idempotent.

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptRow {
    pub id: i64,
    pub raw_text: String,
    pub processed_text: String,
    pub language: String,
    pub mode: String,
    pub model: String,
    pub duration_sec: f64,
    pub favorite: i64,
    pub created_at: String,
}

/// Create the transcript table and its external-content FTS index.
pub fn ensure_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS transcripts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            raw_text TEXT NOT NULL,
            processed_text TEXT NOT NULL DEFAULT '',
            language TEXT DEFAULT '',
            mode TEXT DEFAULT 'cleanup',
            favorite INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            model TEXT DEFAULT '',
            duration_sec REAL DEFAULT 0.0
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS transcripts_fts USING fts5(
            raw_text,
            processed_text,
            content='transcripts',
            content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS transcripts_ai AFTER INSERT ON transcripts BEGIN
            INSERT INTO transcripts_fts(rowid, raw_text, processed_text)
            VALUES (new.id, new.raw_text, new.processed_text);
        END;

        CREATE TRIGGER IF NOT EXISTS transcripts_ad AFTER DELETE ON transcripts BEGIN
            INSERT INTO transcripts_fts(transcripts_fts, rowid, raw_text, processed_text)
            VALUES ('delete', old.id, old.raw_text, old.processed_text);
        END;

        CREATE TRIGGER IF NOT EXISTS transcripts_au AFTER UPDATE ON transcripts BEGIN
            INSERT INTO transcripts_fts(transcripts_fts, rowid, raw_text, processed_text)
            VALUES ('delete', old.id, old.raw_text, old.processed_text);
            INSERT INTO transcripts_fts(rowid, raw_text, processed_text)
            VALUES (new.id, new.raw_text, new.processed_text);
        END;",
    )?;

    // Backfill rows written before the FTS table existed exactly once. The
    // triggers keep new writes current, and running a full-table anti-join on
    // every connection makes search slower as history grows.
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        conn.execute(
            "INSERT INTO transcripts_fts(transcripts_fts) VALUES('rebuild')",
            [],
        )?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    Ok(())
}

pub fn open(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// Repository facade for the History screen. It opens the database per call so
/// the GPUI thread never holds a connection across an await point.
pub struct History {
    db: std::path::PathBuf,
}

impl History {
    pub fn new(db: impl Into<std::path::PathBuf>) -> Self {
        Self { db: db.into() }
    }

    fn conn(&self) -> Result<Connection> {
        open(&self.db)
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<TranscriptRow>> {
        recent(&self.conn()?, limit)
    }

    /// Local calendar dates used for Today/Yesterday headings. `created_at`
    /// stays UTC on disk; only these labels are local.
    pub fn today_and_yesterday(&self) -> (String, String) {
        let conn = match self.conn() {
            Ok(conn) => conn,
            Err(_) => return (String::new(), String::new()),
        };
        conn.query_row(
            "SELECT date('now', 'localtime'), date('now', 'localtime', '-1 day')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or_default()
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<TranscriptRow>> {
        search(&self.conn()?, query, limit)
    }

    pub fn delete(&self, ids: &[i64]) -> Result<usize> {
        let conn = self.conn()?;
        let mut deleted = 0;
        for id in ids {
            deleted += conn.execute("DELETE FROM transcripts WHERE id = ?1", [id])?;
        }
        Ok(deleted)
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> Result<bool> {
        let conn = self.conn()?;
        Ok(conn.execute(
            "UPDATE transcripts SET favorite = ?1 WHERE id = ?2",
            rusqlite::params![favorite as i64, id],
        )? > 0)
    }

    /// Export the given rows as CSV. The caller passes the already-filtered
    /// rows so the export always matches what the screen shows.
    pub fn export_csv(&self, rows: &[TranscriptRow]) -> String {
        let mut out = String::from("created_at,language,mode,model,duration_sec,text\n");
        for row in rows {
            out.push_str(&format!(
                "{},\"{}\",\"{}\",\"{}\",{:.2},\"{}\"\n",
                row.created_at,
                row.language,
                row.mode,
                row.model,
                row.duration_sec,
                row.processed_text.replace('"', "\"\"")
            ));
        }
        out
    }
}

pub fn recent(conn: &Connection, limit: usize) -> Result<Vec<TranscriptRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, raw_text, processed_text, language, mode, model, duration_sec, favorite, created_at
         FROM transcripts ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |row| {
        Ok(TranscriptRow {
            id: row.get(0)?,
            raw_text: row.get(1)?,
            processed_text: row.get(2)?,
            language: row.get(3)?,
            mode: row.get(4)?,
            model: row.get(5)?,
            duration_sec: row.get(6)?,
            favorite: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<TranscriptRow>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.raw_text, t.processed_text, t.language, t.mode, t.model,
                t.duration_sec, t.favorite, t.created_at
         FROM transcripts_fts JOIN transcripts t ON t.id = transcripts_fts.rowid
         WHERE transcripts_fts MATCH ?1
         ORDER BY rank LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![query, limit as i64], |row| {
        Ok(TranscriptRow {
            id: row.get(0)?,
            raw_text: row.get(1)?,
            processed_text: row.get(2)?,
            language: row.get(3)?,
            mode: row.get(4)?,
            model: row.get(5)?,
            duration_sec: row.get(6)?,
            favorite: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn schema_backfills_and_search_matches() {
        let conn = db();
        conn.execute(
            "INSERT INTO transcripts(raw_text, processed_text, language, mode, model, duration_sec)
             VALUES ('hello world', 'hello world', 'en', 'cleanup', 'parakeet', 1.5)",
            [],
        )
        .unwrap();
        let hits = search(&conn, "hello", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].raw_text, "hello world");
    }

    #[test]
    fn export_only_contains_the_rows_the_screen_shows() {
        let conn = db();
        conn.execute(
            "INSERT INTO transcripts(raw_text, processed_text, language, mode, model, duration_sec)
             VALUES ('keep me', 'keep me', 'en', 'cleanup', 'parakeet', 1.0), ('hide me', 'hide me', 'en', 'cleanup', 'parakeet', 1.0)",
            [],
        )
        .unwrap();
        let rows = search(&conn, "keep", 10).unwrap();
        let csv = History::new("unused").export_csv(&rows);
        assert!(csv.contains("keep me"));
        assert!(!csv.contains("hide me"));
        assert_eq!(csv.lines().count(), 2, "header plus one row");
    }

    #[test]
    fn schema_is_idempotent_and_backfills_once() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO transcripts(raw_text, processed_text) VALUES ('prefts row', 'prefts row')",
            [],
        )
        .unwrap();
        // Simulate a legacy database whose index was never built, then let the
        // one-shot backfill repair it.
        conn.execute("DROP TABLE transcripts_fts", []).unwrap();
        conn.pragma_update(None, "user_version", 0).unwrap();
        ensure_schema(&conn).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
        assert_eq!(search(&conn, "prefts", 10).unwrap().len(), 1);
        ensure_schema(&conn).unwrap();
        assert_eq!(search(&conn, "prefts", 10).unwrap().len(), 1);
    }
}
