//! The Record Shortcut as a whole: keeps the configured combination bound on the
//! [`ShortcutListener`] while the shortcut is active, validates changes, suspends it during
//! shortcut capture, and turns its presses into [`RecordIntent`]s through [`RecordModes`].
//!
//! The Cancel Shortcut and the Vocabulary Shortcut share the listener, so they are bound and
//! validated here too; Vocabulary Shortcut presses become [`VocabularyAttempt`]s through
//! [`VocabularyAttempts`].
//!
//! The combination and mode are settings (`settings::Settings::record_shortcut` and
//! `shortcut_mode`); this module does not store them. `app.rs` saves accepted changes and feeds
//! changes made elsewhere back in through [`RecordShortcutHandle::apply_combination`],
//! [`set_mode`](RecordShortcutHandle::set_mode) and [`set_active`](RecordShortcutHandle::set_active).
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
    CancelShortcutCombination, RecordShortcutCombination, ShortcutCombinations, ShortcutProblem,
    VocabularyShortcutCombination, default_cancel_shortcut, default_record_shortcut,
    default_vocabulary_shortcut, validate_shortcut,
};
use super::vocabulary::VocabularyAttempts;
use super::{
    CaptureSink, CapturedKey, KeyAction, KeyCombination, Shortcut, ShortcutError, ShortcutEvent,
    ShortcutListener, ShortcutSink,
};
use crate::dictation::DictationState;

/// The Record Shortcut settings (`record-shortcut.md`, "Settings") and the Cancel Shortcut
/// (`cancel-shortcut.md`), which must differ from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordShortcutConfig {
    pub combination: KeyCombination,
    pub mode: ShortcutMode,
    /// The Cancel Shortcut for the next Dictation (`cancel-shortcut.md` rule 12).
    pub cancel: KeyCombination,
    /// The Vocabulary Shortcut (`vocabulary-shortcut.md`); it holds its combination even while
    /// turned off (rule 19).
    pub vocabulary: KeyCombination,
    /// Whether the Vocabulary Shortcut is turned on (rule 20).
    pub vocabulary_enabled: bool,
}

impl RecordShortcutConfig {
    /// The three combinations, which must differ from one another.
    pub fn combinations(&self) -> ShortcutCombinations {
        ShortcutCombinations {
            record: self.combination.clone(),
            cancel: self.cancel.clone(),
            vocabulary: self.vocabulary.clone(),
        }
    }
}

impl Default for RecordShortcutConfig {
    /// Ctrl+Space in Push-to-Talk Mode (rules 5, 18), Escape to cancel (`cancel-shortcut.md`
    /// rule 10).
    fn default() -> Self {
        Self {
            combination: default_record_shortcut(),
            mode: ShortcutMode::default(),
            cancel: default_cancel_shortcut(),
            vocabulary: default_vocabulary_shortcut(),
            vocabulary_enabled: true,
        }
    }
}

/// A Vocabulary Shortcut press whose keys were released in time: the selection is to be read
/// once every key is up, by `release_by` at the latest (`vocabulary-shortcut.md` rule 4).
/// Dropping it ends the attempt, so the next press can start another (rule 5).
pub struct VocabularyAttempt {
    pub release_by: Instant,
    /// The Vocabulary Shortcut, for the "nothing selected" message.
    pub shortcut: KeyCombination,
    done: Option<FinishOnDrop>,
}

struct FinishOnDrop(Sender<Input>);

impl Drop for FinishOnDrop {
    fn drop(&mut self) {
        let _ = self.0.send(Input::VocabularyFinished);
    }
}

/// Why a new Record Shortcut was not taken; the previous one stays active (rule 23).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ShortcutChangeError {
    /// The combination is not allowed (rule 20).
    #[error("{problem}")]
    #[serde(rename_all = "camelCase")]
    NotAllowed { problem: ShortcutProblem },
    /// The combination is allowed but could not be activated (or saved).
    #[error("cannot activate the shortcut: {reason}")]
    #[serde(rename_all = "camelCase")]
    ActivationFailed { reason: String },
}

/// The Record Shortcut logic around a [`ShortcutListener`], and the Cancel Shortcut, which shares
/// the listener and is bound only while a Dictation is Recording or Transcribing
/// (`cancel-shortcut.md` rule 7).
pub struct RecordShortcut {
    listener: Box<dyn ShortcutListener>,
    config: RecordShortcutConfig,
    modes: RecordModes,
    capturing: bool,
    /// Whether the shortcut is in force: only once first-run setup is finished (rule 17).
    active: bool,
    /// Whether the Cancel Shortcut is bound now (during a Dictation).
    cancel_bound: bool,
    /// Whether the Dictation is Idle (the Vocabulary Shortcut does nothing otherwise).
    dictation_idle: bool,
    attempts: VocabularyAttempts,
}

impl RecordShortcut {
    /// A Record Shortcut with the given settings; `active` says whether first-run setup is done.
    pub fn new(
        listener: Box<dyn ShortcutListener>,
        config: RecordShortcutConfig,
        active: bool,
    ) -> Self {
        Self {
            listener,
            modes: RecordModes::new(config.mode),
            config,
            capturing: false,
            active,
            cancel_bound: false,
            dictation_idle: true,
            attempts: VocabularyAttempts::new(),
        }
    }

    /// Starts the listener and, if the shortcut is active, binds the configured combination;
    /// binds the Vocabulary Shortcut if it is turned on.
    pub fn start(&mut self, sink: ShortcutSink) -> Result<(), ShortcutError> {
        self.listener.start(sink)?;
        if let Err(error) = self.rebind_vocabulary() {
            log::warn!("cannot activate the Vocabulary Shortcut: {error}");
        }
        self.rebind()
    }

    pub fn config(&self) -> &RecordShortcutConfig {
        &self.config
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Feeds one listener event at `now`; returns what the pipeline should do.
    pub fn handle(&mut self, event: ShortcutEvent, now: Instant) -> Option<RecordIntent> {
        if event.shortcut != Shortcut::Record || self.capturing || !self.active {
            return None;
        }
        match event.action {
            KeyAction::Pressed => self.modes.press(now),
            KeyAction::Released => self.modes.release(now),
        }
    }

    /// Feeds one Vocabulary Shortcut event at `now`; returns the attempt to run once its keys
    /// are released in time (`vocabulary-shortcut.md` rules 4–5).
    pub fn handle_vocabulary(
        &mut self,
        event: ShortcutEvent,
        now: Instant,
    ) -> Option<VocabularyAttempt> {
        if event.shortcut != Shortcut::Vocabulary {
            return None;
        }
        match event.action {
            KeyAction::Pressed => {
                if !self.capturing && self.config.vocabulary_enabled {
                    self.attempts.press(now, self.dictation_idle);
                }
                None
            }
            KeyAction::Released => self
                .attempts
                .release(now)
                .map(|release_by| VocabularyAttempt {
                    release_by,
                    shortcut: self.config.vocabulary.clone(),
                    done: None,
                }),
        }
    }

    /// The attempt in progress ended; the next press may start another.
    pub fn vocabulary_finished(&mut self) {
        self.attempts.finished();
    }

    /// Whether `event` is a press of the Cancel Shortcut that must trigger Cancellation
    /// (`cancel-shortcut.md` rule 6: presses only, never releases).
    pub fn is_cancel(&self, event: ShortcutEvent) -> bool {
        event.shortcut == Shortcut::Cancel
            && event.action == KeyAction::Pressed
            && self.cancel_bound
            && !self.capturing
    }

    /// The Dictation's state changed: the Cancel Shortcut is bound while it is Recording or
    /// Transcribing and unbound otherwise (`cancel-shortcut.md` rules 4 and 7). It is bound with
    /// the combination in force when the Dictation began, so a change made during a Dictation
    /// applies from the next one (rule 12).
    pub fn dictation_state(&mut self, state: DictationState) {
        self.dictation_idle = state == DictationState::Idle;
        let wanted = matches!(
            state,
            DictationState::Recording | DictationState::Transcribing
        );
        if wanted == self.cancel_bound {
            return;
        }
        let binding = wanted.then(|| self.config.cancel.clone());
        match self.listener.bind(Shortcut::Cancel, binding) {
            Ok(()) => self.cancel_bound = wanted,
            Err(error) => log::warn!("cannot (un)bind the Cancel Shortcut: {error}"),
        }
    }

    /// Validates a new Cancel Shortcut given in text form (`cancel-shortcut.md` rule 11) and
    /// returns it as the value to save; it applies from the next Dictation (rule 12). Ends a
    /// capture in progress first.
    pub fn set_cancel_combination(
        &mut self,
        text: &str,
    ) -> Result<CancelShortcutCombination, ShortcutChangeError> {
        self.end_capture();
        let not_allowed = |problem| ShortcutChangeError::NotAllowed { problem };
        let combination: KeyCombination = text.parse().map_err(not_allowed)?;
        validate_shortcut(Shortcut::Cancel, &combination, &self.config.combinations())
            .map_err(not_allowed)?;
        let setting =
            CancelShortcutCombination::try_from(combination.clone()).map_err(not_allowed)?;
        self.apply_cancel_combination(&combination)
            .map_err(not_allowed)?;
        Ok(setting)
    }

    /// Takes a Cancel Shortcut changed in the settings (e.g. by "Reset to default") for the next
    /// Dictation, unless another shortcut holds it.
    pub fn apply_cancel_combination(
        &mut self,
        combination: &KeyCombination,
    ) -> Result<(), ShortcutProblem> {
        validate_shortcut(Shortcut::Cancel, combination, &self.config.combinations())?;
        self.config.cancel = combination.clone();
        Ok(())
    }

    /// When [`tick`](Self::tick) must next be called (Push-to-Talk release grace, a held
    /// Vocabulary Shortcut).
    pub fn next_deadline(&self) -> Option<Instant> {
        let modes = self.modes.next_deadline();
        modes.into_iter().chain(self.attempts.next_deadline()).min()
    }

    pub fn tick(&mut self, now: Instant) -> Option<RecordIntent> {
        self.attempts.tick(now);
        self.modes.tick(now)
    }

    /// The pipeline reports that the Recording ended without the shortcut (Cancellation, an
    /// error, or a start it did not honour).
    pub fn recording_ended(&mut self) {
        self.modes.recording_ended();
    }

    /// Changes the mode at once, even during a Recording (rule 4).
    pub fn set_mode(&mut self, mode: ShortcutMode) {
        self.modes.set_mode(mode);
        self.config.mode = mode;
    }

    /// Turns the shortcut on or off: it is off until first-run setup is finished (rule 17).
    pub fn set_active(&mut self, active: bool) -> Result<(), ShortcutError> {
        if active == self.active {
            return Ok(());
        }
        self.active = active;
        if !active {
            self.modes.recording_ended();
        }
        self.rebind()
    }

    /// Validates and activates a new combination given in text form (rules 20–23) and returns
    /// it as the value to save. Ends a capture in progress first. If it fails, the previous
    /// combination stays in force.
    pub fn set_combination(
        &mut self,
        text: &str,
    ) -> Result<RecordShortcutCombination, ShortcutChangeError> {
        self.end_capture();
        let not_allowed = |problem| ShortcutChangeError::NotAllowed { problem };
        let combination: KeyCombination = text.parse().map_err(not_allowed)?;
        validate_shortcut(Shortcut::Record, &combination, &self.config.combinations())
            .map_err(not_allowed)?;
        let setting =
            RecordShortcutCombination::try_from(combination.clone()).map_err(not_allowed)?;
        self.apply_combination(&combination).map_err(|e| {
            ShortcutChangeError::ActivationFailed {
                reason: e.to_string(),
            }
        })?;
        Ok(setting)
    }

    /// Puts an already validated combination in force (e.g. one changed in the settings by
    /// "Restore default"). On failure, including a combination another shortcut holds
    /// (rule 20), the previous combination stays in force.
    pub fn apply_combination(&mut self, combination: &KeyCombination) -> Result<(), ShortcutError> {
        if *combination == self.config.combination {
            return Ok(());
        }
        if let Some(problem) = self
            .config
            .combinations()
            .held_by_other(Shortcut::Record, combination)
        {
            return Err(ShortcutError::Rejected {
                combination: combination.to_string(),
                reason: problem.to_string(),
            });
        }
        if self.active && !self.capturing {
            self.listener
                .bind(Shortcut::Record, Some(combination.clone()))?;
        }
        self.config.combination = combination.clone();
        Ok(())
    }

    /// Validates and activates a new Vocabulary Shortcut given in text form
    /// (`vocabulary-shortcut.md` rules 18–21) and returns it as the value to save. Ends a capture
    /// in progress first. If it fails, the previous combination stays in force.
    pub fn set_vocabulary_combination(
        &mut self,
        text: &str,
    ) -> Result<VocabularyShortcutCombination, ShortcutChangeError> {
        self.end_capture();
        let not_allowed = |problem| ShortcutChangeError::NotAllowed { problem };
        let combination: KeyCombination = text.parse().map_err(not_allowed)?;
        validate_shortcut(
            Shortcut::Vocabulary,
            &combination,
            &self.config.combinations(),
        )
        .map_err(not_allowed)?;
        let setting =
            VocabularyShortcutCombination::try_from(combination.clone()).map_err(not_allowed)?;
        self.apply_vocabulary_combination(&combination)
            .map_err(|e| ShortcutChangeError::ActivationFailed {
                reason: e.to_string(),
            })?;
        Ok(setting)
    }

    /// Puts an already validated Vocabulary Shortcut in force (e.g. one reset to its default in
    /// the settings). On failure, including a combination another shortcut holds, the previous
    /// combination stays in force (rule 21).
    pub fn apply_vocabulary_combination(
        &mut self,
        combination: &KeyCombination,
    ) -> Result<(), ShortcutError> {
        if *combination == self.config.vocabulary {
            return Ok(());
        }
        if let Some(problem) = self
            .config
            .combinations()
            .held_by_other(Shortcut::Vocabulary, combination)
        {
            return Err(ShortcutError::Rejected {
                combination: combination.to_string(),
                reason: problem.to_string(),
            });
        }
        if self.config.vocabulary_enabled && !self.capturing {
            self.listener
                .bind(Shortcut::Vocabulary, Some(combination.clone()))?;
        }
        self.config.vocabulary = combination.clone();
        Ok(())
    }

    /// Turns the Vocabulary Shortcut on or off at once (rules 20–21). While off its combination
    /// is not intercepted. If the change cannot be activated, the previous state stays.
    pub fn set_vocabulary_enabled(&mut self, enabled: bool) -> Result<(), ShortcutError> {
        if enabled == self.config.vocabulary_enabled {
            return Ok(());
        }
        let binding = (enabled && !self.capturing).then(|| self.config.vocabulary.clone());
        self.listener.bind(Shortcut::Vocabulary, binding)?;
        self.config.vocabulary_enabled = enabled;
        if !enabled {
            self.attempts.cancel_held();
        }
        Ok(())
    }

    /// Starts shortcut capture: the Record Shortcut and the Vocabulary Shortcut are suspended
    /// (rule 14, `vocabulary-shortcut.md` rule 5) and keys go to `sink`.
    pub fn begin_capture(&mut self, sink: CaptureSink) -> Result<(), ShortcutError> {
        self.listener.capture(Some(sink))?;
        self.capturing = true;
        let unbound = self
            .listener
            .bind(Shortcut::Record, None)
            .and_then(|()| self.listener.bind(Shortcut::Vocabulary, None));
        if let Err(error) = unbound {
            self.end_capture();
            return Err(error);
        }
        self.attempts.cancel_held();
        Ok(())
    }

    /// Ends shortcut capture and reactivates the current combination.
    pub fn end_capture(&mut self) {
        if !self.capturing {
            return;
        }
        self.capturing = false;
        let _ = self.listener.capture(None);
        if let Err(error) = self.rebind() {
            log::warn!("cannot reactivate the Record Shortcut: {error}");
        }
        if let Err(error) = self.rebind_vocabulary() {
            log::warn!("cannot reactivate the Vocabulary Shortcut: {error}");
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

    /// Binds the combination if the shortcut is in force now, otherwise clears the binding.
    fn rebind(&mut self) -> Result<(), ShortcutError> {
        let binding = (self.active && !self.capturing).then(|| self.config.combination.clone());
        self.listener.bind(Shortcut::Record, binding)
    }

    /// Binds the Vocabulary Shortcut if it is turned on and no capture is active, otherwise
    /// clears the binding.
    fn rebind_vocabulary(&mut self) -> Result<(), ShortcutError> {
        let binding = (self.config.vocabulary_enabled && !self.capturing)
            .then(|| self.config.vocabulary.clone());
        self.listener.bind(Shortcut::Vocabulary, binding)
    }
}

/// Receives the Record Shortcut's intents; this is what the dictation pipeline (#13) consumes.
/// Called on the shortcut worker thread.
pub type IntentSink = Box<dyn FnMut(RecordIntent) + Send>;

/// Receives captured keys for the shortcut-capture UI, on the shortcut worker thread.
pub type CapturedKeySink = Box<dyn FnMut(CapturedKey) + Send>;

/// Called on the shortcut worker thread when the Cancel Shortcut is pressed during a Dictation;
/// the app triggers Cancellation (`Dictation::cancel`).
pub type CancelSink = Box<dyn FnMut() + Send>;

/// Called on the shortcut worker thread with each Vocabulary Shortcut attempt to run. It must
/// return quickly (run the attempt on another thread) and drop the attempt when it is done.
pub type VocabularySink = Box<dyn FnMut(VocabularyAttempt) + Send>;

enum Input {
    Shortcut(ShortcutEvent),
    Captured(CapturedKey),
    VocabularyFinished,
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

    /// The combination and mode currently in force.
    pub fn config(&self) -> RecordShortcutConfig {
        self.core().config().clone()
    }

    pub fn set_mode(&self, mode: ShortcutMode) {
        self.core().set_mode(mode);
    }

    /// Turns the shortcut on once first-run setup is finished, off otherwise (rule 17).
    pub fn set_active(&self, active: bool) -> Result<(), ShortcutError> {
        self.core().set_active(active)
    }

    /// Validates and activates a combination typed or captured by the user; returns the value
    /// to save in the settings.
    pub fn set_combination(
        &self,
        text: &str,
    ) -> Result<RecordShortcutCombination, ShortcutChangeError> {
        self.core().set_combination(text)
    }

    /// Puts a combination changed in the settings in force.
    pub fn apply_combination(&self, combination: &KeyCombination) -> Result<(), ShortcutError> {
        self.core().apply_combination(combination)
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

    /// For the pipeline: the Dictation's state changed (see [`RecordShortcut::dictation_state`]).
    pub fn dictation_state(&self, state: DictationState) {
        self.core().dictation_state(state);
    }

    /// Validates a Cancel Shortcut captured by the user; returns the value to save.
    pub fn set_cancel_combination(
        &self,
        text: &str,
    ) -> Result<CancelShortcutCombination, ShortcutChangeError> {
        self.core().set_cancel_combination(text)
    }

    /// Takes a Cancel Shortcut changed in the settings for the next Dictation.
    pub fn apply_cancel_combination(
        &self,
        combination: &KeyCombination,
    ) -> Result<(), ShortcutProblem> {
        self.core().apply_cancel_combination(combination)
    }

    /// Validates and activates a Vocabulary Shortcut captured by the user; returns the value to
    /// save.
    pub fn set_vocabulary_combination(
        &self,
        text: &str,
    ) -> Result<VocabularyShortcutCombination, ShortcutChangeError> {
        self.core().set_vocabulary_combination(text)
    }

    /// Puts a Vocabulary Shortcut changed in the settings in force.
    pub fn apply_vocabulary_combination(
        &self,
        combination: &KeyCombination,
    ) -> Result<(), ShortcutError> {
        self.core().apply_vocabulary_combination(combination)
    }

    /// Turns the Vocabulary Shortcut on or off.
    pub fn set_vocabulary_enabled(&self, enabled: bool) -> Result<(), ShortcutError> {
        self.core().set_vocabulary_enabled(enabled)
    }
}

/// Starts `core` and a worker thread that turns its listener's events into intents for
/// `intents`, Cancel Shortcut presses into calls of `cancel` and Vocabulary Shortcut attempts
/// into calls of `vocabulary`, and forwards captured keys to `captured`. A listener that cannot
/// start is reported as an error, but the handle still works for settings.
pub fn spawn(
    mut core: RecordShortcut,
    intents: IntentSink,
    captured: CapturedKeySink,
    cancel: CancelSink,
    vocabulary: VocabularySink,
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
        .spawn(move || {
            let sinks = Sinks {
                intents,
                captured,
                cancel,
                vocabulary,
            };
            run_worker(&worker, &rx, sinks);
        })
        .expect("spawn the Record Shortcut worker");
    (handle, started)
}

struct Sinks {
    intents: IntentSink,
    captured: CapturedKeySink,
    cancel: CancelSink,
    vocabulary: VocabularySink,
}

fn run_worker(handle: &RecordShortcutHandle, inputs: &Receiver<Input>, mut sinks: Sinks) {
    loop {
        let deadline = handle.core().next_deadline();
        let input = match deadline {
            Some(due) => inputs.recv_timeout(due.saturating_duration_since(Instant::now())),
            None => inputs.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let intent = match input {
            Ok(Input::Shortcut(event)) if event.shortcut == Shortcut::Vocabulary => {
                let attempt = handle.core().handle_vocabulary(event, Instant::now());
                if let Some(mut attempt) = attempt {
                    attempt.done = Some(FinishOnDrop(handle.inputs.clone()));
                    (sinks.vocabulary)(attempt);
                }
                None
            }
            Ok(Input::Shortcut(event)) => {
                let is_cancel = handle.core().is_cancel(event);
                if is_cancel {
                    (sinks.cancel)();
                    None
                } else {
                    handle.core().handle(event, Instant::now())
                }
            }
            Ok(Input::Captured(key)) => {
                (sinks.captured)(key);
                None
            }
            Ok(Input::VocabularyFinished) => {
                handle.core().vocabulary_finished();
                None
            }
            Err(RecvTimeoutError::Timeout) => handle.core().tick(Instant::now()),
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if let Some(intent) = intent {
            (sinks.intents)(intent);
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
            Self::with(mode, true)
        }

        fn with(mode: ShortcutMode, active: bool) -> Self {
            let fake = FakeShortcutListener::new();
            let config = RecordShortcutConfig {
                mode,
                ..RecordShortcutConfig::default()
            };
            let mut core = RecordShortcut::new(Box::new(fake.clone()), config, active);
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

        fn bound(&self) -> Option<String> {
            self.fake.binding(Shortcut::Record).map(|c| c.to_string())
        }
    }

    #[test]
    fn starts_with_its_combination_bound() {
        let rig = Rig::new(ShortcutMode::PushToTalk);

        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Space"));
        assert_eq!(rig.core.config(), &RecordShortcutConfig::default());
    }

    /// Rule 17: inactive until first-run setup is finished.
    #[test]
    fn the_shortcut_is_inactive_until_first_run_setup_is_finished() {
        let mut rig = Rig::with(ShortcutMode::Toggle, false);

        assert_eq!(rig.bound(), None);
        assert_eq!(rig.press(0), vec![]);

        rig.core.set_active(true).unwrap();
        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Space"));
        assert_eq!(rig.press(100), vec![Start]);
    }

    #[test]
    fn deactivating_unbinds_and_forgets_the_recording() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.press(0);

        rig.core.set_active(false).unwrap();

        assert_eq!(rig.bound(), None);
        rig.core.set_active(true).unwrap();
        assert_eq!(
            rig.press(1000),
            vec![Start],
            "a fresh Recording, not a stop"
        );
    }

    #[test]
    fn a_combination_chosen_while_inactive_is_bound_on_activation() {
        let mut rig = Rig::with(ShortcutMode::Toggle, false);

        rig.core.set_combination("F9").unwrap();
        assert_eq!(rig.bound(), None);

        rig.core.set_active(true).unwrap();
        assert_eq!(rig.bound().as_deref(), Some("F9"));
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
        rig.core.set_mode(ShortcutMode::Toggle);
        assert_eq!(rig.core.config().mode, ShortcutMode::Toggle);
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
    fn an_accepted_combination_takes_effect_immediately_and_is_returned_for_saving() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);

        let saved = rig.core.set_combination("Alt+Shift+Ctrl+D").unwrap();

        assert_eq!(String::from(saved), "Ctrl+Alt+Shift+D");
        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Alt+Shift+D"));
        assert_eq!(
            rig.core.config().combination.to_string(),
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
        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Space"));
        assert_eq!(rig.press(0), vec![Start]);
    }

    /// Acceptance test 13.
    #[test]
    fn a_failed_activation_keeps_the_old_combination_working() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.fake.reject_binds(Some("already in use"));

        let result = rig.core.set_combination("Ctrl+Alt+R");

        assert!(matches!(
            result,
            Err(ShortcutChangeError::ActivationFailed { .. })
        ));
        assert_eq!(rig.core.config().combination, default_record_shortcut());
        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Space"));
        assert_eq!(rig.press(0), vec![Start]);
    }

    /// Rule 24 through the settings: "Restore default" changes the setting, which is applied.
    #[test]
    fn a_combination_changed_in_the_settings_is_applied() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);
        rig.core.set_combination("F9").unwrap();

        rig.core
            .apply_combination(&default_record_shortcut())
            .unwrap();

        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Space"));
    }

    #[test]
    fn a_settings_change_that_cannot_be_activated_keeps_the_old_combination() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);
        rig.fake.reject_binds(Some("already in use"));

        let f9 = "F9".parse().unwrap();
        assert!(rig.core.apply_combination(&f9).is_err());

        assert_eq!(rig.core.config().combination, default_record_shortcut());
        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Space"));
    }

    /// Acceptance test 15.
    #[test]
    fn the_record_shortcut_is_inactive_during_capture() {
        let mut rig = Rig::new(ShortcutMode::Toggle);

        rig.core.begin_capture(Box::new(|_| {})).unwrap();
        assert!(rig.fake.capturing());
        assert_eq!(rig.press(0), vec![]);
        assert_eq!(rig.bound(), None);

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
        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Win"));
    }

    #[test]
    fn a_rejected_proposal_from_capture_reactivates_the_old_combination() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.core.begin_capture(Box::new(|_| {})).unwrap();

        assert!(rig.core.set_combination("A").is_err());

        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Space"));
    }

    #[test]
    fn capture_while_inactive_leaves_the_shortcut_unbound() {
        let mut rig = Rig::with(ShortcutMode::Toggle, false);

        rig.core.begin_capture(Box::new(|_| {})).unwrap();
        rig.core.end_capture();

        assert_eq!(rig.bound(), None);
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
            RecordShortcutConfig::default(),
            true,
        );
        let (tx, intents) = mpsc::channel();
        let (key_tx, keys) = mpsc::channel();
        let (cancel_tx, cancels) = mpsc::channel();
        let (vocabulary_tx, attempts) = mpsc::channel();
        let (handle, started) = spawn(
            core,
            Box::new(move |i| tx.send(i).unwrap()),
            Box::new(move |k| key_tx.send(k).unwrap()),
            Box::new(move || cancel_tx.send(()).unwrap()),
            Box::new(move |a| vocabulary_tx.send(a).unwrap()),
        );
        started.unwrap();
        let wait = Duration::from_secs(5);

        fake.press(Shortcut::Record);
        assert_eq!(intents.recv_timeout(wait), Ok(Start));
        fake.release(Shortcut::Record);
        // The stop arrives only after the 50 ms release grace.
        assert_eq!(intents.recv_timeout(wait), Ok(Stop));

        handle.dictation_state(DictationState::Transcribing);
        fake.press(Shortcut::Cancel);
        assert_eq!(cancels.recv_timeout(wait), Ok(()));

        handle.begin_capture().unwrap();
        fake.capture_key("LeftCtrl", KeyAction::Pressed);
        assert_eq!(keys.recv_timeout(wait).unwrap().key, "LeftCtrl");
        handle.end_capture();

        // vocabulary-shortcut.md: an attempt arrives at the release, and a second press is
        // ignored until the first attempt is dropped (rule 5).
        handle.dictation_state(DictationState::Idle);
        fake.press(Shortcut::Vocabulary);
        fake.release(Shortcut::Vocabulary);
        let attempt = attempts.recv_timeout(wait).unwrap();
        assert_eq!(attempt.shortcut.to_string(), "Ctrl+Alt+D");
        fake.press(Shortcut::Vocabulary);
        fake.release(Shortcut::Vocabulary);
        assert!(attempts.recv_timeout(Duration::from_millis(100)).is_err());
        drop(attempt);
        let deadline = Instant::now() + wait;
        loop {
            fake.press(Shortcut::Vocabulary);
            fake.release(Shortcut::Vocabulary);
            if attempts.recv_timeout(Duration::from_millis(50)).is_ok() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the finished attempt was not forgotten"
            );
        }
    }

    /// Delivers the Vocabulary Shortcut events the fake reported at `ms`; returns the attempts.
    fn vocabulary_attempts(rig: &mut Rig, ms: u64) -> Vec<VocabularyAttempt> {
        let now = rig.at(ms);
        let events: Vec<_> = rig.events.try_iter().collect();
        events
            .into_iter()
            .filter_map(|e| rig.core.handle_vocabulary(e, now))
            .collect()
    }

    fn vocabulary_bound(rig: &Rig) -> Option<String> {
        rig.fake
            .binding(Shortcut::Vocabulary)
            .map(|c| c.to_string())
    }

    #[test]
    fn the_vocabulary_shortcut_is_bound_when_turned_on_and_unbound_when_off() {
        let mut rig = Rig::with(ShortcutMode::Toggle, false);
        // Bound even before first-run setup is finished: it does not start a Dictation.
        assert_eq!(vocabulary_bound(&rig).as_deref(), Some("Ctrl+Alt+D"));

        // Acceptance test 12: turned off, the combination is not intercepted.
        rig.core.set_vocabulary_enabled(false).unwrap();
        assert_eq!(vocabulary_bound(&rig), None);
        assert!(!rig.fake.press(Shortcut::Vocabulary));

        rig.core.set_vocabulary_enabled(true).unwrap();
        assert_eq!(vocabulary_bound(&rig).as_deref(), Some("Ctrl+Alt+D"));
    }

    /// Acceptance test 2.
    #[test]
    fn the_attempt_starts_only_at_the_release() {
        let mut rig = Rig::new(ShortcutMode::Toggle);

        rig.fake.press(Shortcut::Vocabulary);
        assert!(vocabulary_attempts(&mut rig, 0).is_empty());
        rig.core.tick(rig.at(1000));
        rig.fake.release(Shortcut::Vocabulary);
        let attempts = vocabulary_attempts(&mut rig, 1200);
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].release_by, rig.at(2000));
    }

    /// Acceptance test 3.
    #[test]
    fn keys_held_for_more_than_two_seconds_drop_the_attempt() {
        let mut rig = Rig::new(ShortcutMode::Toggle);

        rig.fake.press(Shortcut::Vocabulary);
        vocabulary_attempts(&mut rig, 0);
        assert_eq!(rig.core.next_deadline(), Some(rig.at(2000)));
        rig.core.tick(rig.at(2000));
        rig.fake.release(Shortcut::Vocabulary);
        assert!(vocabulary_attempts(&mut rig, 2100).is_empty());
        assert_eq!(rig.core.next_deadline(), None);
    }

    /// Acceptance test 6 and rule 5: nothing during a Dictation, and the Recording goes on.
    #[test]
    fn the_vocabulary_shortcut_does_nothing_during_a_dictation() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        assert_eq!(rig.press(0), vec![Start]);
        rig.release(100);
        rig.core.dictation_state(DictationState::Recording);

        rig.fake.press(Shortcut::Vocabulary);
        rig.fake.release(Shortcut::Vocabulary);
        assert!(vocabulary_attempts(&mut rig, 200).is_empty());
        assert_eq!(rig.press(300), vec![Stop]);

        // Nothing was remembered for later.
        rig.core.dictation_state(DictationState::Idle);
        assert!(vocabulary_attempts(&mut rig, 400).is_empty());
    }

    #[test]
    fn the_vocabulary_shortcut_is_suspended_during_capture() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.core.begin_capture(Box::new(|_| {})).unwrap();
        assert_eq!(vocabulary_bound(&rig), None);
        rig.core.end_capture();
        assert_eq!(vocabulary_bound(&rig).as_deref(), Some("Ctrl+Alt+D"));

        rig.core.set_vocabulary_enabled(false).unwrap();
        rig.core.begin_capture(Box::new(|_| {})).unwrap();
        rig.core.end_capture();
        assert_eq!(vocabulary_bound(&rig), None);
    }

    /// Rules 19 and 21; acceptance test 10 (core half).
    #[test]
    fn a_new_vocabulary_shortcut_takes_effect_at_once_and_must_differ_from_the_others() {
        let mut rig = Rig::new(ShortcutMode::Toggle);

        let saved = rig.core.set_vocabulary_combination("Alt+Ctrl+V").unwrap();
        assert_eq!(String::from(saved), "Ctrl+Alt+V");
        assert_eq!(vocabulary_bound(&rig).as_deref(), Some("Ctrl+Alt+V"));

        for (text, problem) in [
            ("Ctrl+Space", ShortcutProblem::SameAsRecord),
            ("Escape", ShortcutProblem::EscapeReserved),
            ("Ctrl+C", ShortcutProblem::EditingCombination),
        ] {
            assert_eq!(
                rig.core.set_vocabulary_combination(text),
                Err(ShortcutChangeError::NotAllowed { problem }),
                "{text}"
            );
        }
        assert_eq!(vocabulary_bound(&rig).as_deref(), Some("Ctrl+Alt+V"));

        // A turned-off Vocabulary Shortcut still holds its combination.
        rig.core.set_vocabulary_enabled(false).unwrap();
        assert_eq!(
            rig.core.set_combination("Ctrl+Alt+V"),
            Err(ShortcutChangeError::NotAllowed {
                problem: ShortcutProblem::SameAsVocabulary
            })
        );
        assert_eq!(
            rig.core.set_cancel_combination("Ctrl+Alt+V"),
            Err(ShortcutChangeError::NotAllowed {
                problem: ShortcutProblem::SameAsVocabulary
            })
        );
        assert!(
            rig.core
                .apply_combination(&"Ctrl+Alt+V".parse().unwrap())
                .is_err()
        );
        assert!(
            rig.core
                .apply_vocabulary_combination(&"Ctrl+Space".parse().unwrap())
                .is_err()
        );
        // Changed while off: stored, not bound.
        rig.core.set_vocabulary_combination("Ctrl+Alt+W").unwrap();
        assert_eq!(vocabulary_bound(&rig), None);
        assert_eq!(rig.core.config().vocabulary.to_string(), "Ctrl+Alt+W");
    }

    #[test]
    fn a_vocabulary_shortcut_that_cannot_be_activated_keeps_the_old_one() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.fake.reject_binds(Some("taken"));

        assert!(matches!(
            rig.core.set_vocabulary_combination("Ctrl+Alt+V"),
            Err(ShortcutChangeError::ActivationFailed { .. })
        ));
        assert_eq!(rig.core.config().vocabulary.to_string(), "Ctrl+Alt+D");
        assert!(rig.core.set_vocabulary_enabled(false).is_err());
        assert!(rig.core.config().vocabulary_enabled);
    }

    fn cancel_bound(rig: &Rig) -> Option<String> {
        rig.fake.binding(Shortcut::Cancel).map(|c| c.to_string())
    }

    /// Delivers whatever the fake listener reported; returns how many asked for Cancellation.
    fn cancellations(rig: &mut Rig) -> usize {
        let events: Vec<_> = rig.events.try_iter().collect();
        events
            .into_iter()
            .filter(|e| rig.core.is_cancel(*e))
            .count()
    }

    // cancel-shortcut.md rule 7: active only while Recording or Transcribing, so Escape reaches
    // applications whenever Echo is Idle (or Inserting, rule 4).
    #[test]
    fn the_cancel_shortcut_is_bound_only_while_recording_or_transcribing() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        assert_eq!(cancel_bound(&rig), None);

        rig.core.dictation_state(DictationState::Recording);
        assert_eq!(cancel_bound(&rig).as_deref(), Some("Escape"));
        rig.core.dictation_state(DictationState::Transcribing);
        assert_eq!(cancel_bound(&rig).as_deref(), Some("Escape"));
        rig.core.dictation_state(DictationState::Inserting);
        assert_eq!(cancel_bound(&rig), None);
        rig.core.dictation_state(DictationState::Idle);
        assert_eq!(cancel_bound(&rig), None);
    }

    // Rule 6: the press cancels, the release does nothing.
    #[test]
    fn a_cancel_press_asks_for_cancellation_and_its_release_does_not() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.core.dictation_state(DictationState::Recording);

        rig.fake.press(Shortcut::Cancel);
        assert_eq!(cancellations(&mut rig), 1);
        rig.fake.release(Shortcut::Cancel);
        assert_eq!(cancellations(&mut rig), 0);
    }

    #[test]
    fn a_cancel_press_while_idle_is_not_even_reported() {
        let mut rig = Rig::new(ShortcutMode::Toggle);

        assert!(!rig.fake.press(Shortcut::Cancel));
        assert_eq!(cancellations(&mut rig), 0);
    }

    // Acceptance tests 6 and 7 at the shortcut level: hold Record (Push-to-Talk), press Cancel,
    // the pipeline reports the end; releasing Record then does nothing and the next press starts
    // a fresh Recording.
    #[test]
    fn after_a_cancellation_the_release_does_nothing_and_the_next_press_starts_afresh() {
        let mut rig = Rig::new(ShortcutMode::PushToTalk);
        assert_eq!(rig.press(0), vec![Start]);
        rig.core.dictation_state(DictationState::Recording);

        rig.fake.press(Shortcut::Cancel);
        assert_eq!(cancellations(&mut rig), 1);
        rig.core.recording_ended();
        rig.core.dictation_state(DictationState::Idle);

        assert_eq!(rig.release(500), vec![]);
        let later = rig.at(600);
        assert_eq!(rig.core.tick(later), None);
        assert_eq!(rig.press(1000), vec![Start]);
    }

    // Rule 12: a change made during a Dictation applies from the next one.
    #[test]
    fn a_cancel_shortcut_changed_during_a_dictation_applies_from_the_next_one() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.core.dictation_state(DictationState::Recording);

        let saved = rig.core.set_cancel_combination("Shift+Ctrl+Q").unwrap();
        assert_eq!(String::from(saved), "Ctrl+Shift+Q");
        rig.core.dictation_state(DictationState::Transcribing);
        assert_eq!(cancel_bound(&rig).as_deref(), Some("Escape"));

        rig.core.dictation_state(DictationState::Idle);
        rig.core.dictation_state(DictationState::Recording);
        assert_eq!(cancel_bound(&rig).as_deref(), Some("Ctrl+Shift+Q"));
    }

    // Acceptance test 8 and record-shortcut.md rule 20: the two shortcuts must differ.
    #[test]
    fn the_cancel_and_record_shortcuts_cannot_be_the_same() {
        let mut rig = Rig::new(ShortcutMode::Toggle);

        assert_eq!(
            rig.core.set_cancel_combination("Ctrl+Space"),
            Err(ShortcutChangeError::NotAllowed {
                problem: ShortcutProblem::SameAsRecord
            })
        );
        assert_eq!(rig.core.config().cancel, default_cancel_shortcut());

        rig.core.set_cancel_combination("F8").unwrap();
        assert_eq!(
            rig.core.set_combination("F8"),
            Err(ShortcutChangeError::NotAllowed {
                problem: ShortcutProblem::SameAsCancel
            })
        );
        assert!(rig.core.apply_combination(&"F8".parse().unwrap()).is_err());
        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Space"));
        assert!(
            rig.core
                .apply_cancel_combination(&"Ctrl+Space".parse().unwrap())
                .is_err()
        );
        assert_eq!(rig.core.config().cancel.to_string(), "F8");
    }

    #[test]
    fn setting_the_cancel_shortcut_ends_the_capture() {
        let mut rig = Rig::new(ShortcutMode::Toggle);
        rig.core.begin_capture(Box::new(|_| {})).unwrap();

        rig.core.set_cancel_combination("Escape").unwrap();

        assert!(!rig.core.is_capturing());
        assert_eq!(rig.bound().as_deref(), Some("Ctrl+Space"));
    }
}
