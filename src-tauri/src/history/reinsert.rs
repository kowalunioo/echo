//! Re-insert (`history.md` rule 13): insert a stored Transcript again with the same Insertion
//! method as a Dictation, after Echo's window has handed focus back to the application the user
//! was in before.

use super::{History, HistoryError};
use crate::insertion::{InsertionError, SharedInserter};

/// Why a Re-insert did not happen.
#[derive(Debug, thiserror::Error)]
pub enum ReinsertError {
    /// The entry was deleted in the meantime.
    #[error("the History entry no longer exists")]
    NotFound,
    #[error(transparent)]
    History(#[from] HistoryError),
    #[error(transparent)]
    Insertion(#[from] InsertionError),
}

/// Inserts the text of entry `id`. `return_focus` runs first and must hide Echo's window and
/// wait until the previously focused application is active again. Blocking.
pub fn reinsert(
    history: &History,
    inserter: &SharedInserter,
    id: u32,
    return_focus: impl FnOnce(),
) -> Result<(), ReinsertError> {
    let entry = history.get(id)?.ok_or(ReinsertError::NotFound)?;
    return_focus();
    inserter.insert(&entry.text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::history::{EntryLanguage, HistoryStore, NewEntry};
    use crate::insertion::FakeInserter;

    fn history_with(text: &str) -> (History, u32) {
        let history = History::new(HistoryStore::open_in_memory().unwrap(), 5).unwrap();
        let entry = history
            .add(NewEntry {
                text: text.into(),
                model: "whisper-small".into(),
                language: EntryLanguage::Automatic { detected: None },
            })
            .unwrap()
            .unwrap();
        (history, entry.id)
    }

    // history.md acceptance test 10, with the fake Inserter in place of Notepad.
    #[test]
    fn reinsert_returns_focus_first_and_then_inserts_the_exact_text() {
        let (history, id) = history_with("Ala ma kota");
        let probe = FakeInserter::new();
        let inserter = SharedInserter::new();
        inserter.set(probe.clone());
        let order = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&order);
        let watcher = probe.clone();

        reinsert(&history, &inserter, id, move || {
            seen.lock().unwrap().push(watcher.inserted().len());
        })
        .unwrap();

        assert_eq!(
            *order.lock().unwrap(),
            vec![0],
            "focus returned before inserting"
        );
        assert_eq!(probe.inserted(), vec!["Ala ma kota"]);
    }

    #[test]
    fn a_missing_entry_inserts_nothing_and_keeps_focus() {
        let (history, _) = history_with("x");
        let probe = FakeInserter::new();
        let inserter = SharedInserter::new();
        inserter.set(probe.clone());
        let mut focus_returned = false;

        let result = reinsert(&history, &inserter, 999, || focus_returned = true);

        assert!(matches!(result, Err(ReinsertError::NotFound)));
        assert!(!focus_returned);
        assert!(probe.inserted().is_empty());
    }

    #[test]
    fn a_failed_insertion_is_reported_and_the_entry_stays() {
        let (history, id) = history_with("x");
        let probe = FakeInserter::new();
        probe.fail_with(Some(InsertionError::Blocked("elevated".into())));
        let inserter = SharedInserter::new();
        inserter.set(probe);

        let result = reinsert(&history, &inserter, id, || {});

        assert!(matches!(result, Err(ReinsertError::Insertion(_))));
        assert!(history.get(id).unwrap().is_some());
    }
}
