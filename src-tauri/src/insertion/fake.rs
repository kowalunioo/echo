use std::sync::{Arc, Mutex, MutexGuard};

use super::{Inserter, InsertionError};

/// An [`Inserter`] that records every text it receives and optionally fails.
///
/// Clones share state, so a test keeps one clone to inspect the fake while the code under test
/// owns another.
#[derive(Debug, Clone, Default)]
pub struct FakeInserter {
    state: Arc<Mutex<State>>,
}

#[derive(Debug, Default)]
struct State {
    inserted: Vec<String>,
    failure: Option<InsertionError>,
}

impl FakeInserter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes later insertions fail with `error` (or succeed again with `None`). Failed
    /// insertions are not recorded.
    pub fn fail_with(&self, error: Option<InsertionError>) {
        self.state().failure = error;
    }

    /// Every text inserted so far, oldest first.
    pub fn inserted(&self) -> Vec<String> {
        self.state().inserted.clone()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Inserter for FakeInserter {
    fn insert(&mut self, text: &str) -> Result<(), InsertionError> {
        let mut state = self.state();
        match &state.failure {
            Some(error) => Err(error.clone()),
            None => {
                state.inserted.push(text.to_owned());
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_inserted_text_exactly() {
        let probe = FakeInserter::new();
        let mut inserter: Box<dyn Inserter> = Box::new(probe.clone());

        inserter.insert("so we go").unwrap();
        inserter.insert(" zażółć ").unwrap();

        assert_eq!(probe.inserted(), vec!["so we go", " zażółć "]);
    }

    #[test]
    fn a_failing_inserter_reports_the_error_and_records_nothing() {
        let probe = FakeInserter::new();
        let mut inserter = probe.clone();
        let error = InsertionError::Blocked("elevated window".into());
        probe.fail_with(Some(error.clone()));

        assert_eq!(inserter.insert("hello"), Err(error));
        assert!(probe.inserted().is_empty());
    }
}
