//! Error indication state (`dictation-pipeline.md` rules 39a–39d): the error the tray shows and
//! the notices the main window lists. Pure logic; the runtime publishes it.

use super::{DictationProblem, ProblemKind};

/// The main window keeps at most this many notices; older ones are dropped.
const MAX_NOTICES: usize = 20;

#[derive(Debug, Default)]
pub struct Indicator {
    last_id: u32,
    /// The red tray icon and its tooltip: the most recent error, until the main window is seen or
    /// a Dictation succeeds (rule 39c).
    error: Option<DictationProblem>,
    /// Errors for the main window, oldest first, until the user dismisses them (rule 39d).
    notices: Vec<DictationProblem>,
}

impl Indicator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an error; it becomes the tray error and joins the notices.
    pub fn report(&mut self, kind: ProblemKind, detail: impl Into<String>) -> DictationProblem {
        self.last_id += 1;
        let problem = DictationProblem {
            id: self.last_id,
            kind,
            detail: detail.into(),
        };
        self.error = Some(problem.clone());
        self.notices.push(problem.clone());
        let excess = self.notices.len().saturating_sub(MAX_NOTICES);
        self.notices.drain(..excess);
        problem
    }

    /// A Dictation was inserted without error: the tray returns to idle (rule 39c).
    pub fn dictation_succeeded(&mut self) {
        self.error = None;
    }

    /// The user has the main window in front of them: the tray returns to idle (rule 39c). The
    /// notices stay listed until dismissed.
    pub fn window_seen(&mut self) {
        self.error = None;
    }

    /// The user dismissed the notices in the main window.
    pub fn dismiss_notices(&mut self) {
        self.notices.clear();
    }

    pub fn error(&self) -> Option<&DictationProblem> {
        self.error.as_ref()
    }

    pub fn notices(&self) -> &[DictationProblem] {
        &self.notices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_recent_error_is_shown_and_all_are_listed() {
        let mut i = Indicator::new();
        i.report(ProblemKind::NoModel, "");
        let second = i.report(ProblemKind::TranscriptionFailed, "boom");
        assert_eq!(i.error(), Some(&second));
        assert_eq!(i.notices().len(), 2);
        assert!(i.notices()[0].id < i.notices()[1].id);
    }

    #[test]
    fn seeing_the_window_or_succeeding_clears_the_tray_error_but_not_the_notices() {
        let mut i = Indicator::new();
        i.report(ProblemKind::InsertionFailed, "x");
        i.window_seen();
        assert_eq!(i.error(), None);
        assert_eq!(i.notices().len(), 1);
        i.report(ProblemKind::InsertionFailed, "y");
        i.dictation_succeeded();
        assert_eq!(i.error(), None);
        i.dismiss_notices();
        assert!(i.notices().is_empty());
    }

    #[test]
    fn notices_are_capped() {
        let mut i = Indicator::new();
        for n in 0..30 {
            i.report(ProblemKind::MicrophoneFailed, n.to_string());
        }
        assert_eq!(i.notices().len(), MAX_NOTICES);
        assert_eq!(i.notices()[0].detail, "10");
    }
}
