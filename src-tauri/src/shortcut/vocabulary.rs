//! When a Vocabulary Shortcut press becomes an attempt to read the selection
//! (`vocabulary-shortcut.md` rules 3–5), as pure logic over an injected clock.
//!
//! A press starts an attempt only while the Dictation is Idle and no other attempt is in
//! progress; otherwise it does nothing (the hook still swallows it, rule 5). The attempt goes on
//! at the shortcut's release, however long the keys are held — or, if they are still held
//! [`RELEASE_WAIT`] after the press, at that moment, so that whoever reads the selection shows
//! the release-the-keys message instead of copying (rule 4). From then on the attempt is in
//! progress until whoever reads the selection reports it [`finished`](VocabularyAttempts::finished).

use std::time::{Duration, Instant};

/// How long after the press the attempt waits for every key to be released (rule 4).
pub const RELEASE_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    /// Pressed, waiting for the release until `deadline`.
    Held {
        deadline: Instant,
    },
    /// Released or out of time; the selection is being read and added.
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
            deadline: now + RELEASE_WAIT,
        };
        true
    }

    /// The shortcut was released. Returns the instant by which every key must be up for the
    /// copy to start, when this release lets an attempt go on (rule 4).
    pub fn release(&mut self) -> Option<Instant> {
        let Phase::Held { deadline } = self.phase else {
            return None;
        };
        self.phase = Phase::InProgress;
        Some(deadline)
    }

    /// Time passed: an attempt still held at its deadline goes on now, and returns that
    /// deadline; whoever reads the selection then finds the keys held and says so (rule 4).
    pub fn tick(&mut self, now: Instant) -> Option<Instant> {
        match self.phase {
            Phase::Held { deadline } if now >= deadline => {
                self.phase = Phase::InProgress;
                Some(deadline)
            }
            _ => None,
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
        assert_eq!(attempts.next_deadline(), Some(at(t0, 10_000)));
        assert_eq!(attempts.tick(at(t0, 1500)), None);
        assert_eq!(attempts.release(), Some(at(t0, 10_000)));
        assert_eq!(attempts.next_deadline(), None);
        assert!(attempts.busy());
    }

    /// Acceptance test 3: held for 3 s, the attempt still goes on at the release.
    #[test]
    fn keys_held_for_seconds_still_go_on_at_the_release() {
        let t0 = Instant::now();
        let mut attempts = VocabularyAttempts::new();

        attempts.press(t0, true);
        assert_eq!(attempts.tick(at(t0, 2000)), None);
        assert_eq!(attempts.tick(at(t0, 3000)), None);
        assert_eq!(attempts.release(), Some(at(t0, 10_000)));
        assert!(attempts.busy());
    }

    /// Acceptance test 3: still held 10 s after the press, the attempt goes on without a release
    /// (to show the release-the-keys message); the late release then does nothing.
    #[test]
    fn keys_held_for_ten_seconds_hand_the_attempt_over_at_the_deadline() {
        let t0 = Instant::now();
        let mut attempts = VocabularyAttempts::new();

        attempts.press(t0, true);
        assert_eq!(attempts.tick(at(t0, 9999)), None);
        assert_eq!(attempts.tick(at(t0, 10_000)), Some(at(t0, 10_000)));
        assert_eq!(attempts.next_deadline(), None);
        assert!(attempts.busy());
        assert_eq!(attempts.release(), None);
        attempts.finished();
        assert!(!attempts.busy());
    }

    /// Rule 5: nothing during a Dictation or while another attempt is in progress, and nothing
    /// is remembered for later.
    #[test]
    fn presses_while_busy_do_nothing() {
        let t0 = Instant::now();
        let mut attempts = VocabularyAttempts::new();

        assert!(!attempts.press(t0, false));
        assert_eq!(attempts.release(), None);
        assert!(!attempts.busy());

        assert!(attempts.press(at(t0, 100), true));
        assert!(!attempts.press(at(t0, 150), true));
        attempts.release();
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
