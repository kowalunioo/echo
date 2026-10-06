//! The **Inserter** seam: Insertion of a Transcript into the application that has keyboard
//! focus.
//!
//! The real Inserter (clipboard paste with restore, typing fallback —
//! `docs/specs/dictation-pipeline.md` rules 31–37) arrives in a later slice. [`FakeInserter`]
//! records what it was given.

mod fake;

use std::sync::{Arc, Mutex, PoisonError};

pub use fake::FakeInserter;

/// Why an Insertion failed. Either way the Transcript is already in History and the user is told
/// "Couldn't insert the text — it is in History" (rule 36).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InsertionError {
    /// Windows refused simulated input, e.g. into a window running as administrator.
    #[error("Windows blocked input into the focused window: {0}")]
    Blocked(String),
    /// Both clipboard paste and typing failed for another reason.
    #[error("insertion failed: {0}")]
    Failed(String),
}

/// Delivers a Transcript into whichever application has keyboard focus.
///
/// # Contract
///
/// - [`insert`](Inserter::insert) delivers `text` exactly — no added space or newline (rule 32)
///   — to the application focused at the moment of the call (rule 31).
/// - It is blocking and runs on a worker thread. It returns once the text has been delivered or
///   every method has failed; restoring the user's clipboard may finish in the background
///   afterwards (rule 34).
/// - Waiting for held modifier keys and choosing between paste and typing are the
///   implementation's business (rules 33–35); the caller only learns success or failure.
pub trait Inserter: Send {
    /// Inserts `text` into the focused application.
    fn insert(&mut self, text: &str) -> Result<(), InsertionError>;
}

/// The app-wide Inserter, kept in Tauri's managed state so every feature that inserts text
/// (a Dictation, History's Re-insert) uses the same Insertion method (`history.md` rule 13).
///
/// It starts empty: until the real Inserter is registered with [`set`](Self::set), every
/// Insertion fails with [`InsertionError::Failed`]. Clones share the same Inserter, and
/// insertions are serialised so two never interleave.
#[derive(Clone, Default)]
pub struct SharedInserter {
    inner: Arc<Mutex<Option<Box<dyn Inserter>>>>,
}

impl SharedInserter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the Inserter every later Insertion uses, replacing any earlier one.
    pub fn set(&self, inserter: impl Inserter + 'static) {
        *self.inner.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(inserter));
    }

    /// Inserts `text` with the registered Inserter. Blocking; call it from a worker thread.
    pub fn insert(&self, text: &str) -> Result<(), InsertionError> {
        match self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            Some(inserter) => inserter.insert(text),
            None => Err(InsertionError::Failed("no Inserter is available".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_inserter_without_an_inserter_fails() {
        assert!(matches!(
            SharedInserter::new().insert("x"),
            Err(InsertionError::Failed(_))
        ));
    }

    #[test]
    fn a_shared_inserter_uses_the_registered_inserter() {
        let probe = FakeInserter::new();
        let shared = SharedInserter::new();
        shared.set(probe.clone());

        shared.clone().insert("Ala ma kota").unwrap();

        assert_eq!(probe.inserted(), vec!["Ala ma kota"]);
    }
}
