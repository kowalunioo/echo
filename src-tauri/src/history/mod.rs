//! **History**: the stored list of recent Transcripts (`docs/specs/history.md`).
//!
//! [`History`] is the service the rest of Echo uses. The Dictation pipeline calls
//! [`History::add`] once per Dictation that produced a non-empty Transcript and was not cancelled
//! (`dictation-pipeline.md` rule 38 — deciding that is the pipeline's job), before Insertion
//! starts. The tray reads [`History::latest`] and follows [`History::subscribe`]. The History
//! limit comes from the settings: `lib.rs` passes every change to [`History::set_limit`].
//!
//! Storage is SQLite ([`HistoryStore`]); text only, never audio.

pub mod commands;
mod reinsert;
mod store;

pub use commands::HistoryChanged;
pub use reinsert::{ReinsertError, reinsert};
pub use store::{HistoryError, HistoryStore, SCHEMA_VERSION};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;

/// One stored Transcript (rule 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// Unique and never reused, even after the entry is deleted.
    pub id: u32,
    /// When the entry was created, in milliseconds since the Unix epoch (UTC). Shown in local
    /// time.
    // Exported to TypeScript as a plain `number` (millisecond timestamps fit exactly): specta
    // refuses i64 as a BigInt and exports f64 as `number | null`.
    #[specta(type = u32)]
    pub created_at: i64,
    /// The Transcript exactly as inserted.
    pub text: String,
    /// The id of the Model that produced it (`models.md`).
    pub model: String,
    /// The effective Dictation Language.
    pub language: EntryLanguage,
}

/// The effective Dictation Language of a stored Transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EntryLanguage {
    /// Automatic detection, with the language the Engine detected if it reported one.
    Automatic { detected: Option<String> },
    /// A chosen language: an ISO 639-1 code such as `"pl"`.
    Specific { code: String },
}

/// What the pipeline hands to [`History::add`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntry {
    pub text: String,
    pub model: String,
    pub language: EntryLanguage,
}

/// Where History ended up when Echo started.
#[derive(Debug)]
pub enum OpenOutcome {
    /// The database file is in use.
    Opened,
    /// The file was not a usable database; it was renamed and a fresh one created.
    Replaced {
        renamed_to: PathBuf,
        error: HistoryError,
    },
    /// The file could not be used (e.g. written by a newer Echo) and was left untouched;
    /// History lives in memory until Echo exits.
    InMemory { error: HistoryError },
}

type Listener = Box<dyn Fn(&[HistoryEntry]) + Send + Sync>;
type Clock = Box<dyn Fn() -> i64 + Send + Sync>;

/// The History service: storage plus the limit and change notifications.
pub struct History {
    store: Mutex<HistoryStore>,
    limit: AtomicU32,
    listeners: RwLock<Vec<Listener>>,
    clock: Clock,
}

impl History {
    /// History over `store`, keeping at most `limit` entries (applied at once).
    pub fn new(store: HistoryStore, limit: u32) -> Result<Self, HistoryError> {
        let history = Self {
            store: Mutex::new(store),
            limit: AtomicU32::new(limit),
            listeners: RwLock::new(Vec::new()),
            clock: Box::new(now_millis),
        };
        history.lock().trim(limit)?;
        Ok(history)
    }

    /// Opens the database at `path`. A file that is not a usable database is renamed with a
    /// `.broken` suffix and replaced by a fresh one; a file from a newer Echo is left alone and
    /// History is kept in memory instead, so Echo always starts.
    pub fn open(path: &Path, limit: u32) -> (Self, OpenOutcome) {
        let (store, outcome) = match HistoryStore::open(path) {
            Ok(store) => (store, OpenOutcome::Opened),
            Err(error @ HistoryError::NewerSchema { .. }) => {
                (in_memory(), OpenOutcome::InMemory { error })
            }
            Err(error) => {
                let mut name = path.file_name().unwrap_or_default().to_owned();
                name.push(".broken");
                let renamed_to = path.with_file_name(name);
                match std::fs::rename(path, &renamed_to)
                    .map_err(HistoryError::from)
                    .and_then(|()| HistoryStore::open(path))
                {
                    Ok(store) => (store, OpenOutcome::Replaced { renamed_to, error }),
                    Err(second) => {
                        log::error!("could not replace the History database: {second}");
                        (in_memory(), OpenOutcome::InMemory { error })
                    }
                }
            }
        };
        let history = Self::new(store, limit).unwrap_or_else(|error| {
            log::error!("could not apply the History limit: {error}");
            Self::new(in_memory(), limit).expect("an in-memory database always works")
        });
        (history, outcome)
    }

    /// Replaces the clock (milliseconds since the Unix epoch) for tests.
    pub fn with_clock(mut self, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    /// Stores a Transcript and applies the limit. Returns the new entry, or `None` when nothing
    /// was kept: a limit of 0 (rule 9) or a text that is empty or whitespace only (rule 2).
    pub fn add(&self, entry: NewEntry) -> Result<Option<HistoryEntry>, HistoryError> {
        if entry.text.trim().is_empty() {
            return Ok(None);
        }
        let limit = self.limit();
        let created_at = (self.clock)();
        self.change(|store| store.add(&entry, created_at, limit))
    }

    /// Every entry, newest first.
    pub fn entries(&self) -> Result<Vec<HistoryEntry>, HistoryError> {
        self.lock().list()
    }

    /// The newest entry, if any (the tray's "Copy last Transcript", rule 15).
    pub fn latest(&self) -> Result<Option<HistoryEntry>, HistoryError> {
        Ok(self.entries()?.into_iter().next())
    }

    /// The entry with `id`, if it exists.
    pub fn get(&self, id: u32) -> Result<Option<HistoryEntry>, HistoryError> {
        self.lock().get(id)
    }

    /// Permanently deletes one entry and returns it so the UI can offer Undo (rule 12).
    pub fn delete(&self, id: u32) -> Result<Option<HistoryEntry>, HistoryError> {
        self.change(|store| store.delete(id))
    }

    /// Puts a deleted entry back with its original id and time (Undo), within the limit.
    pub fn restore(&self, entry: &HistoryEntry) -> Result<(), HistoryError> {
        let limit = self.limit();
        self.change(|store| store.restore(entry, limit))
    }

    /// Deletes every entry (rule 14).
    pub fn clear(&self) -> Result<(), HistoryError> {
        self.change(|store| store.clear().map(drop))
    }

    /// The current History limit.
    pub fn limit(&self) -> u32 {
        self.limit.load(Ordering::SeqCst)
    }

    /// Changes the limit; lowering it deletes the oldest surplus entries at once (rule 8).
    pub fn set_limit(&self, limit: u32) -> Result<(), HistoryError> {
        self.limit.store(limit, Ordering::SeqCst);
        self.change(|store| store.trim(limit).map(drop))
    }

    /// Registers `listener` to run with the complete list (newest first) after every change.
    pub fn subscribe(&self, listener: impl Fn(&[HistoryEntry]) + Send + Sync + 'static) {
        self.listeners
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Box::new(listener));
    }

    /// Runs `operation` on the store and then tells the listeners, outside the store's lock.
    fn change<T>(
        &self,
        operation: impl FnOnce(&mut HistoryStore) -> Result<T, HistoryError>,
    ) -> Result<T, HistoryError> {
        let (result, entries) = {
            let mut store = self.lock();
            let result = operation(&mut store)?;
            (result, store.list()?)
        };
        for listener in self
            .listeners
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            listener(&entries);
        }
        Ok(result)
    }

    fn lock(&self) -> MutexGuard<'_, HistoryStore> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn in_memory() -> HistoryStore {
    HistoryStore::open_in_memory().expect("an in-memory database always works")
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn new_entry(text: &str) -> NewEntry {
        NewEntry {
            text: text.to_owned(),
            model: "whisper-large-v3-turbo".to_owned(),
            language: EntryLanguage::Specific { code: "en".into() },
        }
    }

    fn history(limit: u32) -> History {
        History::new(HistoryStore::open_in_memory().unwrap(), limit).unwrap()
    }

    fn texts(history: &History) -> Vec<String> {
        history
            .entries()
            .unwrap()
            .into_iter()
            .map(|e| e.text)
            .collect()
    }

    /// Records every list the listeners received.
    fn record(history: &History) -> Arc<Mutex<Vec<Vec<String>>>> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        history.subscribe(move |entries| {
            sink.lock()
                .unwrap()
                .push(entries.iter().map(|e| e.text.clone()).collect());
        });
        seen
    }

    // history.md acceptance test 1 (History part).
    #[test]
    fn an_added_transcript_is_stored_with_the_model_and_the_current_time() {
        let history = history(5);

        let stored = history.add(new_entry("Hello")).unwrap().unwrap();

        assert_eq!(stored.text, "Hello");
        assert_eq!(stored.model, "whisper-large-v3-turbo");
        assert!((now_millis() - stored.created_at).abs() < 2_000);
        assert_eq!(history.entries().unwrap(), vec![stored]);
    }

    // Rule 2: an empty Transcript never becomes an entry, even if a caller passes one.
    #[test]
    fn empty_or_blank_text_is_not_stored() {
        let history = history(5);
        assert_eq!(history.add(new_entry("")).unwrap(), None);
        assert_eq!(history.add(new_entry("  \n\t")).unwrap(), None);
        assert!(history.entries().unwrap().is_empty());
    }

    // history.md acceptance test 4.
    #[test]
    fn with_limit_5_seven_transcripts_leave_the_five_newest_in_order() {
        let tick = Arc::new(AtomicU32::new(0));
        let clock = Arc::clone(&tick);
        let history =
            history(5).with_clock(move || i64::from(clock.fetch_add(1, Ordering::SeqCst)));

        for n in 1..=7 {
            history.add(new_entry(&format!("t{n}"))).unwrap();
        }

        assert_eq!(texts(&history), ["t7", "t6", "t5", "t4", "t3"]);
    }

    // history.md acceptance test 5.
    #[test]
    fn lowering_the_limit_keeps_the_newest_at_once() {
        let history = history(5);
        for n in 1..=5 {
            history.add(new_entry(&format!("t{n}"))).unwrap();
        }

        history.set_limit(2).unwrap();

        assert_eq!(texts(&history), ["t5", "t4"]);
        assert_eq!(history.limit(), 2);
    }

    // history.md acceptance test 6 (History part).
    #[test]
    fn with_limit_0_history_stays_empty() {
        let history = history(0);

        assert_eq!(history.add(new_entry("Hello")).unwrap(), None);

        assert!(history.entries().unwrap().is_empty());
        assert_eq!(history.latest().unwrap(), None);
    }

    #[test]
    fn opening_applies_the_current_limit() {
        let mut store = HistoryStore::open_in_memory().unwrap();
        for n in 1..=4 {
            store.add(&new_entry(&n.to_string()), n, 100).unwrap();
        }

        let history = History::new(store, 1).unwrap();

        assert_eq!(texts(&history), ["4"]);
    }

    // tray.md acceptance test 5 (History part).
    #[test]
    fn latest_is_the_newest_entry() {
        let history = history(5);
        history.add(new_entry("older")).unwrap();
        history.add(new_entry("newest")).unwrap();
        assert_eq!(history.latest().unwrap().unwrap().text, "newest");
    }

    // history.md acceptance test 9 (service part).
    #[test]
    fn delete_and_undo() {
        let history = history(5);
        let entry = history.add(new_entry("Ala ma kota")).unwrap().unwrap();

        assert_eq!(history.delete(entry.id).unwrap(), Some(entry.clone()));
        assert!(history.entries().unwrap().is_empty());

        history.restore(&entry).unwrap();
        assert_eq!(history.entries().unwrap(), vec![entry]);
    }

    // Rule 16: every change reaches the listeners with the full list.
    #[test]
    fn listeners_hear_about_every_change() {
        let history = history(5);
        let seen = record(&history);

        let first = history.add(new_entry("a")).unwrap().unwrap();
        history.add(new_entry("b")).unwrap();
        history.delete(first.id).unwrap();
        history.restore(&first).unwrap();
        history.set_limit(1).unwrap();
        history.clear().unwrap();

        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                vec!["a"],
                vec!["b", "a"],
                vec!["b"],
                vec!["b", "a"],
                vec!["b"],
                Vec::<&str>::new(),
            ]
        );
    }

    #[test]
    fn listeners_may_read_history() {
        let history = Arc::new(history(5));
        let reader = Arc::clone(&history);
        let latest = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&latest);
        history.subscribe(move |_| {
            *sink.lock().unwrap() = reader.latest().unwrap().map(|e| e.text);
        });

        history.add(new_entry("x")).unwrap();

        assert_eq!(latest.lock().unwrap().as_deref(), Some("x"));
    }

    #[test]
    fn a_missing_file_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");

        let (history, outcome) = History::open(&path, 5);

        assert!(matches!(outcome, OpenOutcome::Opened));
        history.add(new_entry("kept")).unwrap();
        drop(history);
        let (reopened, _) = History::open(&path, 5);
        assert_eq!(texts(&reopened), ["kept"]);
    }

    #[test]
    fn an_unreadable_file_is_renamed_broken_and_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        std::fs::write(&path, vec![b'x'; 4096]).unwrap();

        let (history, outcome) = History::open(&path, 5);

        let broken = dir.path().join("history.db.broken");
        assert!(
            matches!(&outcome, OpenOutcome::Replaced { renamed_to, .. } if *renamed_to == broken)
        );
        assert_eq!(std::fs::read(&broken).unwrap(), vec![b'x'; 4096]);
        history.add(new_entry("works")).unwrap();
        assert_eq!(HistoryStore::open(&path).unwrap().list().unwrap().len(), 1);
    }

    #[test]
    fn a_file_from_a_newer_echo_is_left_alone_and_history_runs_in_memory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .unwrap();
        }
        let before = std::fs::read(&path).unwrap();

        let (history, outcome) = History::open(&path, 5);

        assert!(matches!(outcome, OpenOutcome::InMemory { .. }));
        history.add(new_entry("in memory")).unwrap();
        assert_eq!(texts(&history), ["in memory"]);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
