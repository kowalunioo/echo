//! Toggle Mode and Push-to-Talk Mode: turns presses and releases of the Record Shortcut into
//! "start Recording" / "stop Recording" intents (`record-shortcut.md` rules 1–5, 9–11).
//!
//! Pure logic: time comes in as arguments, so tests control the clock.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

/// A second press within this time of the previous accepted press is ignored (rule 10).
pub const DEBOUNCE: Duration = Duration::from_millis(30);

/// In Push-to-Talk Mode a release is acted on this long after it happens, unless a press
/// arrives in between (rule 11).
pub const RELEASE_GRACE: Duration = Duration::from_millis(50);

/// How the Record Shortcut starts and stops a Recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutMode {
    /// Hold to record (the default, rule 5).
    #[default]
    PushToTalk,
    /// Press to start, press again to stop.
    Toggle,
}

/// What the Record Shortcut asks the dictation pipeline to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RecordIntent {
    /// Start a Recording (or, while busy, remember the request — `dictation-pipeline.md` 27–29).
    Start,
    /// Stop the Recording (or, while busy, forget the remembered request).
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Recording {
        /// Whether releasing the shortcut stops this Recording: only in Push-to-Talk Mode, and
        /// only if the mode has not changed since it started (rule 4).
        release_stops: bool,
    },
}

/// The Toggle / Push-to-Talk state machine.
#[derive(Debug, Clone)]
pub struct RecordModes {
    mode: ShortcutMode,
    state: State,
    last_press: Option<Instant>,
    pending_release: Option<Instant>,
}

impl RecordModes {
    pub fn new(mode: ShortcutMode) -> Self {
        Self {
            mode,
            state: State::Idle,
            last_press: None,
            pending_release: None,
        }
    }

    pub fn mode(&self) -> ShortcutMode {
        self.mode
    }

    /// Whether this machine believes a Recording it started is in progress.
    pub fn is_recording(&self) -> bool {
        matches!(self.state, State::Recording { .. })
    }

    /// Changes the mode. A Recording in progress keeps going and is stopped by the next press,
    /// whichever mode it started in (rule 4).
    pub fn set_mode(&mut self, mode: ShortcutMode) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        if let State::Recording { release_stops } = &mut self.state {
            *release_stops = false;
            self.pending_release = None;
        }
    }

    /// A press of the Record Shortcut at `now`.
    pub fn press(&mut self, now: Instant) -> Option<RecordIntent> {
        // A press inside the release grace cancels the pending release: both are ignored and the
        // Recording continues (rule 11). This takes precedence over the debounce.
        if self.pending_release.take().is_some() {
            return None;
        }
        if self
            .last_press
            .is_some_and(|last| now.saturating_duration_since(last) < DEBOUNCE)
        {
            return None;
        }
        self.last_press = Some(now);
        match self.state {
            State::Idle => {
                self.state = State::Recording {
                    release_stops: self.mode == ShortcutMode::PushToTalk,
                };
                Some(RecordIntent::Start)
            }
            // In Toggle Mode this is the stopping press; in Push-to-Talk Mode it can only happen
            // after a mode change (rule 4) or a missed release, and stopping is the safe answer.
            State::Recording { .. } => self.stop(),
        }
    }

    /// A release of the Record Shortcut at `now`. In Push-to-Talk Mode it schedules the stop
    /// [`RELEASE_GRACE`] later; otherwise it is ignored.
    pub fn release(&mut self, now: Instant) -> Option<RecordIntent> {
        if let State::Recording {
            release_stops: true,
        } = self.state
        {
            self.pending_release = Some(now + RELEASE_GRACE);
        }
        None
    }

    /// When [`tick`](Self::tick) must next be called, if anything is pending.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.pending_release
    }

    /// Acts on a pending release whose grace period has passed by `now`.
    pub fn tick(&mut self, now: Instant) -> Option<RecordIntent> {
        match self.pending_release {
            Some(due) if now >= due => self.stop(),
            _ => None,
        }
    }

    /// The pipeline reports that the Recording ended without the shortcut (Cancellation, an
    /// error, or a start it could not honour), so the next press starts a fresh one and a
    /// Push-to-Talk release after it does nothing (`cancel-shortcut.md` rule 9).
    pub fn recording_ended(&mut self) {
        self.state = State::Idle;
        self.pending_release = None;
    }

    fn stop(&mut self) -> Option<RecordIntent> {
        self.state = State::Idle;
        self.pending_release = None;
        Some(RecordIntent::Stop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RecordIntent::{Start, Stop};

    /// A controllable clock: milliseconds since an arbitrary start.
    struct Clock(Instant);

    impl Clock {
        fn new() -> Self {
            Self(Instant::now())
        }
        fn at(&self, ms: u64) -> Instant {
            self.0 + Duration::from_millis(ms)
        }
    }

    #[test]
    fn push_to_talk_is_the_default() {
        assert_eq!(ShortcutMode::default(), ShortcutMode::PushToTalk);
    }

    /// Acceptance test 1.
    #[test]
    fn push_to_talk_starts_on_press_and_stops_50_ms_after_release() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::PushToTalk);

        assert_eq!(modes.press(t.at(0)), Some(Start));
        assert_eq!(modes.release(t.at(1000)), None);
        assert_eq!(modes.next_deadline(), Some(t.at(1050)));
        assert_eq!(modes.tick(t.at(1049)), None);
        assert_eq!(modes.tick(t.at(1050)), Some(Stop));
        assert!(!modes.is_recording());
        assert_eq!(modes.next_deadline(), None);
    }

    /// Acceptance test 2.
    #[test]
    fn a_press_within_the_release_grace_cancels_the_release() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::PushToTalk);

        modes.press(t.at(0));
        modes.release(t.at(1000));
        assert_eq!(modes.press(t.at(1020)), None);
        assert_eq!(modes.tick(t.at(1100)), None);
        assert!(modes.is_recording());

        modes.release(t.at(2000));
        assert_eq!(modes.tick(t.at(2050)), Some(Stop));
    }

    #[test]
    fn the_release_grace_wins_over_the_debounce() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::PushToTalk);

        modes.press(t.at(0));
        modes.release(t.at(5));
        assert_eq!(modes.press(t.at(15)), None);
        assert_eq!(modes.tick(t.at(100)), None);
        assert!(modes.is_recording());
    }

    /// Acceptance test 3.
    #[test]
    fn toggle_starts_on_one_press_and_stops_on_the_next_ignoring_releases() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::Toggle);

        assert_eq!(modes.press(t.at(0)), Some(Start));
        assert_eq!(modes.release(t.at(100)), None);
        assert_eq!(modes.next_deadline(), None);
        assert_eq!(modes.tick(t.at(10_000)), None);
        assert!(modes.is_recording());
        assert_eq!(modes.press(t.at(3000)), Some(Stop));
    }

    /// Acceptance test 4.
    #[test]
    fn a_second_press_within_30_ms_is_ignored() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::Toggle);

        assert_eq!(modes.press(t.at(0)), Some(Start));
        assert_eq!(modes.press(t.at(10)), None);
        assert_eq!(modes.press(t.at(29)), None);
        assert!(modes.is_recording());
        assert_eq!(modes.press(t.at(30)), Some(Stop));
    }

    /// Acceptance test 8.
    #[test]
    fn switching_to_toggle_mid_recording_leaves_the_stop_to_the_next_press() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::PushToTalk);

        modes.press(t.at(0));
        modes.set_mode(ShortcutMode::Toggle);
        assert_eq!(modes.release(t.at(500)), None);
        assert_eq!(modes.tick(t.at(1000)), None);
        assert!(modes.is_recording());
        assert_eq!(modes.press(t.at(2000)), Some(Stop));
    }

    #[test]
    fn switching_to_push_to_talk_mid_recording_also_waits_for_a_press() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::Toggle);

        modes.press(t.at(0));
        modes.release(t.at(100));
        modes.set_mode(ShortcutMode::PushToTalk);
        modes.release(t.at(500));
        assert_eq!(modes.tick(t.at(1000)), None);
        assert_eq!(modes.press(t.at(2000)), Some(Stop));
    }

    #[test]
    fn a_mode_change_during_the_release_grace_keeps_the_recording() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::PushToTalk);

        modes.press(t.at(0));
        modes.release(t.at(100));
        modes.set_mode(ShortcutMode::Toggle);
        assert_eq!(modes.tick(t.at(200)), None);
        assert!(modes.is_recording());
    }

    #[test]
    fn after_the_recording_ends_elsewhere_a_release_does_nothing_and_a_press_starts_afresh() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::PushToTalk);

        modes.press(t.at(0));
        modes.recording_ended();
        assert_eq!(modes.release(t.at(100)), None);
        assert_eq!(modes.tick(t.at(1000)), None);
        assert_eq!(modes.press(t.at(2000)), Some(Start));
    }

    #[test]
    fn a_release_with_nothing_recording_is_ignored() {
        let t = Clock::new();
        let mut modes = RecordModes::new(ShortcutMode::PushToTalk);

        assert_eq!(modes.release(t.at(0)), None);
        assert_eq!(modes.next_deadline(), None);
    }
}
