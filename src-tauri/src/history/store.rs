//! The History database: one SQLite file in Echo's data directory (`history.md` rule 5).
//!
//! The schema is versioned with SQLite's `user_version`: each entry of [`MIGRATIONS`] upgrades
//! the database by one version, and opening applies the ones it has not seen yet. The database
//! starts from scratch under `com.enloque.echo`; nothing from any earlier app is read (rule 13).

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{EntryLanguage, HistoryEntry, NewEntry};

/// Schema upgrades, oldest first. Entry `n` takes the database from version `n` to `n + 1`.
/// Never edit a released entry; append a new one.
const MIGRATIONS: &[&str] = &[
    // Version 1. `language` is an ISO 639-1 code, or `auto` for automatic detection, in which
    // case `detected_language` holds what the Engine reported (if anything).
    "CREATE TABLE entries (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        created_at INTEGER NOT NULL,
        text TEXT NOT NULL,
        model TEXT NOT NULL,
        language TEXT NOT NULL,
        detected_language TEXT
    );
    CREATE INDEX entries_newest ON entries (created_at DESC, id DESC);",
];

/// The schema version this build writes.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

const AUTOMATIC: &str = "auto";

/// Why the History database could not be used.
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("History database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("History folder could not be created: {0}")]
    Io(#[from] std::io::Error),
    /// The file was written by a newer Echo; it is left untouched.
    #[error("History database has schema version {found}, newer than the supported {supported}")]
    NewerSchema { found: u32, supported: u32 },
}

/// Storage for History entries. Every method is one statement or one transaction, so a crash
/// never leaves a half-applied change.
pub struct HistoryStore {
    conn: Connection,
}

impl HistoryStore {
    /// Opens (or creates) the database file at `path` and brings its schema up to date.
    pub fn open(path: &Path) -> Result<Self, HistoryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::prepare(Connection::open(path)?)
    }

    /// A database that lives only in memory (tests, and the fallback when the file is unusable).
    pub fn open_in_memory() -> Result<Self, HistoryError> {
        Self::prepare(Connection::open_in_memory()?)
    }

    fn prepare(mut conn: Connection) -> Result<Self, HistoryError> {
        // Deleted entries are removed permanently (rule 10): overwrite their text on disk too.
        conn.pragma_update(None, "secure_delete", true)?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// The schema version of the open database.
    pub fn schema_version(&self) -> Result<u32, HistoryError> {
        Ok(user_version(&self.conn)?)
    }

    /// Stores `entry` as created at `created_at` (ms since the Unix epoch, UTC), then deletes the
    /// oldest entries so at most `limit` remain (rule 7). Returns the stored entry, or `None`
    /// when `limit` is 0 and nothing is kept (rule 9).
    pub fn add(
        &mut self,
        entry: &NewEntry,
        created_at: i64,
        limit: u32,
    ) -> Result<Option<HistoryEntry>, HistoryError> {
        if limit == 0 {
            return Ok(None);
        }
        let (language, detected) = language_columns(&entry.language);
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO entries (created_at, text, model, language, detected_language)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![created_at, entry.text, entry.model, language, detected],
        )?;
        let id = tx.last_insert_rowid();
        trim(&tx, limit)?;
        tx.commit()?;
        self.get_row(id)
    }

    /// Puts a deleted entry back with its original id and creation time (Undo, rule 12), then
    /// applies `limit`. Restoring an entry that is still present changes nothing.
    pub fn restore(&mut self, entry: &HistoryEntry, limit: u32) -> Result<(), HistoryError> {
        let (language, detected) = language_columns(&entry.language);
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO entries
                 (id, created_at, text, model, language, detected_language)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entry.id,
                entry.created_at,
                entry.text,
                entry.model,
                language,
                detected
            ],
        )?;
        trim(&tx, limit)?;
        tx.commit()?;
        Ok(())
    }

    /// Every entry, newest first.
    pub fn list(&self) -> Result<Vec<HistoryEntry>, HistoryError> {
        let mut statement = self.conn.prepare_cached(
            "SELECT id, created_at, text, model, language, detected_language
             FROM entries ORDER BY created_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], entry_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The entry with `id`, if it exists.
    pub fn get(&self, id: u32) -> Result<Option<HistoryEntry>, HistoryError> {
        self.get_row(i64::from(id))
    }

    fn get_row(&self, id: i64) -> Result<Option<HistoryEntry>, HistoryError> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, created_at, text, model, language, detected_language
                 FROM entries WHERE id = ?1",
                [id],
                entry_from_row,
            )
            .optional()?)
    }

    /// Permanently deletes the entry with `id` and returns it, so the caller can offer Undo.
    pub fn delete(&mut self, id: u32) -> Result<Option<HistoryEntry>, HistoryError> {
        let tx = self.conn.transaction()?;
        let entry = tx
            .query_row(
                "SELECT id, created_at, text, model, language, detected_language
                 FROM entries WHERE id = ?1",
                [id],
                entry_from_row,
            )
            .optional()?;
        tx.execute("DELETE FROM entries WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(entry)
    }

    /// Deletes the oldest entries so at most `limit` remain (rule 8). Returns how many went.
    pub fn trim(&mut self, limit: u32) -> Result<usize, HistoryError> {
        Ok(trim(&self.conn, limit)?)
    }

    /// Deletes every entry (Clear all, rule 14). Returns how many went.
    pub fn clear(&mut self) -> Result<usize, HistoryError> {
        Ok(self.conn.execute("DELETE FROM entries", [])?)
    }
}

fn trim(conn: &Connection, limit: u32) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM entries WHERE id NOT IN
             (SELECT id FROM entries ORDER BY created_at DESC, id DESC LIMIT ?1)",
        [limit],
    )
}

fn user_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

/// Applies every migration the database has not seen, all in one transaction.
fn migrate(conn: &mut Connection) -> Result<(), HistoryError> {
    let found = user_version(conn)?;
    if found > SCHEMA_VERSION {
        return Err(HistoryError::NewerSchema {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    if found == SCHEMA_VERSION {
        return Ok(());
    }
    let tx = conn.transaction()?;
    for sql in &MIGRATIONS[found as usize..] {
        tx.execute_batch(sql)?;
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

fn language_columns(language: &EntryLanguage) -> (&str, Option<&str>) {
    match language {
        EntryLanguage::Automatic { detected } => (AUTOMATIC, detected.as_deref()),
        EntryLanguage::Specific { code } => (code.as_str(), None),
    }
}

fn entry_from_row(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let language: String = row.get(4)?;
    let language = if language == AUTOMATIC {
        EntryLanguage::Automatic {
            detected: row.get(5)?,
        }
    } else {
        EntryLanguage::Specific { code: language }
    };
    Ok(HistoryEntry {
        id: row.get(0)?,
        created_at: row.get(1)?,
        text: row.get(2)?,
        model: row.get(3)?,
        language,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str) -> NewEntry {
        NewEntry {
            text: text.to_owned(),
            model: "whisper-large-v3-turbo".to_owned(),
            language: EntryLanguage::Specific { code: "pl".into() },
        }
    }

    fn texts(store: &HistoryStore) -> Vec<String> {
        store.list().unwrap().into_iter().map(|e| e.text).collect()
    }

    fn store_with(count: u32) -> HistoryStore {
        let mut store = HistoryStore::open_in_memory().unwrap();
        for n in 1..=count {
            store
                .add(&entry(&n.to_string()), 1_000 * i64::from(n), 100)
                .unwrap();
        }
        store
    }

    #[test]
    fn an_added_entry_keeps_every_field() {
        let mut store = HistoryStore::open_in_memory().unwrap();
        let new = NewEntry {
            text: "Ala ma kota.".into(),
            model: "parakeet-tdt-0.6b-v3".into(),
            language: EntryLanguage::Automatic {
                detected: Some("pl".into()),
            },
        };

        let stored = store.add(&new, 1_759_000_000_000, 5).unwrap().unwrap();

        assert_eq!(stored.text, "Ala ma kota.");
        assert_eq!(stored.model, "parakeet-tdt-0.6b-v3");
        assert_eq!(stored.created_at, 1_759_000_000_000);
        assert_eq!(stored.language, new.language);
        assert_eq!(store.list().unwrap(), vec![stored]);
    }

    #[test]
    fn automatic_language_without_a_detected_one_round_trips() {
        let mut store = HistoryStore::open_in_memory().unwrap();
        let mut new = entry("x");
        new.language = EntryLanguage::Automatic { detected: None };

        let stored = store.add(&new, 1, 5).unwrap().unwrap();

        assert_eq!(stored.language, EntryLanguage::Automatic { detected: None });
    }

    #[test]
    fn entries_are_listed_newest_first() {
        let store = store_with(3);
        assert_eq!(texts(&store), ["3", "2", "1"]);
    }

    #[test]
    fn entries_created_in_the_same_millisecond_keep_their_order() {
        let mut store = HistoryStore::open_in_memory().unwrap();
        store.add(&entry("first"), 7, 5).unwrap();
        store.add(&entry("second"), 7, 5).unwrap();
        assert_eq!(texts(&store), ["second", "first"]);
    }

    // history.md acceptance test 4 (storage part) and rule 7.
    #[test]
    fn adding_beyond_the_limit_deletes_the_oldest() {
        let mut store = HistoryStore::open_in_memory().unwrap();
        for n in 1..=7 {
            store.add(&entry(&n.to_string()), n, 5).unwrap();
        }
        assert_eq!(texts(&store), ["7", "6", "5", "4", "3"]);
    }

    // history.md acceptance test 5 and rule 8.
    #[test]
    fn lowering_the_limit_keeps_the_newest() {
        let mut store = store_with(5);

        assert_eq!(store.trim(2).unwrap(), 3);

        assert_eq!(texts(&store), ["5", "4"]);
    }

    // Rule 9.
    #[test]
    fn with_limit_zero_nothing_is_kept() {
        let mut store = store_with(2);

        assert_eq!(store.add(&entry("new"), 9_999, 0).unwrap(), None);
        store.trim(0).unwrap();

        assert!(store.list().unwrap().is_empty());
    }

    // history.md acceptance test 9 (storage part).
    #[test]
    fn a_deleted_entry_is_gone_and_undo_restores_it_with_its_original_time() {
        let mut store = store_with(3);
        let middle = store.list().unwrap()[1].clone();

        let deleted = store.delete(middle.id).unwrap();

        assert_eq!(deleted.as_ref(), Some(&middle));
        assert_eq!(texts(&store), ["3", "1"]);
        assert_eq!(store.get(middle.id).unwrap(), None);

        store.restore(&middle, 5).unwrap();

        assert_eq!(texts(&store), ["3", "2", "1"]);
        assert_eq!(store.get(middle.id).unwrap(), Some(middle));
    }

    #[test]
    fn deleting_a_missing_entry_reports_nothing() {
        let mut store = store_with(1);
        assert_eq!(store.delete(999).unwrap(), None);
        assert_eq!(texts(&store), ["1"]);
    }

    #[test]
    fn restoring_respects_the_limit_and_ignores_duplicates() {
        let mut store = store_with(3);
        let oldest = store.list().unwrap()[2].clone();
        store.delete(oldest.id).unwrap();

        store.restore(&oldest, 2).unwrap();
        assert_eq!(
            texts(&store),
            ["3", "2"],
            "the restored entry is the oldest"
        );

        let newest = store.list().unwrap()[0].clone();
        store.restore(&newest, 5).unwrap();
        assert_eq!(texts(&store), ["3", "2"]);
    }

    #[test]
    fn new_entries_never_reuse_a_deleted_id() {
        let mut store = store_with(2);
        let newest = store.list().unwrap()[0].clone();
        store.delete(newest.id).unwrap();

        let added = store.add(&entry("new"), 5_000, 5).unwrap().unwrap();

        assert_ne!(added.id, newest.id);
        store.restore(&newest, 5).unwrap();
        assert_eq!(store.list().unwrap().len(), 3);
    }

    // Rule 14.
    #[test]
    fn clear_deletes_everything() {
        let mut store = store_with(4);
        assert_eq!(store.clear().unwrap(), 4);
        assert!(store.list().unwrap().is_empty());
    }

    // history.md acceptance test 13 and rule 5.
    #[test]
    fn a_fresh_database_file_is_created_with_the_current_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("history.db");

        let store = HistoryStore::open(&path).unwrap();

        assert!(path.exists());
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn entries_survive_reopening_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let mut store = HistoryStore::open(&path).unwrap();
        store.add(&entry("kept"), 1, 5).unwrap();
        drop(store);

        let reopened = HistoryStore::open(&path).unwrap();

        assert_eq!(texts(&reopened), ["kept"]);
    }

    #[test]
    fn a_database_from_a_newer_echo_is_refused_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .unwrap();
        }

        let result = HistoryStore::open(&path);

        assert!(matches!(
            result,
            Err(HistoryError::NewerSchema { found, .. }) if found == SCHEMA_VERSION + 1
        ));
        let conn = Connection::open(&path).unwrap();
        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION + 1);
    }

    #[test]
    fn a_file_that_is_not_a_database_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        std::fs::write(
            &path,
            b"this is not sqlite, just some text that is long enough",
        )
        .unwrap();

        assert!(matches!(
            HistoryStore::open(&path),
            Err(HistoryError::Database(_))
        ));
    }

    #[test]
    fn migrating_an_up_to_date_database_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let mut store = HistoryStore::open(&path).unwrap();
        store.add(&entry("kept"), 1, 5).unwrap();

        migrate(&mut store.conn).unwrap();

        assert_eq!(texts(&store), ["kept"]);
    }
}
