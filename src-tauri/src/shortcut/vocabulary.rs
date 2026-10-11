//! When a Vocabulary Shortcut press becomes an attempt to read the selection
//! (`vocabulary-shortcut.md` rules 3–5), as pure logic over an injected clock.
//!
//! A press starts an attempt only while the Dictation is Idle and no other attempt is in
//! progress; otherwise it does nothing (the hook still swallows it, rule 5). The copy waits for
//! the shortcut's release; an attempt whose keys are still held [`RELEASE_LIMIT`] after the press
//! is dropped silently (rule 4). Once released, the attempt is in progress until whoever reads
//! the selection reports it [`finished`](VocabularyAttempts::finished).

use std::time::{Duration, Instant};

/// How long after the press the keys may still be held before the attempt is dropped (rule 4).
pub const RELEASE_LIMIT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    /// Pressed, waiting for the release until `deadline`.
    Held {
        deadline: Instant,
    },
    /// Released; the selection is being read and added.
    InProgress,
}

/// The Vocabulary Shortcut's attempts.
#[derive(Debug, Clone)]
pub struct VocabularyAttempts {
    phase: Phase,
}

impl Default for VocabularyAttempts {
    fn default() -> Self {
        Self::new()
    }
}

impl VocabularyAttempts {
    pub const fn new() -> Self {
        Self {
            phase: Phase::Ready,
        }
    }

    /// The shortcut was pressed at `now`. Starts an attempt unless a Dictation is under way or
    /// another attempt is (rule 5); returns whether it did.
    pub fn press(&mut self, now: Instant, dictation_idle: bool) -> bool {
        if !dictation_idle || self.phase != Phase::Ready {
            return false;
        }
        self.phase = Phase::Held {
            deadline: now + RELEASE_LIMIT,
        };
        true
    }

    /// The shortcut was released at `now`. Returns the instant by which every key must be up for
    /// the copy to start, when the attempt goes on (rule 4).
    pub fn release(&mut self, now: Instant) -> Option<Instant> {
        let Phase::Held { deadline } = self.phase else {
            return None;
        };
        if now >= deadline {
            self.phase = Phase::Ready;
            return None;
        }
        self.phase = Phase::InProgress;
        Some(deadline)
    }

    /// Time passed: an attempt still held at its deadline is dropped silently.
    pub fn tick(&mut self, now: Instant) {
        if let Phase::Held { deadline } = self.phase
            && now >= deadline
        {
            self.phase = Phase::Ready;
        }
    }

    /// When [`tick`](Self::tick) next has something to do.
    pub fn next_deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Held { deadline } => Some(deadline),
            _ => None,
        }
    }

    /// The attempt in progress ended (whatever its outcome).
    pub fn finished(&mut self) {
        if self.phase == Phase::InProgress {
            self.phase = Phase::Ready;
        }
    }

    /// Forgets an attempt that is still held (e.g. the shortcut was turned off meanwhile).
    pub fn cancel_held(&mut self) {
        if matches!(self.phase, Phase::Held { .. }) {
            self.phase = Phase::Ready;
        }
    }

    /// Whether an attempt has started and not yet finished.
    pub fn busy(&self) -> bool {
        self.phase != Phase::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    /// Acceptance test 2: nothing happens until the release.
    #[test]
    fn the_attempt_goes_on_at_the_release_with_the_press_deadline() {
        let t0 = Instant::now();
        let mut attempts = VocabularyAttempts::new();

        assert!(attempts.press(t0, true));
        assert_eq!(attempts.next_deadline(), Some(at(t0, 2000)));
        attempts.tick(at(t0, 1500));
        assert_eq!(attempts.release(at(t0, 1500)), Some(at(t0, 2000)));
        assert_eq!(attempts.next_deadline(), None);
        assert!(attempts.busy());
    }

    /// Acceptance test 3.
    #[test]
    fn keys_held_for_two_seconds_drop_the_attempt() {
        let t0 = Instant::now();
        let mut attempts = VocabularyAttempts::new();

        attempts.press(t0, true);
        attempts.tick(at(t0, 2000));
        assert!(!attempts.busy());
        assert_eq!(attempts.release(at(t0, 2100)), None);

        // Without a tick in between, a late release drops it too.
        attempts.press(at(t0, 3000), true);
        assert_eq!(attempts.release(at(t0, 5100)), None);
        assert!(!attempts.busy());
    }

    /// Rule 5: nothing during a Dictation or while another attempt is in progress, and nothing
    /// is remembered for later.
    #[test]
    fn presses_while_busy_do_nothing() {
        let t0 = Instant::now();
        let mut attempts = VocabularyAttempts::new();

        assert!(!attempts.press(t0, false));
        assert_eq!(attempts.release(at(t0, 10)), None);
        assert!(!attempts.busy());

        assert!(attempts.press(at(t0, 100), true));
        assert!(!attempts.press(at(t0, 150), true));
        attempts.release(at(t0, 200));
        assert!(!attempts.press(at(t0, 300), true));
        attempts.finished();
        assert!(!attempts.busy());
        assert!(attempts.press(at(t0, 400), true));
    }

    #[test]
    fn a_finish_report_only_ends_an_attempt_in_progress() {
        let t0 = Instant::now();
        let mut attempts = VocabularyAttempts::new();
        attempts.press(t0, true);
        attempts.finished();
        assert!(attempts.busy(), "still held, not in progress");
        attempts.cancel_held();
        assert!(!attempts.busy());
    }
}
