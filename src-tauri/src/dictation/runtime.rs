//! Runs the [`Machine`] on a worker thread against the real pieces — or fakes in tests: the
//! active Model ([`DictationModels`]), an [`AudioSource`], a [`VoiceDetector`], History, the
//! [`SharedInserter`] and the Record Shortcut.
//!
//! Everything reaches the worker as a message: shortcut intents, Cancellation, audio from the
//! source, and the results of the Engine and Insertion, which run on threads of their own so the
//! worker never blocks. Audio stays in memory and is dropped when the Dictation ends (rule 13).

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::audio::{AudioEvent, AudioSource, AudioSourceError, AudioStream, SpeechConverter};
use crate::engine::{DictationLanguage, Engine, EngineError, TranscriptionRequest};
use crate::history::{EntryLanguage, NewEntry};
use crate::insertion::SharedInserter;
use crate::models::NoActiveModel;
use crate::shortcut::modes::RecordIntent;

use super::cleanup::clean_up;
use super::indicator::Indicator;
use super::machine::{Command, Input, Machine, Outcome};
use super::vad::{SpeechGate, VoiceDetector, pad_short};
use super::{DictationProblem, DictationStatus, ProblemKind};

/// A Recording stops by itself after this long (rule 12).
pub const MAX_RECORDING: Duration = Duration::from_secs(10 * 60);

/// Where Dictations get their Model: the Model manager in the app, a fixed Engine in tests.
pub trait DictationModels: Send {
    /// Begins a Dictation with the active Model, or refuses when none is active (rule 5). The
    /// Dictation ends for the Model side when the last clone of the returned value is dropped.
    fn begin(&self) -> Result<Arc<dyn DictationModel>, NoActiveModel>;
}

/// The Model of one Dictation.
pub trait DictationModel: Send + Sync {
    /// The Model's name, stored with the History entry.
    fn name(&self) -> String;
    /// Loads the Model if needed. Blocking; called when the Recording starts (rule 6).
    fn load(&self) -> Result<(), EngineError>;
    /// Runs the Engine. Blocking.
    fn transcribe(&self, request: TranscriptionRequest<'_>) -> Result<String, EngineError>;
    /// Frees the Model after the Engine crashed, so the next Dictation loads it again (rule 23).
    fn unload(&self);
}

/// A [`DictationModel`] over a plain [`Engine`], for tests and developer tools.
pub struct EngineModel<E> {
    name: String,
    engine: Mutex<E>,
}

impl<E: Engine> EngineModel<E> {
    pub fn new(name: impl Into<String>, engine: E) -> Self {
        Self {
            name: name.into(),
            engine: Mutex::new(engine),
        }
    }

    fn engine(&self) -> std::sync::MutexGuard<'_, E> {
        self.engine.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<E: Engine + 'static> DictationModel for EngineModel<E> {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn load(&self) -> Result<(), EngineError> {
        self.engine().load()
    }
    fn transcribe(&self, request: TranscriptionRequest<'_>) -> Result<String, EngineError> {
        self.engine().transcribe(request)
    }
    fn unload(&self) {
        self.engine().unload();
    }
}

/// The Dictation Language and Vocabulary for one Dictation (rule 22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationContext {
    pub language: DictationLanguage,
    pub vocabulary: Vec<String>,
}

impl Default for DictationContext {
    /// The defaults of `dictation-language.md` (Automatic) and `vocabulary.md` (empty).
    fn default() -> Self {
        Self {
            language: DictationLanguage::Automatic,
            vocabulary: Vec::new(),
        }
    }
}

/// The pieces a [`Dictation`] runs against.
pub struct DictationDeps {
    pub models: Box<dyn DictationModels>,
    pub source: Box<dyn AudioSource>,
    /// Makes a fresh detector for each Recording.
    pub detector: Box<dyn Fn() -> Box<dyn VoiceDetector> + Send>,
    /// The Dictation Language and Vocabulary, read when a Recording starts.
    pub context: Box<dyn Fn() -> DictationContext + Send>,
    /// Adds a Transcript to History.
    pub history: Box<dyn FnMut(NewEntry) -> Result<(), String> + Send>,
    pub inserter: SharedInserter,
    /// Tells the Record Shortcut a Recording ended without it
    /// (`RecordShortcutHandle::recording_ended`).
    pub shortcut_reset: Box<dyn FnMut() + Send>,
    /// Receives every changed [`DictationStatus`].
    pub publish: Box<dyn FnMut(&DictationStatus) + Send>,
    /// [`MAX_RECORDING`] in the app; shorter in tests.
    pub max_recording: Duration,
}

enum Msg {
    Intent(RecordIntent),
    Cancel,
    Audio(u64, AudioEvent),
    Transcribed(u64, Result<String, Failure>),
    Inserted(u64, Result<(), String>),
    Problem(ProblemKind, String),
    WindowSeen,
    DismissNotices,
}

/// Why a Dictation's transcription failed.
enum Failure {
    Load(String),
    Engine(String),
}

/// Handle to the running Dictation pipeline. Clones share it.
#[derive(Clone)]
pub struct Dictation {
    tx: Sender<Msg>,
    status: Arc<Mutex<DictationStatus>>,
}

impl Dictation {
    /// Starts the pipeline's worker thread. It runs until every handle is dropped.
    pub fn spawn(deps: DictationDeps) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(DictationStatus::default()));
        let worker = Worker {
            deps,
            tx: tx.clone(),
            rx,
            deferred: VecDeque::new(),
            machine: Machine::new(),
            indicator: Indicator::new(),
            status: Arc::clone(&status),
            published: DictationStatus::default(),
            seq: 0,
            recording: None,
            model: None,
            load: None,
            context: DictationContext::default(),
            failure: None,
            had_error: false,
        };
        std::thread::Builder::new()
            .name("echo-dictation".into())
            .spawn(move || worker.run())
            .expect("spawn the dictation worker");
        Self { tx, status }
    }

    /// A Record Shortcut intent (the Record Shortcut's `IntentSink`).
    pub fn intent(&self, intent: RecordIntent) {
        let _ = self.tx.send(Msg::Intent(intent));
    }

    /// Cancellation (rule 39): the entry point for the Cancel Shortcut (#15).
    pub fn cancel(&self) {
        let _ = self.tx.send(Msg::Cancel);
    }

    /// An error from outside the Dictation that is indicated the same way, e.g. a Model download
    /// failure (rule 39a).
    pub fn report_problem(&self, kind: ProblemKind, detail: impl Into<String>) {
        let _ = self.tx.send(Msg::Problem(kind, detail.into()));
    }

    /// The main window is in front of the user: the tray error clears (rule 39c).
    pub fn window_seen(&self) {
        let _ = self.tx.send(Msg::WindowSeen);
    }

    /// The user dismissed the main window's notices.
    pub fn dismiss_notices(&self) {
        let _ = self.tx.send(Msg::DismissNotices);
    }

    /// The latest published status.
    pub fn status(&self) -> DictationStatus {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

struct Recording {
    stream: AudioStream,
    converter: Option<SpeechConverter>,
    gate: SpeechGate,
    deadline: Instant,
}

struct Worker {
    deps: DictationDeps,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    /// Messages set aside while draining a stopped Recording's audio.
    deferred: VecDeque<Msg>,
    machine: Machine,
    indicator: Indicator,
    status: Arc<Mutex<DictationStatus>>,
    published: DictationStatus,
    /// Identifies the current Dictation; results of older ones are ignored.
    seq: u64,
    recording: Option<Recording>,
    model: Option<Arc<dyn DictationModel>>,
    load: Option<JoinHandle<Result<(), EngineError>>>,
    context: DictationContext,
    /// The error to report when the machine says the transcription failed.
    failure: Option<(ProblemKind, String)>,
    /// Whether the current Dictation has raised an error (then its Insertion does not clear the
    /// tray error).
    had_error: bool,
}

impl Worker {
    fn run(mut self) {
        loop {
            let msg = match self.deferred.pop_front() {
                Some(msg) => msg,
                None => {
                    let next = match &self.recording {
                        Some(r) => self
                            .rx
                            .recv_timeout(r.deadline.saturating_duration_since(Instant::now())),
                        None => self.rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    };
                    match next {
                        Ok(msg) => msg,
                        Err(RecvTimeoutError::Timeout) => {
                            self.feed(Input::RecordingEnded);
                            self.publish();
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            };
            self.on_message(msg);
            self.publish();
        }
    }

    fn on_message(&mut self, msg: Msg) {
        match msg {
            Msg::Intent(intent) => self.feed(Input::Intent(intent)),
            Msg::Cancel => self.feed(Input::Cancel),
            Msg::Audio(seq, event) if seq == self.seq => self.on_audio(event),
            Msg::Audio(..) => {}
            Msg::Transcribed(seq, result) if seq == self.seq => {
                let result = result.map_err(|failure| {
                    let (kind, detail) = match failure {
                        Failure::Load(d) => (ProblemKind::ModelLoadFailed, d),
                        Failure::Engine(d) => (ProblemKind::TranscriptionFailed, d),
                    };
                    self.failure = Some((kind, detail.clone()));
                    detail
                });
                self.feed(Input::Transcribed(result));
            }
            Msg::Inserted(seq, result) if seq == self.seq => {
                if let Err(detail) = result {
                    self.report(ProblemKind::InsertionFailed, detail);
                }
                self.feed(Input::Inserted);
            }
            Msg::Transcribed(..) | Msg::Inserted(..) => {}
            Msg::Problem(kind, detail) => {
                self.indicator.report(kind, detail);
            }
            Msg::WindowSeen => self.indicator.window_seen(),
            Msg::DismissNotices => self.indicator.dismiss_notices(),
        }
    }

    fn feed(&mut self, input: Input) {
        let commands = self.machine.handle(input);
        for command in commands {
            self.execute(command);
        }
    }

    fn execute(&mut self, command: Command) {
        match command {
            Command::StartRecording => self.start_recording(),
            Command::StopRecording => self.stop_recording(),
            Command::DiscardRecording => {
                if let Some(recording) = self.recording.take() {
                    recording.stream.stop();
                }
            }
            Command::StoreAndInsert(text) => self.store_and_insert(text),
            Command::Finish(outcome) => self.finish(outcome),
            Command::ResetShortcut => (self.deps.shortcut_reset)(),
        }
    }

    fn report(&mut self, kind: ProblemKind, detail: impl Into<String>) -> DictationProblem {
        self.had_error = true;
        let detail = detail.into();
        log::warn!("dictation problem {kind:?}: {detail}");
        self.indicator.report(kind, detail)
    }

    fn start_recording(&mut self) {
        self.seq += 1;
        self.had_error = false;
        self.failure = None;
        let model = match self.deps.models.begin() {
            Ok(model) => model,
            Err(NoActiveModel) => {
                self.report(ProblemKind::NoModel, "");
                return self.feed(Input::StartFailed);
            }
        };
        // Rule 6: load in the background while the user speaks.
        let loading = Arc::clone(&model);
        self.load = Some(
            std::thread::Builder::new()
                .name("echo-model-load".into())
                .spawn(move || loading.load())
                .expect("spawn the Model load thread"),
        );
        self.context = (self.deps.context)();
        let tx = self.tx.clone();
        let seq = self.seq;
        let sink = Box::new(move |event| {
            let _ = tx.send(Msg::Audio(seq, event));
        });
        match self.deps.source.start(sink) {
            Ok(stream) => {
                self.model = Some(model);
                self.recording = Some(Recording {
                    stream,
                    converter: None,
                    gate: SpeechGate::new((self.deps.detector)()),
                    deadline: Instant::now() + self.deps.max_recording,
                });
            }
            Err(error) => {
                self.load = None;
                let kind = match error {
                    AudioSourceError::NotFound => ProblemKind::MicrophoneNotFound,
                    AudioSourceError::AccessDenied => ProblemKind::MicrophoneAccessDenied,
                    AudioSourceError::Disconnected | AudioSourceError::Failed(_) => {
                        ProblemKind::MicrophoneFailed
                    }
                };
                self.report(kind, error.to_string());
                self.feed(Input::StartFailed);
            }
        }
    }

    fn on_audio(&mut self, event: AudioEvent) {
        match event {
            AudioEvent::Frames(frames) => {
                let Some(recording) = &mut self.recording else {
                    return;
                };
                let first = recording.converter.is_none();
                if first {
                    match SpeechConverter::new(frames.format) {
                        Ok(converter) => recording.converter = Some(converter),
                        Err(error) => {
                            self.report(ProblemKind::MicrophoneFailed, error.to_string());
                            return self.feed(Input::RecordingEnded);
                        }
                    }
                }
                if let Some(converter) = &mut recording.converter {
                    recording.gate.push(&converter.push(&frames.samples));
                }
                if first {
                    self.feed(Input::AudioArrived);
                }
            }
            AudioEvent::Ended => self.feed(Input::RecordingEnded),
            AudioEvent::Failed(error) => {
                let kind = match error {
                    AudioSourceError::Disconnected => ProblemKind::MicrophoneDisconnected,
                    _ => ProblemKind::MicrophoneFailed,
                };
                self.report(kind, error.to_string());
                self.feed(Input::RecordingEnded);
            }
        }
    }

    fn stop_recording(&mut self) {
        let Some(recording) = self.recording.take() else {
            return self.feed(Input::NoSpeech);
        };
        let Recording {
            stream,
            mut converter,
            mut gate,
            ..
        } = recording;
        stream.stop();
        // Audio the source delivered before it stopped may still be queued.
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Audio(seq, AudioEvent::Frames(frames)) if seq == self.seq => {
                    if converter.is_none() {
                        converter = SpeechConverter::new(frames.format).ok();
                    }
                    if let Some(converter) = &mut converter {
                        gate.push(&converter.push(&frames.samples));
                    }
                }
                Msg::Audio(..) => {}
                other => self.deferred.push_back(other),
            }
        }
        if let Some(converter) = converter {
            gate.push(&converter.finish());
        }
        let speech = gate.finish();
        if speech.is_empty() {
            return self.feed(Input::NoSpeech);
        }
        let audio = pad_short(speech);
        let (Some(model), load) = (self.model.clone(), self.load.take()) else {
            return self.feed(Input::NoSpeech);
        };
        let context = self.context.clone();
        let tx = self.tx.clone();
        let seq = self.seq;
        std::thread::Builder::new()
            .name("echo-transcribe".into())
            .spawn(move || {
                let result = transcribe(&*model, load, &audio, &context);
                let _ = tx.send(Msg::Transcribed(seq, result));
            })
            .expect("spawn the transcription thread");
    }

    fn store_and_insert(&mut self, text: String) {
        let model = self.model.as_ref().map(|m| m.name()).unwrap_or_default();
        let language = match &self.context.language {
            DictationLanguage::Automatic => EntryLanguage::Automatic { detected: None },
            DictationLanguage::Specific(code) => EntryLanguage::Specific { code: code.clone() },
        };
        // Rule 38: History first, so a failed Insertion still leaves the Transcript there.
        if let Err(error) = (self.deps.history)(NewEntry {
            text: text.clone(),
            model,
            language,
        }) {
            log::error!("could not add the Transcript to History: {error}");
        }
        let inserter = self.deps.inserter.clone();
        let tx = self.tx.clone();
        let seq = self.seq;
        std::thread::Builder::new()
            .name("echo-insert".into())
            .spawn(move || {
                let result = inserter.insert(&text).map_err(|e| e.to_string());
                let _ = tx.send(Msg::Inserted(seq, result));
            })
            .expect("spawn the Insertion thread");
    }

    fn finish(&mut self, outcome: Outcome) {
        // Releasing the Model ends the Dictation for the Model manager.
        self.model = None;
        self.load = None;
        match outcome {
            Outcome::Inserted if !self.had_error => self.indicator.dictation_succeeded(),
            Outcome::TranscriptionFailed(detail) => {
                let (kind, detail) = self
                    .failure
                    .take()
                    .unwrap_or((ProblemKind::TranscriptionFailed, detail));
                self.report(kind, detail);
            }
            Outcome::Inserted | Outcome::NoSpeech | Outcome::Cancelled => {}
        }
    }

    fn publish(&mut self) {
        let status = DictationStatus {
            state: self.machine.state(),
            listening: self.machine.listening(),
            error: self.indicator.error().cloned(),
            notices: self.indicator.notices().to_vec(),
        };
        if status != self.published {
            *self.status.lock().unwrap_or_else(PoisonError::into_inner) = status.clone();
            (self.deps.publish)(&status);
            self.published = status;
        }
    }
}

/// Waits for the background load, runs the Engine and cleans the Transcript. An Engine panic is
/// treated as a crash: the Model is unloaded so the next Dictation loads it afresh (rule 23).
fn transcribe(
    model: &dyn DictationModel,
    load: Option<JoinHandle<Result<(), EngineError>>>,
    audio: &[f32],
    context: &DictationContext,
) -> Result<String, Failure> {
    let loaded = match load.map(JoinHandle::join) {
        Some(Ok(result)) => result,
        Some(Err(_)) => Err(EngineError::ModelLoad("the Model load crashed".into())),
        None => Ok(()),
    };
    if let Err(error) = loaded {
        return Err(Failure::Load(engine_detail(error)));
    }
    let request = TranscriptionRequest {
        audio,
        language: &context.language,
        vocabulary: &context.vocabulary,
    };
    match catch_unwind(AssertUnwindSafe(|| model.transcribe(request))) {
        Ok(Ok(raw)) => Ok(clean_up(&raw)),
        Ok(Err(error)) => Err(Failure::Engine(engine_detail(error))),
        Err(_) => {
            model.unload();
            Err(Failure::Engine("the Engine crashed".into()))
        }
    }
}

/// The detail of an Engine error without its "transcription failed" prefix, which the UI adds.
fn engine_detail(error: EngineError) -> String {
    match error {
        EngineError::ModelLoad(detail) | EngineError::Transcription(detail) => detail,
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
