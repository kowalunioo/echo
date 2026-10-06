//! The Record Shortcut as a whole: keeps the configured combination bound on the
//! [`ShortcutListener`], validates changes, suspends it during shortcut capture, and turns its
//! presses into [`RecordIntent`]s through [`RecordModes`].
//!
//! [`RecordShortcut`] is synchronous and takes time as an argument, so tests drive it with the
//! fake listener and a controllable clock. [`spawn`] runs it for the app: a worker thread feeds
//! it the listener's events and the real clock, and hands intents to the pipeline.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::modes::{RecordIntent, RecordModes, ShortcutMode};
use super::validation::{
    ShortcutProblem, default_cancel_shortcut, default_record_shortcut, validate_record_shortcut,
};
use super::{
    CaptureSink, CapturedKey, KeyAction, KeyCombination, Shortcut, ShortcutError, ShortcutEvent,
    ShortcutListener, ShortcutSink,
};

/// The Record Shortcut settings (`record-shortcut.md`, "Settings").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordShortcutConfig {
    pub combination: KeyCombination,
    pub mode: ShortcutMode,
}

impl Default for RecordShortcutConfig {
    /// Ctrl+Space in Push-to-Talk Mode (rules 5, 18).
    fn default() -> Self {
        Self {
            combination: default_record_shortcut(),
            mode: ShortcutMode::default(),
        }
    }
}

/// Where the Record Shortcut settings are kept.
///
/// Temporary seam: the settings store (#23) is being built in parallel; until it lands, the app
/// uses [`InMemoryConfigStore`] and settings reset to the defaults on restart.
pub trait RecordShortcutConfigStore: Send {
    fn load(&self) -> RecordShortcutConfig;
    fn save(&mut self, config: &RecordShortcutConfig);
}

/// Keeps the settings in memory only.
#[derive(Debug, Clone, Default)]
pub struct InMemoryConfigStore(pub RecordShortcutConfig);

impl RecordShortcutConfigStore for InMemoryConfigStore {
    fn load(&self) -> RecordShortcutConfig {
        self.0.clone()
    }
    fn save(&mut self, config: &RecordShortcutConfig) {
        self.0 = config.clone();
    }
}

/// What the interface shows about the Record Shortcut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecordShortcutState {
    /// The current combination in canonical text form, e.g. `"Ctrl+Space"`.
    pub combination: String,
    pub mode: ShortcutMode,
    /// The combination "Reset to default" restores.
    pub default_combination: String,
}

/// Why a new Record Shortcut was not taken; the previous one stays active (rule 23).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ShortcutChangeError {
    /// The combination is not allowed (rule 20).
    #[error("{problem}")]
    #[serde(rename_all = "camelCase")]
    NotAllowed { problem: ShortcutProblem },
    /// The combination is allowed but could not be activated.
    #[error("cannot activate the shortcut: {reason}")]
    #[serde(rename_all = "camelCase")]
    ActivationFailed { reason: String },
}

/// The Record Shortcut logic around a [`ShortcutListener`].
pub struct RecordShortcut {
    listener: Box<dyn ShortcutListener>,
    store: Box<dyn RecordShortcutConfigStore>,
    config: RecordShortcutConfig,
    cancel: KeyCombination,
    modes: RecordModes,
    capturing: bool,
}

impl RecordShortcut {
    pub fn new(
        listener: Box<dyn ShortcutListener>,
        store: Box<dyn RecordShortcutConfigStore>,
    ) -> Self {
        let config = store.load();
        Self {
            listener,
            store,
            modes: RecordModes::new(config.mode),
            config,
            cancel: default_cancel_shortcut(),
            capturing: false,
        }
    }

    /// Starts the listener and binds the configured combination.
    pub fn start(&mut self, sink: ShortcutSink) -> Result<(), ShortcutError> {
        self.listener.start(sink)?;
        self.listener
            .bind(Shortcut::Record, Some(self.config.combination.clone()))
    }

    pub fn state(&self) -> RecordShortcutState {
        RecordShortcutState {
            combination: self.config.combination.to_string(),
            mode: self.config.mode,
            default_combination: default_record_shortcut().to_string(),
        }
    }

    /// Feeds one listener event at `now`; returns what the pipeline should do.
    pub fn handle(&mut self, event: ShortcutEvent, now: Instant) -> Option<RecordIntent> {
        if event.shortcut != Shortcut::Record || self.capturing {
            return None;
        }
        match event.action {
            KeyAction::Pressed => self.modes.press(now),
            KeyAction::Released => self.modes.release(now),
        }
    }

    /// When [`tick`](Self::tick) must next be called (Push-to-Talk release grace).
    pub fn next_deadline(&self) -> Option<Instant> {
        self.modes.next_deadline()
    }

    pub fn tick(&mut self, now: Instant) -> Option<RecordIntent> {
        self.modes.tick(now)
    }

    /// The pipeline reports that the Recording ended without the shortcut (Cancellation, an
    /// error, or a start it did not honour).
    pub fn recording_ended(&mut self) {
        self.modes.recording_ended();
    }

    /// Changes the mode at once, even during a Recording (rule 4).
    pub fn set_mode(&mut self, mode: ShortcutMode) -> RecordShortcutState {
        self.modes.set_mode(mode);
        self.config.mode = mode;
        self.store.save(&self.config);
        self.state()
    }

    /// Validates and activates a new combination given in text form (rules 20–23). Ends a
    /// capture in progress first.
    pub fn set_combination(
        &mut self,
        text: &str,
    ) -> Result<RecordShortcutState, ShortcutChangeError> {
        self.end_capture();
        let not_allowed = |problem| ShortcutChangeError::NotAllowed { problem };
        let combination: KeyCombination = text.parse().map_err(not_allowed)?;
        validate_record_shortcut(&combination, &self.cancel).map_err(not_allowed)?;
        self.listener
            .bind(Shortcut::Record, Some(combination.clone()))
            .map_err(|e| ShortcutChangeError::ActivationFailed {
                reason: e.to_string(),
            })?;
        self.config.combination = combination;
        self.store.save(&self.config);
        Ok(self.state())
    }

    /// Restores Ctrl+Space, subject to the same validation (rule 24).
    pub fn reset(&mut self) -> Result<RecordShortcutState, ShortcutChangeError> {
        self.set_combination(&default_record_shortcut().to_string())
    }

    /// Starts shortcut capture: the Record Shortcut is suspended (rule 14) and keys go to `sink`.
    pub fn begin_capture(&mut self, sink: CaptureSink) -> Result<(), ShortcutError> {
        self.listener.capture(Some(sink))?;
        self.capturing = true;
        if let Err(error) = self.listener.bind(Shortcut::Record, None) {
            self.end_capture();
            return Err(error);
        }
        Ok(())
    }

    /// Ends shortcut capture and reactivates the current combination.
    pub fn end_capture(&mut self) {
        if !self.capturing {
            return;
        }
        self.capturing = false;
        let _ = self.listener.capture(None);
        if let Err(error) = self
            .listener
            .bind(Shortcut::Record, Some(self.config.combination.clone()))
        {
            eprintln!("Echo: cannot reactivate the Record Shortcut: {error}");
        }
    }

    pub fn is_capturing(&self) -> bool {
        self.capturing
    }

    /// A key pressed or released in Echo's own window (see
    /// [`ShortcutListener::own_window_key`]).
    pub fn own_window_key(&mut self, key: &str, action: KeyAction) {
        self.listener.own_window_key(key, action);
    }
}

/// Receives the Record Shortcut's intents; this is what the dictation pipeline (#13) consumes.
/// Called on the shortcut worker thread.
pub type IntentSink = Box<dyn FnMut(RecordIntent) + Send>;

/// Receives captured keys for the shortcut-capture UI, on the shortcut worker thread.
pub type CapturedKeySink = Box<dyn FnMut(CapturedKey) + Send>;

enum Input {
    Shortcut(ShortcutEvent),
    Captured(CapturedKey),
}

/// A running Record Shortcut, shared by the commands and (later) the pipeline.
#[derive(Clone)]
pub struct RecordShortcutHandle {
    core: Arc<Mutex<RecordShortcut>>,
    inputs: Sender<Input>,
}

impl RecordShortcutHandle {
    fn core(&self) -> MutexGuard<'_, RecordShortcut> {
        self.core.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn state(&self) -> RecordShortcutState {
        self.core().state()
    }

    pub fn set_mode(&self, mode: ShortcutMode) -> RecordShortcutState {
        self.core().set_mode(mode)
    }

    pub fn set_combination(&self, text: &str) -> Result<RecordShortcutState, ShortcutChangeError> {
        self.core().set_combination(text)
    }

    pub fn reset(&self) -> Result<RecordShortcutState, ShortcutChangeError> {
        self.core().reset()
    }

    pub fn begin_capture(&self) -> Result<(), ShortcutError> {
        let inputs = self.inputs.clone();
        self.core().begin_capture(Box::new(move |key| {
            let _ = inputs.send(Input::Captured(key));
        }))
    }

    pub fn end_capture(&self) {
        self.core().end_capture();
    }

    pub fn own_window_key(&self, key: &str, action: KeyAction) {
        self.core().own_window_key(key, action);
    }

    /// For the pipeline: the Recording ended without the shortcut (see
    /// [`RecordShortcut::recording_ended`]).
    pub fn recording_ended(&self) {
        self.core().recording_ended();
    }
}

/// Starts `core` and a worker thread that turns its listener's events into intents for
/// `intents` and forwards captured keys to `captured`. A listener that cannot start is
/// reported as an error, but the handle still works for settings.
pub fn spawn(
    mut core: RecordShortcut,
    intents: IntentSink,
    captured: CapturedKeySink,
) -> (RecordShortcutHandle, Result<(), ShortcutError>) {
    let (tx, rx) = mpsc::channel();
    let events = tx.clone();
    let started = core.start(Box::new(move |event| {
        let _ = events.send(Input::Shortcut(event));
    }));
    let core = Arc::new(Mutex::new(core));
    let handle = RecordShortcutHandle {
        core: Arc::clone(&core),
        inputs: tx,
    };
    let worker = handle.clone();
    std::thread::Builder::new()
        .name("echo-record-shortcut".into())
        .spawn(move || run_worker(&worker, rx, intents, captured))
        .expect("spawn the Record Shortcut worker");
    (handle, started)
}

fn run_worker(
    handle: &RecordShortcutHandle,
    inputs: Receiver<Input>,
    mut intents: IntentSink,
    mut captured: CapturedKeySink,
) {
    loop {
        let deadline = handle.core().next_deadline();
        let input = match deadline {
            Some(due) => inputs.recv_timeout(due.saturating_duration_since(Instant::now())),
            None => inputs.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let intent = match input {
            Ok(Input::Shortcut(event)) => handle.core().handle(event, Instant::now()),
            Ok(Input::Captured(key)) => {
                captured(key);
                None
            }
            Err(RecvTimeoutError::Timeout) => handle.core().tick(Instant::now()),
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if let Some(intent) = intent {
            intents(intent);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::FakeShortcutListener;
    use super::*;
    use RecordIntent::{Start, Stop};

    struct Rig {
        fake: FakeShortcutListener,
        core: RecordShortcut,
        events: Receiver<ShortcutEvent>,
        t0: Instant,
    }

    impl Rig {
        fn new(mode: ShortcutMode) -> Self {
            let fake = FakeShortcutListener::new();
            let store = InMemoryConfigStore(RecordShortcutConfig {
                mode,
                ..RecordShortcutConfig::default()
            });
            let mut core = RecordShortcut::new(Box::new(fake.clone()), Box::new(store));
            let (tx, events) = mpsc::channel();
            core.start(Box::new(move |e| tx.send(e).unwrap())).unwrap();
            Self {
                fake,
                core,
                events,
                t0: Instant::now(),
            }
        }

        fn at(&self, ms: u64) -> Instant {
            self.t0 + Duration::from_millis(ms)
        }

        /// Delivers whatever the fake listener reported to the core at time `ms`.
        fn deliver(&mut self, ms: u64) -> Vec<RecordIntent> {
            let now = self.at(ms);
            let events: Vec<_> = self.events.try_iter().collect();
            events
                .into_iter()
                .filter_map(|e| self.core.handle(e, now))
                .collect()
        }

        fn press(&mut self, ms: u64) -> Vec<RecordIntent> {
            self.fake.press(Shortcut::Record);
            self.deliver(ms)
        }

        fn release(&mut self, ms: u64) -> Vec<RecordIntent> {
            self.fake.release(Shortcut::Record);
            self.deliver(ms)
        }
    }

    #[test]
    fn starts_with_ctrl_space_in_push_to_talk_mode() {
        let rig = Rig::new(ShortcutMode::PushToTalk);

        assert_eq!(
            rig.fake.binding(Shortcut::Record),
            Some(default_record_shortcut())
        );
        assert_eq!(
            rig.core.state(),
            RecordShortcutState {
                combination: "Ctrl+Space".into(),
                mode: ShortcutMode::PushToTalk,
                default_combination: "Ctrl+Space".into(),
            }
        );
    }

    /// Acceptance test 1, through the listener.
    #[test]
    fn push_to_talk_through_the_listener() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);

        assert_eq!(rig.press(0), vec![Start]);
        assert_eq!(rig.release(800), vec![]);
        assert_eq!(rig.core.next_deadline(), Some(rig.at(850)));
        assert_eq!(rig.core.tick(rig.at(850)), Some(Stop));
    }

    /// Acceptance test 3, through the listener.
    #[test]
    fn toggle_through_the_listener() {
        let mut rig = Rig::new(ShortcutMode::Toggle);

        assert_eq!(rig.press(0), vec![Start]);
        assert_eq!(rig.release(100), vec![]);
        assert_eq!(rig.press(900), vec![Stop]);
    }

    /// Acceptance test 8, through the listener.
    #[test]
    fn a_mode_change_mid_recording_is_stopped_by_the_next_press() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);

        rig.press(0);
        assert_eq!(
            rig.core.set_mode(ShortcutMode::Toggle).mode,
            ShortcutMode::Toggle
        );
        assert_eq!(rig.release(300), vec![]);
        assert_eq!(rig.core.tick(rig.at(1000)), None);
        assert_eq!(rig.press(2000), vec![Stop]);
    }

    #[test]
    fn the_cancel_shortcut_does_not_drive_the_record_modes() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        let mut listener = rig.fake.clone();
        listener
            .bind(Shortcut::Cancel, Some(default_cancel_shortcut()))
            .unwrap();

        rig.fake.press(Shortcut::Cancel);
        assert_eq!(rig.deliver(0), vec![]);
    }

    #[test]
    fn an_accepted_combination_takes_effect_immediately_and_is_saved() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);

        let state = rig.core.set_combination("Alt+Shift+Ctrl+D").unwrap();

        assert_eq!(state.combination, "Ctrl+Alt+Shift+D");
        assert_eq!(
            rig.fake.binding(Shortcut::Record).unwrap().to_string(),
            "Ctrl+Alt+Shift+D"
        );
        assert_eq!(
            rig.core.store.load().combination.to_string(),
            "Ctrl+Alt+Shift+D"
        );
    }

    #[test]
    fn a_disallowed_combination_is_rejected_and_the_old_one_stays() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);

        for (text, problem) in [
            ("Space", ShortcutProblem::NeedsModifier),
            ("Escape", ShortcutProblem::EscapeReserved),
            ("", ShortcutProblem::Empty),
            ("Ctrl+Nonsense", ShortcutProblem::Invalid),
        ] {
            assert_eq!(
                rig.core.set_combination(text),
                Err(ShortcutChangeError::NotAllowed { problem }),
                "{text}"
            );
        }
        assert_eq!(rig.core.state().combination, "Ctrl+Space");
        assert_eq!(rig.press(0), vec![Start]);
    }

    /// Acceptance test 13.
    #[test]
    fn a_failed_activation_keeps_the_old_combination_working() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.fake.reject_binds(Some("already in use"));

        let result = rig.core.set_combination("Ctrl+Alt+D");

        assert!(matches!(
            result,
            Err(ShortcutChangeError::ActivationFailed { .. })
        ));
        assert_eq!(rig.core.state().combination, "Ctrl+Space");
        assert_eq!(
            rig.fake.binding(Shortcut::Record),
            Some(default_record_shortcut())
        );
        assert_eq!(rig.press(0), vec![Start]);
    }

    #[test]
    fn reset_restores_ctrl_space() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);
        rig.core.set_combination("F9").unwrap();

        assert_eq!(rig.core.reset().unwrap().combination, "Ctrl+Space");
        assert_eq!(
            rig.fake.binding(Shortcut::Record),
            Some(default_record_shortcut())
        );
    }

    /// Acceptance test 15.
    #[test]
    fn the_record_shortcut_is_inactive_during_capture() {
        let mut rig = Rig::new(ShortcutMode::Toggle);

        rig.core.begin_capture(Box::new(|_| {})).unwrap();
        assert!(rig.fake.capturing());
        assert_eq!(rig.press(0), vec![]);
        assert_eq!(rig.fake.binding(Shortcut::Record), None);

        rig.core.end_capture();
        assert!(!rig.fake.capturing());
        assert_eq!(rig.press(100), vec![Start]);
    }

    #[test]
    fn setting_a_combination_ends_the_capture() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.core.begin_capture(Box::new(|_| {})).unwrap();

        rig.core.set_combination("Ctrl+Win").unwrap();

        assert!(!rig.core.is_capturing());
        assert!(!rig.fake.capturing());
        assert_eq!(
            rig.fake.binding(Shortcut::Record).unwrap().to_string(),
            "Ctrl+Win"
        );
    }

    #[test]
    fn a_rejected_proposal_from_capture_reactivates_the_old_combination() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.core.begin_capture(Box::new(|_| {})).unwrap();

        assert!(rig.core.set_combination("A").is_err());

        assert_eq!(
            rig.fake.binding(Shortcut::Record),
            Some(default_record_shortcut())
        );
    }

    #[test]
    fn after_the_pipeline_reports_the_end_a_release_does_nothing() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);

        rig.press(0);
        rig.core.recording_ended();
        assert_eq!(rig.release(100), vec![]);
        assert_eq!(rig.core.next_deadline(), None);
        assert_eq!(rig.press(1000), vec![Start]);
    }

    #[test]
    fn keys_from_echos_own_window_go_to_the_listener() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);

        rig.core.own_window_key("LeftCtrl", KeyAction::Pressed);
        rig.core.own_window_key("Space", KeyAction::Released);

        assert_eq!(
            rig.fake.own_window_keys(),
            vec![
                ("LeftCtrl".to_owned(), KeyAction::Pressed),
                ("Space".to_owned(), KeyAction::Released)
            ]
        );
    }

    #[test]
    fn the_worker_delivers_intents_with_the_real_clock() {
        let fake = FakeShortcutListener::new();
        let core = RecordShortcut::new(
            Box::new(fake.clone()),
            Box::new(InMemoryConfigStore::default()),
        );
        let (tx, intents) = mpsc::channel();
        let (key_tx, keys) = mpsc::channel();
        let (handle, started) = spawn(
            core,
            Box::new(move |i| tx.send(i).unwrap()),
            Box::new(move |k| key_tx.send(k).unwrap()),
        );
        started.unwrap();
        let wait = Duration::from_secs(5);

        fake.press(Shortcut::Record);
        assert_eq!(intents.recv_timeout(wait), Ok(Start));
        fake.release(Shortcut::Record);
        // The stop arrives only after the 50 ms release grace.
        assert_eq!(intents.recv_timeout(wait), Ok(Stop));

        handle.begin_capture().unwrap();
        fake.capture_key("LeftCtrl", KeyAction::Pressed);
        assert_eq!(keys.recv_timeout(wait).unwrap().key, "LeftCtrl");
        handle.end_capture();
    }
}
