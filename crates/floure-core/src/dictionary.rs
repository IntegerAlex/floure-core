//! User dictionary: corrections that bias decoding and replace phrases.
//!
//! Entries live in the same `history.db` file as transcripts. The pipeline
//! already reads this table through `config::with_dictionary_hotwords`; this
//! module is the repository the Configuration screen edits.

use std::path::PathBuf;

use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub id: i64,
    pub phrase: String,
    pub replacement: String,
    pub category: String,
    pub notes: String,
    pub use_count: i64,
    pub is_favorite: bool,
    pub auto_learned: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub fn ensure_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS dictionary_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            phrase TEXT NOT NULL UNIQUE,
            replacement TEXT NOT NULL,
            category TEXT DEFAULT 'custom',
            notes TEXT DEFAULT '',
            use_count INTEGER DEFAULT 0,
            is_favorite INTEGER DEFAULT 0,
            auto_learned INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )
}

pub struct Dictionary {
    db: PathBuf,
}

impl Dictionary {
    pub fn new(db: impl Into<PathBuf>) -> Self {
        Self { db: db.into() }
    }

    fn conn(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db)?;
        ensure_schema(&conn)?;
        Ok(conn)
    }

    pub fn list(
        &self,
        search: Option<&str>,
        category: Option<&str>,
        favorites_only: bool,
    ) -> Result<Vec<DictionaryEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, phrase, replacement, category, notes, use_count, is_favorite,
                    auto_learned, created_at, updated_at
             FROM dictionary_entries ORDER BY is_favorite DESC, updated_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_entry)?;
        let search = search
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());
        let category = category
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty());
        Ok(rows
            .filter_map(|row| row.ok())
            .filter(|entry| {
                search
                    .as_ref()
                    .is_none_or(|s| entry.phrase.to_lowercase().contains(s))
                    && category.as_ref().is_none_or(|c| &entry.category == c)
                    && (!favorites_only || entry.is_favorite)
            })
            .collect())
    }

    pub fn add(
        &self,
        phrase: &str,
        replacement: &str,
        category: Option<&str>,
        notes: Option<&str>,
    ) -> Result<Option<DictionaryEntry>> {
        let phrase = phrase.trim();
        let replacement = replacement.trim();
        if !valid(phrase) || !valid(replacement) {
            return Ok(None);
        }
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO dictionary_entries (phrase, replacement, category, notes)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                phrase,
                replacement,
                category.unwrap_or("custom"),
                notes.unwrap_or_default()
            ],
        )?;
        let id = conn.last_insert_rowid();
        if id == 0 {
            return Ok(None);
        }
        Ok(Some(self.get(&conn, id)?))
    }

    pub fn update(
        &self,
        id: i64,
        phrase: Option<&str>,
        replacement: Option<&str>,
        category: Option<&str>,
        notes: Option<&str>,
    ) -> Result<Option<DictionaryEntry>> {
        let conn = self.conn()?;
        if let Some(p) = phrase.map(str::trim).filter(|p| valid(p)) {
            conn.execute(
                "UPDATE dictionary_entries SET phrase = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![p, id],
            )?;
        }
        if let Some(r) = replacement.map(str::trim).filter(|r| valid(r)) {
            conn.execute(
                "UPDATE dictionary_entries SET replacement = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![r, id],
            )?;
        }
        if let Some(c) = category.map(str::trim).filter(|c| !c.is_empty()) {
            conn.execute(
                "UPDATE dictionary_entries SET category = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![c, id],
            )?;
        }
        if let Some(n) = notes {
            conn.execute(
                "UPDATE dictionary_entries SET notes = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![n, id],
            )?;
        }
        Ok(Some(self.get(&conn, id)?))
    }

    pub fn delete(&self, id: i64) -> Result<bool> {
        let conn = self.conn()?;
        Ok(conn.execute("DELETE FROM dictionary_entries WHERE id = ?1", [id])? > 0)
    }

    /// Flip the favorite bit in one statement so racy toggles cannot lose an
    /// update. Returns `None` when the row is gone.
    pub fn toggle_favorite(&self, id: i64) -> Result<Option<bool>> {
        let conn = self.conn()?;
        let flipped = conn
            .query_row(
                "UPDATE dictionary_entries
                 SET is_favorite = 1 - is_favorite, updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?1 RETURNING is_favorite",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        Ok(flipped.map(|v| v != 0))
    }

    /// Import `phrase,replacement` rows. Headers are optional; malformed or
    /// over-long rows are counted as skipped instead of aborting the import.
    pub fn import_csv(&self, csv_text: &str) -> Result<(u32, u32)> {
        let conn = self.conn()?;
        let mut imported = 0u32;
        let mut skipped = 0u32;
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(csv_text.as_bytes());
        for record in reader.records().take(1000) {
            let row = match record {
                Ok(row) => row,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            let fields: Vec<&str> = row.iter().collect();
            if fields.is_empty() || fields.iter().all(|f| f.trim().is_empty()) {
                continue;
            }
            // A header row is harmless to skip; importing it as an entry is
            // not, so treat `phrase,...` as the header it is.
            if fields[0].trim().eq_ignore_ascii_case("phrase") {
                continue;
            }
            let phrase = fields[0].trim();
            let replacement = fields.get(1).map_or(phrase, |f| f.trim());
            if !valid(phrase) || !valid(replacement) {
                skipped += 1;
                continue;
            }
            match conn.execute(
                "INSERT OR IGNORE INTO dictionary_entries (phrase, replacement) VALUES (?1, ?2)",
                rusqlite::params![phrase, replacement],
            ) {
                Ok(1) => imported += 1,
                _ => skipped += 1,
            }
        }
        Ok((imported, skipped))
    }

    /// Phrase -> replacement, longest phrase first so overlapping entries do
    /// not shadow a more specific one. Used by the output pass after ASR.
    pub fn replacements(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT phrase, replacement FROM dictionary_entries
             WHERE phrase != '' AND replacement != ''
             ORDER BY LENGTH(phrase) DESC",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub fn export_csv(&self) -> Result<String> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT phrase, replacement FROM dictionary_entries
             ORDER BY is_favorite DESC, updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut lines = vec!["phrase,replacement".to_string()];
        for row in rows {
            let (phrase, replacement) = row?;
            lines.push(format!(
                "\"{}\",\"{}\"",
                phrase.replace('"', "\"\""),
                replacement.replace('"', "\"\"")
            ));
        }
        Ok(lines.join("\n"))
    }

    fn get(&self, conn: &Connection, id: i64) -> Result<DictionaryEntry> {
        Ok(conn.query_row(
            "SELECT id, phrase, replacement, category, notes, use_count, is_favorite,
                    auto_learned, created_at, updated_at
             FROM dictionary_entries WHERE id = ?1",
            [id],
            row_to_entry,
        )?)
    }
}

fn valid(text: &str) -> bool {
    !text.is_empty() && text.len() <= 60
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<DictionaryEntry> {
    Ok(DictionaryEntry {
        id: row.get(0)?,
        phrase: row.get(1)?,
        replacement: row.get(2)?,
        category: row.get(3)?,
        notes: row.get(4)?,
        use_count: row.get(5)?,
        is_favorite: row.get::<_, i64>(6)? != 0,
        auto_learned: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    struct TempDb(PathBuf);

    impl TempDb {
        fn new() -> Self {
            let n = NEXT.fetch_add(1, Ordering::SeqCst);
            Self(std::env::temp_dir().join(format!("floure-dict-{}-{n}.db", std::process::id())))
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn dict() -> (TempDb, Dictionary) {
        let temp = TempDb::new();
        let dict = Dictionary::new(&temp.0);
        (temp, dict)
    }

    #[test]
    fn add_list_and_filter_round_trip() {
        let (_temp, dict) = dict();
        let entry = dict.add("tauri", "GPUI", Some("tools"), None).unwrap();
        assert_eq!(entry.as_ref().map(|e| e.replacement.as_str()), Some("GPUI"));
        assert_eq!(dict.list(None, None, false).unwrap().len(), 1);
        assert_eq!(dict.list(Some("tau"), None, false).unwrap().len(), 1);
        assert_eq!(dict.list(Some("zzz"), None, false).unwrap().len(), 0);
        assert_eq!(dict.list(None, Some("tools"), false).unwrap().len(), 1);
    }

    #[test]
    fn duplicate_phrase_is_ignored_not_duplicated() {
        let (_temp, dict) = dict();
        assert!(dict.add("floure", "Floure", None, None).unwrap().is_some());
        assert!(dict.add("floure", "Floure", None, None).unwrap().is_none());
        assert_eq!(dict.list(None, None, false).unwrap().len(), 1);
    }

    #[test]
    fn blank_or_overlong_entries_are_rejected() {
        let (_temp, dict) = dict();
        assert!(dict.add("   ", "x", None, None).unwrap().is_none());
        assert!(
            dict.add("x", &"a".repeat(61), None, None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn favorite_toggle_survives_round_trip() {
        let (_temp, dict) = dict();
        let id = dict
            .add("ollama", "Ollama", None, None)
            .unwrap()
            .unwrap()
            .id;
        assert_eq!(dict.toggle_favorite(id).unwrap(), Some(true));
        assert!(dict.list(None, None, true).unwrap()[0].is_favorite);
        assert_eq!(dict.toggle_favorite(id).unwrap(), Some(false));
        assert_eq!(dict.toggle_favorite(999).unwrap(), None);
    }

    #[test]
    fn replacements_are_longest_first() {
        let (_temp, dict) = dict();
        dict.add("new york", "New York", None, None).unwrap();
        dict.add("york", "York", None, None).unwrap();
        let replacements = dict.replacements().unwrap();
        assert_eq!(replacements[0].0, "new york");
        assert_eq!(replacements.len(), 2);
    }

    #[test]
    fn csv_import_and_export_round_trip() {
        let (_temp, dict) = dict();
        let (imported, skipped) = dict
            .import_csv("phrase,replacement\nGPUI,gpui\n\"quoted, phrase\",value\nbad,\n")
            .unwrap();
        assert_eq!((imported, skipped), (2, 1));
        let csv = dict.export_csv().unwrap();
        assert!(csv.starts_with("phrase,replacement"));
        assert!(csv.contains("\"quoted, phrase\",\"value\""));
    }
}
