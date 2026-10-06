//! The Model manager: the download queue, the active Model and its Engine, idle unload and
//! deletion (`models.md` rules 7–8, 14–28).
//!
//! Everything that blocks (downloads, Model loads) runs off the caller's thread; the manager
//! publishes a fresh [`ModelsState`] to its listeners after every change, and reports download
//! and load failures as [`ModelProblem`]s for the error indication (`dictation-pipeline.md`
//! rules 39a–39d).
//!
//! The dictation pipeline holds a [`DictationGuard`] for the length of each Dictation: it hands
//! out the active Model's Engine, refuses switching and deleting while it lives (rules 20, 27),
//! and restarts the inactivity timer when dropped (rule 24a).

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::download::{self, CancelToken, DownloadConfig, DownloadError, DownloadTarget, Progress};
use super::{DISK_SPACE_MARGIN, DiskSpace, ModelId, ModelStorage};
use crate::engine::{Engine, EngineError, TranscribeCppEngine};
use crate::settings::{SettingsStore, UnloadModelAfter};

// --- seams -----------------------------------------------------------------------------------

/// The settings the manager reads and writes.
pub trait ModelSettings: Send + Sync {
    fn active_model(&self) -> Option<ModelId>;
    fn set_active_model(&self, model: Option<ModelId>);
    fn unload_model_after(&self) -> UnloadModelAfter;
}

impl ModelSettings for SettingsStore {
    fn active_model(&self) -> Option<ModelId> {
        self.get().active_model
    }

    fn set_active_model(&self, model: Option<ModelId>) {
        if let Err(error) = self.update(|s| s.active_model = model) {
            log::error!("could not save the active Model: {error}");
        }
    }

    fn unload_model_after(&self) -> UnloadModelAfter {
        self.get().unload_model_after
    }
}

/// Creates the Engine for a Model file.
pub trait EngineFactory: Send + Sync {
    fn create(&self, model: ModelId, path: &Path) -> Box<dyn Engine>;
}

impl<F> EngineFactory for F
where
    F: Fn(ModelId, &Path) -> Box<dyn Engine> + Send + Sync,
{
    fn create(&self, model: ModelId, path: &Path) -> Box<dyn Engine> {
        self(model, path)
    }
}

/// The real Engines (transcribe-cpp).
#[derive(Debug, Clone, Copy, Default)]
pub struct TranscribeCppEngines;

impl EngineFactory for TranscribeCppEngines {
    fn create(&self, _model: ModelId, path: &Path) -> Box<dyn Engine> {
        Box::new(TranscribeCppEngine::new(path))
    }
}

/// A monotonic clock for the inactivity timer.
pub trait Clock: Send + Sync {
    /// Time since an arbitrary fixed point.
    fn now(&self) -> Duration;
}

/// The real clock.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    start: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }
}

/// A clock that only moves when told to. Clones share the time.
#[derive(Debug, Clone, Default)]
pub struct FakeClock {
    now: Arc<Mutex<Duration>>,
}

impl FakeClock {
    pub fn advance(&self, by: Duration) {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The active Model's Engine, shared with the dictation pipeline.
pub type SharedEngine = Arc<Mutex<Box<dyn Engine>>>;

// --- state shown to the user -----------------------------------------------------------------

/// Everything the Models page, the Model indicator and the onboarding show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelsState {
    /// The three Models in list order.
    pub models: Vec<ModelEntry>,
    /// The active Model, if any (rule 17).
    pub active: Option<ModelId>,
    /// Whether the active Model is in memory.
    pub active_state: ActiveModelState,
    /// The Model being loaded to become active ("Loading <Model>…", rule 18).
    pub activating: Option<ModelId>,
    /// The latest failed load ("Couldn't load <Model>", rule 19), until the next successful one.
    pub load_failure: Option<LoadFailure>,
    /// A Dictation is Recording or Transcribing: switching and deleting are disabled
    /// (rules 20, 27).
    pub dictation_in_progress: bool,
}

/// One Model as shown on its card. Byte counts are `u32`: every 0.1.0 Model is under 4 GiB, and
/// TypeScript numbers carry them exactly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub id: ModelId,
    pub name: String,
    pub size_bytes: u32,
    pub languages: u32,
    pub recommended: bool,
    /// The verified file is present (rule 5).
    pub downloaded: bool,
    pub download: DownloadState,
}

/// Where a Model's download stands. `Idle` covers both "not downloaded" and "downloaded";
/// [`ModelEntry::downloaded`] tells them apart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum DownloadState {
    Idle,
    /// Waiting for another download to finish (rule 7).
    Queued,
    Downloading {
        downloaded: u32,
        total: u32,
        #[serde(rename = "bytesPerSecond")]
        bytes_per_second: u32,
    },
    /// "Verifying…" (rule 13).
    Verifying,
    /// "Paused — X% downloaded", with Resume and Delete (rule 14).
    Paused {
        downloaded: u32,
        total: u32,
    },
    /// A failed download with its reason and "Retry" (rule 16). `downloaded` is what the partial
    /// file kept for the retry.
    Failed {
        failure: DownloadFailure,
        downloaded: u32,
        total: u32,
    },
}

/// Why a download failed, for a plain-language message in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFailure {
    pub kind: FailureKind,
    /// Technical detail (in English) shown under the message.
    pub detail: String,
    /// For [`FailureKind::DiskSpace`]: the free space needed, in bytes.
    pub needed_bytes: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum FailureKind {
    Network,
    Stalled,
    BadRange,
    SizeMismatch,
    /// "Download was corrupted — please try again" (rule 12).
    Corrupted,
    Storage,
    DiskSpace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ActiveModelState {
    /// No Model is active.
    None,
    Loading,
    Ready,
    /// Unloaded after inactivity; loads on the next Dictation (rule 24b).
    Unloaded,
    /// The active Model could not be loaded (e.g. at start); the next Dictation tries again.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LoadFailure {
    pub model: ModelId,
    pub reason: String,
}

/// A download or load failure, for the error indication of `dictation-pipeline.md` rules
/// 39a–39d (Overlay message, red tray icon, window notice).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelProblem {
    pub kind: ModelProblemKind,
    pub model: ModelId,
    /// The Model's name, for the tray tooltip.
    pub model_name: String,
    /// Technical detail in English.
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ModelProblemKind {
    Download,
    Load,
}

/// Why a request was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("the Model is not downloaded")]
    NotDownloaded,
    #[error("a Dictation is in progress")]
    DictationInProgress,
    #[error("another Model is being loaded")]
    LoadInProgress,
    #[error("could not delete the Model files: {0}")]
    Storage(String),
}

/// No Model is active, so a Dictation cannot start (`dictation-pipeline.md` rule 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no Model is active")]
pub struct NoActiveModel;

// --- the manager ---------------------------------------------------------------------------

type StateListener = Box<dyn Fn(&ModelsState) + Send + Sync>;
type ProblemListener = Box<dyn Fn(&ModelProblem) + Send + Sync>;

/// Owns the Models at run time. Cheap to clone; clones share everything.
#[derive(Clone)]
pub struct ModelManager {
    shared: Arc<Shared>,
}

struct Shared {
    storage: ModelStorage,
    settings: Arc<dyn ModelSettings>,
    engines: Arc<dyn EngineFactory>,
    disk: Arc<dyn DiskSpace>,
    clock: Arc<dyn Clock>,
    config: DownloadConfig,
    /// Runs the downloads. An `Option` only so `Drop` can shut it down without blocking, which
    /// matters when the last clone of the manager is dropped inside a download task.
    runtime: Option<tokio::runtime::Runtime>,
    client: reqwest::Client,
    inner: Mutex<Inner>,
    /// Serialises publishing so the last state a listener sees is always the latest.
    publishing: Mutex<()>,
    listeners: RwLock<Vec<StateListener>>,
    problem_listeners: RwLock<Vec<ProblemListener>>,
}

struct Inner {
    downloaded: [bool; 3],
    downloads: [DownloadState; 3],
    queue: VecDeque<ModelId>,
    running: Option<Running>,
    next_generation: u64,
    /// The active Model's Engine. Present whenever a Model is active.
    engine: Option<(ModelId, SharedEngine)>,
    engine_state: ActiveModelState,
    activating: Option<ModelId>,
    load_failure: Option<LoadFailure>,
    dictation: bool,
    /// When the inactivity timer last restarted (rule 24a).
    last_activity: Duration,
    shut_down: bool,
}

struct Running {
    model: ModelId,
    cancel: CancelToken,
    generation: u64,
}

impl ModelManager {
    pub fn new(
        storage: ModelStorage,
        settings: Arc<dyn ModelSettings>,
        engines: Arc<dyn EngineFactory>,
        disk: Arc<dyn DiskSpace>,
        clock: Arc<dyn Clock>,
        config: DownloadConfig,
    ) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("echo-downloads")
            .enable_all()
            .build()
            .expect("the download runtime starts");
        let client = download::http_client(&config).expect("the HTTP client builds");
        let now = clock.now();
        Self {
            shared: Arc::new(Shared {
                storage,
                settings,
                engines,
                disk,
                clock,
                config,
                runtime: Some(runtime),
                client,
                inner: Mutex::new(Inner {
                    downloaded: [false; 3],
                    downloads: [
                        DownloadState::Idle,
                        DownloadState::Idle,
                        DownloadState::Idle,
                    ],
                    queue: VecDeque::new(),
                    running: None,
                    next_generation: 0,
                    engine: None,
                    engine_state: ActiveModelState::None,
                    activating: None,
                    load_failure: None,
                    dictation: false,
                    last_activity: now,
                    shut_down: false,
                }),
                publishing: Mutex::new(()),
                listeners: RwLock::new(Vec::new()),
                problem_listeners: RwLock::new(Vec::new()),
            }),
        }
    }

    /// Registers a listener for every state change.
    pub fn subscribe(&self, listener: impl Fn(&ModelsState) + Send + Sync + 'static) {
        write(&self.shared.listeners).push(Box::new(listener));
    }

    /// Registers a listener for download and load failures (the error indication).
    pub fn on_problem(&self, listener: impl Fn(&ModelProblem) + Send + Sync + 'static) {
        write(&self.shared.problem_listeners).push(Box::new(listener));
    }

    /// Looks at the models folder and loads the active Model (rules 5, 14, 23, 24). Call once,
    /// after subscribing.
    pub fn start(&self) {
        let storage = &self.shared.storage;
        let first_downloaded = {
            let mut inner = self.lock();
            for id in ModelId::ALL {
                let i = id.index();
                inner.downloaded[i] = storage.is_downloaded(id);
                inner.downloads[i] = match storage.partial_len(id) {
                    Some(len) if len > 0 && !inner.downloaded[i] => self.paused(id, len),
                    _ => DownloadState::Idle,
                };
            }
            ModelId::ALL
                .into_iter()
                .find(|id| inner.downloaded[id.index()])
        };
        match self.shared.settings.active_model() {
            // Load in the background so the first Dictation is fast (rule 18).
            Some(id) if storage.is_downloaded(id) => self.load_active_at_start(id),
            active => {
                // Rule 23: the active Model's file is gone; fall back to the first downloaded.
                if let Some(id) = active {
                    log::warn!("the active Model {id:?} is missing");
                    self.shared.settings.set_active_model(None);
                }
                if let Some(id) = first_downloaded {
                    let _ = self.activate(id);
                }
            }
        }
        self.publish();
    }

    /// The current state.
    pub fn state(&self) -> ModelsState {
        let inner = self.lock();
        self.snapshot(&inner)
    }

    /// Starts downloading a Model, or queues it behind the running download (rule 7).
    pub fn download(&self, id: ModelId) {
        {
            let mut inner = self.lock();
            let i = id.index();
            let busy = matches!(
                inner.downloads[i],
                DownloadState::Queued
                    | DownloadState::Downloading { .. }
                    | DownloadState::Verifying
            );
            if inner.downloaded[i] || busy || inner.shut_down {
                return;
            }
            inner.downloads[i] = DownloadState::Queued;
            inner.queue.push_back(id);
        }
        self.pump();
        self.publish();
    }

    /// Cancels a running or queued download; the partial file is kept (rule 14).
    pub fn cancel_download(&self, id: ModelId) {
        {
            let mut inner = self.lock();
            if let Some(running) = inner.running.as_ref().filter(|r| r.model == id) {
                running.cancel.cancel();
                return; // The download task reports Paused once it has stopped.
            }
            if let Some(position) = inner.queue.iter().position(|q| *q == id) {
                inner.queue.remove(position);
                inner.downloads[id.index()] = self.resting_state(id);
            }
        }
        self.publish();
    }

    /// Makes a downloaded Model active: loads it now and switches only when the load succeeds
    /// (rules 18–20).
    pub fn activate(&self, id: ModelId) -> Result<(), ModelError> {
        {
            let mut inner = self.lock();
            if !inner.downloaded[id.index()] {
                return Err(ModelError::NotDownloaded);
            }
            if inner.dictation {
                return Err(ModelError::DictationInProgress);
            }
            if inner.activating.is_some() {
                return Err(ModelError::LoadInProgress);
            }
            let current = self.shared.settings.active_model();
            if current == Some(id) && inner.engine_state == ActiveModelState::Ready {
                return Ok(());
            }
            inner.activating = Some(id);
            inner.load_failure = None;
        }
        self.publish();

        let manager = self.clone();
        let path = self.shared.storage.final_path(id);
        thread::Builder::new()
            .name("echo-model-load".into())
            .spawn(move || {
                let mut engine = manager.shared.engines.create(id, &path);
                let result = engine.load();
                manager.finish_activation(id, engine, result);
            })
            .expect("the load thread starts");
        Ok(())
    }

    /// Deletes a downloaded Model or a partial download (rules 25–28).
    pub fn delete(&self, id: ModelId) -> Result<(), ModelError> {
        let was_active = {
            let mut inner = self.lock();
            let active = self.shared.settings.active_model();
            if inner.dictation && active == Some(id) {
                return Err(ModelError::DictationInProgress);
            }
            if inner.activating == Some(id) {
                return Err(ModelError::LoadInProgress);
            }
            // Abandon a running download of this Model; its task's result is ignored.
            if let Some(running) = inner.running.take_if(|r| r.model == id) {
                running.cancel.cancel();
            }
            inner.queue.retain(|q| *q != id);
            // Unload first: the Engine may hold the file open.
            if inner.engine.as_ref().is_some_and(|(m, _)| *m == id) {
                if let Some((_, engine)) = inner.engine.take() {
                    lock_engine(&engine).unload();
                }
                inner.engine_state = ActiveModelState::None;
            }
            if inner.load_failure.as_ref().is_some_and(|f| f.model == id) {
                inner.load_failure = None;
            }
            active == Some(id)
        };
        let deleted = self.shared.storage.delete(id);
        {
            let mut inner = self.lock();
            inner.downloaded[id.index()] = self.shared.storage.is_downloaded(id);
            inner.downloads[id.index()] = self.resting_state(id);
        }
        if was_active {
            self.shared.settings.set_active_model(None);
            let next = {
                let inner = self.lock();
                ModelId::ALL
                    .into_iter()
                    .find(|m| inner.downloaded[m.index()])
            };
            if let Some(next) = next {
                let _ = self.activate(next);
            }
        }
        self.pump();
        self.publish();
        deleted.map_err(|e| ModelError::Storage(e.to_string()))
    }

    /// Starts a Dictation with the active Model, or refuses when there is none
    /// (`dictation-pipeline.md` rule 5). While the guard lives, switching and deleting the
    /// active Model are refused and the inactivity timer is stopped.
    pub fn begin_dictation(&self) -> Result<DictationGuard, NoActiveModel> {
        let guard = {
            let mut inner = self.lock();
            let active = self.shared.settings.active_model();
            let Some((model, engine)) = inner.engine.clone().filter(|(m, _)| Some(*m) == active)
            else {
                return Err(NoActiveModel);
            };
            inner.dictation = true;
            DictationGuard {
                manager: self.clone(),
                model,
                engine,
            }
        };
        self.publish();
        Ok(guard)
    }

    /// Unloads the active Model when it has been unused for the chosen time (rule 24a). The app
    /// calls this about once a second; tests call it after moving a [`FakeClock`].
    pub fn tick(&self) {
        let Some(limit) = self.shared.settings.unload_model_after().duration() else {
            return;
        };
        {
            let mut inner = self.lock();
            let idle = self.shared.clock.now().saturating_sub(inner.last_activity);
            if inner.dictation
                || inner.activating.is_some()
                || inner.engine_state != ActiveModelState::Ready
                || idle < limit
            {
                return;
            }
            let Some((model, engine)) = inner.engine.clone() else {
                return;
            };
            log::info!(
                "unloading {model:?} after {} s without a Dictation",
                idle.as_secs()
            );
            lock_engine(&engine).unload();
            inner.engine_state = ActiveModelState::Unloaded;
        }
        self.publish();
    }

    /// The "Unload Model after inactivity" setting changed: the timer restarts with the new value
    /// (rule 24c). Choosing Never cancels a pending unload simply because [`tick`](Self::tick)
    /// then never unloads; an unloaded Model stays unloaded.
    pub fn unload_setting_changed(&self) {
        self.lock().last_activity = self.shared.clock.now();
    }

    /// Stops the download (its partial file is kept) and unloads the Model; for quitting
    /// (rules 15, 24).
    pub fn shutdown(&self) {
        let mut inner = self.lock();
        inner.shut_down = true;
        inner.queue.clear();
        if let Some(running) = inner.running.take() {
            running.cancel.cancel();
        }
        if let Some((_, engine)) = inner.engine.take() {
            lock_engine(&engine).unload();
        }
    }

    // --- internals ---------------------------------------------------------------------------

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.shared
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn file_size(&self, id: ModelId) -> u64 {
        self.shared.storage.file(id).size
    }

    fn paused(&self, id: ModelId, downloaded: u64) -> DownloadState {
        DownloadState::Paused {
            downloaded: bytes(downloaded),
            total: bytes(self.file_size(id)),
        }
    }

    /// The state of a Model that is not downloading: Paused when a partial file exists.
    fn resting_state(&self, id: ModelId) -> DownloadState {
        match self.shared.storage.partial_len(id) {
            Some(len) if len > 0 && !self.shared.storage.is_downloaded(id) => self.paused(id, len),
            _ => DownloadState::Idle,
        }
    }

    fn snapshot(&self, inner: &Inner) -> ModelsState {
        let active = self.shared.settings.active_model();
        ModelsState {
            models: ModelId::ALL
                .into_iter()
                .map(|id| {
                    let info = id.info();
                    ModelEntry {
                        id,
                        name: info.name.into(),
                        size_bytes: bytes(self.file_size(id)),
                        languages: info.languages,
                        recommended: info.recommended,
                        downloaded: inner.downloaded[id.index()],
                        download: inner.downloads[id.index()].clone(),
                    }
                })
                .collect(),
            active,
            active_state: if active.is_some() {
                inner.engine_state
            } else {
                ActiveModelState::None
            },
            activating: inner.activating,
            load_failure: inner.load_failure.clone(),
            dictation_in_progress: inner.dictation,
        }
    }

    fn publish(&self) {
        let _order = self
            .shared
            .publishing
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let state = self.state();
        for listener in read(&self.shared.listeners).iter() {
            listener(&state);
        }
    }

    fn report(&self, problem: ModelProblem) {
        log::warn!(
            "{:?} failed for {}: {}",
            problem.kind,
            problem.model_name,
            problem.detail
        );
        for listener in read(&self.shared.problem_listeners).iter() {
            listener(&problem);
        }
    }

    /// Loads the active Model at start without changing which Model is active; a failure leaves
    /// it active in the error state, and the next Dictation tries again.
    fn load_active_at_start(&self, id: ModelId) {
        let path = self.shared.storage.final_path(id);
        let engine: SharedEngine = Arc::new(Mutex::new(self.shared.engines.create(id, &path)));
        {
            let mut inner = self.lock();
            inner.engine = Some((id, engine.clone()));
            inner.engine_state = ActiveModelState::Loading;
        }
        let manager = self.clone();
        thread::Builder::new()
            .name("echo-model-load".into())
            .spawn(move || {
                let result = lock_engine(&engine).load();
                let problem = {
                    let mut inner = manager.lock();
                    let still_current = inner
                        .engine
                        .as_ref()
                        .is_some_and(|(_, e)| Arc::ptr_eq(e, &engine));
                    if !still_current {
                        return;
                    }
                    inner.last_activity = manager.shared.clock.now();
                    match result {
                        Ok(()) => {
                            inner.engine_state = ActiveModelState::Ready;
                            None
                        }
                        Err(error) => {
                            inner.engine_state = ActiveModelState::Error;
                            let reason = engine_reason(&error);
                            inner.load_failure = Some(LoadFailure {
                                model: id,
                                reason: reason.clone(),
                            });
                            Some(load_problem(id, reason))
                        }
                    }
                };
                if let Some(problem) = problem {
                    manager.report(problem);
                }
                manager.publish();
            })
            .expect("the load thread starts");
    }

    fn finish_activation(
        &self,
        id: ModelId,
        mut engine: Box<dyn Engine>,
        result: Result<(), EngineError>,
    ) {
        let outcome = {
            let mut inner = self.lock();
            inner.activating = None;
            if inner.shut_down || !inner.downloaded[id.index()] {
                engine.unload();
                None
            } else {
                match result {
                    Ok(()) => {
                        if let Some((_, previous)) = inner.engine.take() {
                            lock_engine(&previous).unload();
                        }
                        inner.engine = Some((id, Arc::new(Mutex::new(engine))));
                        inner.engine_state = ActiveModelState::Ready;
                        inner.load_failure = None;
                        inner.last_activity = self.shared.clock.now();
                        Some(Ok(()))
                    }
                    Err(error) => {
                        let reason = engine_reason(&error);
                        inner.load_failure = Some(LoadFailure {
                            model: id,
                            reason: reason.clone(),
                        });
                        Some(Err(load_problem(id, reason)))
                    }
                }
            }
        };
        match outcome {
            Some(Ok(())) => self.shared.settings.set_active_model(Some(id)),
            Some(Err(problem)) => self.report(problem),
            None => {}
        }
        self.publish();
    }

    /// Starts the next queued download if none is running.
    fn pump(&self) {
        let mut problems = Vec::new();
        {
            let mut inner = self.lock();
            while inner.running.is_none() && !inner.shut_down {
                let Some(id) = inner.queue.pop_front() else {
                    break;
                };
                let i = id.index();
                let file = self.shared.storage.file(id).clone();
                let have = self
                    .shared
                    .storage
                    .partial_len(id)
                    .filter(|len| *len <= file.size)
                    .unwrap_or(0);
                // The partial file already holds `have` bytes; the rest plus the margin must fit.
                let needed = file.size - have + DISK_SPACE_MARGIN;
                let free = self.shared.disk.free_bytes(self.shared.storage.dir());
                match free {
                    Ok(free) if free < needed => {
                        inner.downloads[i] = DownloadState::Failed {
                            failure: DownloadFailure {
                                kind: FailureKind::DiskSpace,
                                detail: format!("{free} bytes free, {needed} needed"),
                                needed_bytes: Some(bytes(needed)),
                            },
                            downloaded: bytes(have),
                            total: bytes(file.size),
                        };
                        problems.push(download_problem(id, "not enough disk space".into()));
                        continue;
                    }
                    Ok(_) => {}
                    Err(error) => log::warn!("could not check free disk space: {error}"),
                }
                let generation = inner.next_generation;
                inner.next_generation += 1;
                let cancel = CancelToken::new();
                inner.running = Some(Running {
                    model: id,
                    cancel: cancel.clone(),
                    generation,
                });
                inner.downloads[i] = DownloadState::Downloading {
                    downloaded: bytes(have),
                    total: bytes(file.size),
                    bytes_per_second: 0,
                };
                let target = DownloadTarget {
                    url: file.url.clone(),
                    size: file.size,
                    sha256: file.sha256.clone(),
                    partial: self.shared.storage.partial_path(id),
                    final_path: self.shared.storage.final_path(id),
                };
                self.spawn_download(id, generation, target, cancel);
            }
        }
        for problem in problems {
            self.report(problem);
        }
    }

    fn spawn_download(
        &self,
        id: ModelId,
        generation: u64,
        target: DownloadTarget,
        cancel: CancelToken,
    ) {
        let manager = self.clone();
        let Some(runtime) = self.shared.runtime.as_ref() else {
            return;
        };
        runtime.spawn(async move {
            log::info!("downloading {}", id.info().name);
            let progress_manager = manager.clone();
            let mut on_progress = move |progress: Progress| {
                progress_manager.on_progress(id, generation, progress);
            };
            let result = download::download(
                &manager.shared.client,
                &target,
                &manager.shared.config,
                &cancel,
                &mut on_progress,
            )
            .await;
            manager.finish_download(id, generation, result);
        });
    }

    fn on_progress(&self, id: ModelId, generation: u64, progress: Progress) {
        {
            let mut inner = self.lock();
            if !inner
                .running
                .as_ref()
                .is_some_and(|r| r.generation == generation)
            {
                return;
            }
            inner.downloads[id.index()] = match progress {
                Progress::Transferring {
                    downloaded,
                    total,
                    bytes_per_second,
                } => DownloadState::Downloading {
                    downloaded: bytes(downloaded),
                    total: bytes(total),
                    bytes_per_second: bytes(bytes_per_second),
                },
                Progress::Verifying => DownloadState::Verifying,
            };
        }
        self.publish();
    }

    fn finish_download(&self, id: ModelId, generation: u64, result: Result<(), DownloadError>) {
        let (follow_up, problem) = {
            let mut inner = self.lock();
            if !inner
                .running
                .as_ref()
                .is_some_and(|r| r.generation == generation)
            {
                // Abandoned (deleted or quitting); its outcome no longer matters.
                return;
            }
            inner.running = None;
            let i = id.index();
            match result {
                Ok(()) => {
                    log::info!("downloaded and verified {}", id.info().name);
                    inner.downloaded[i] = true;
                    inner.downloads[i] = DownloadState::Idle;
                    // Rules 21–22: the first Model becomes active; later ones only when none is.
                    let none_active =
                        self.shared.settings.active_model().is_none() && inner.activating.is_none();
                    (none_active.then_some(id), None)
                }
                Err(DownloadError::Cancelled) => {
                    inner.downloads[i] = self.resting_state(id);
                    (None, None)
                }
                Err(error) => {
                    let kept = self.shared.storage.partial_len(id).unwrap_or(0);
                    let failure = failure_of(&error);
                    inner.downloads[i] = DownloadState::Failed {
                        failure,
                        downloaded: bytes(kept),
                        total: bytes(self.file_size(id)),
                    };
                    (None, Some(download_problem(id, error.to_string())))
                }
            }
        };
        if let Some(problem) = problem {
            self.report(problem);
        }
        if let Some(id) = follow_up {
            let _ = self.activate(id);
        }
        self.pump();
        self.publish();
    }

    fn end_dictation(&self) {
        {
            let mut inner = self.lock();
            inner.dictation = false;
            inner.last_activity = self.shared.clock.now();
        }
        self.publish();
    }

    /// Loads the guard's Engine, keeping the Model indicator up to date (rule 24b).
    fn load_for_dictation(&self, engine: &SharedEngine) -> Result<(), EngineError> {
        let is_current = |inner: &Inner| {
            inner
                .engine
                .as_ref()
                .is_some_and(|(_, e)| Arc::ptr_eq(e, engine))
        };
        let changed = {
            let mut inner = self.lock();
            let reload = is_current(&inner) && inner.engine_state != ActiveModelState::Ready;
            if reload {
                inner.engine_state = ActiveModelState::Loading;
            }
            reload
        };
        if changed {
            self.publish();
        }
        let result = lock_engine(engine).load();
        {
            let mut inner = self.lock();
            if is_current(&inner) {
                inner.engine_state = match result {
                    Ok(()) => ActiveModelState::Ready,
                    Err(_) => ActiveModelState::Error,
                };
            }
        }
        self.publish();
        result
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// Held by the dictation pipeline for one Dictation (from the Record Shortcut press until the
/// Dictation ends). Dropping it ends the Dictation for the Model manager: switching and
/// deleting are allowed again and the inactivity timer restarts (rule 24a).
pub struct DictationGuard {
    manager: ModelManager,
    model: ModelId,
    engine: SharedEngine,
}

impl DictationGuard {
    /// The Model this Dictation uses.
    pub fn model(&self) -> ModelId {
        self.model
    }

    /// The active Model's Engine. Lock it on a worker thread to transcribe.
    pub fn engine(&self) -> &SharedEngine {
        &self.engine
    }

    /// Loads the Model if it is not in memory (after an idle unload or a failed load), updating
    /// the Model indicator. Blocking: call it on a worker thread when the Recording starts
    /// (`dictation-pipeline.md` rule 6).
    pub fn load(&self) -> Result<(), EngineError> {
        self.manager.load_for_dictation(&self.engine)
    }
}

impl Drop for DictationGuard {
    fn drop(&mut self) {
        self.manager.end_dictation();
    }
}

impl std::fmt::Debug for DictationGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DictationGuard")
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

fn lock_engine(engine: &SharedEngine) -> MutexGuard<'_, Box<dyn Engine>> {
    engine.lock().unwrap_or_else(PoisonError::into_inner)
}

fn read<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}

fn write<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}

/// A byte count for the UI (see [`ModelEntry`]).
fn bytes(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn engine_reason(error: &EngineError) -> String {
    match error {
        EngineError::ModelLoad(detail) | EngineError::Transcription(detail) => detail.clone(),
    }
}

fn load_problem(id: ModelId, detail: String) -> ModelProblem {
    ModelProblem {
        kind: ModelProblemKind::Load,
        model: id,
        model_name: id.info().name.into(),
        detail,
    }
}

fn download_problem(id: ModelId, detail: String) -> ModelProblem {
    ModelProblem {
        kind: ModelProblemKind::Download,
        model: id,
        model_name: id.info().name.into(),
        detail,
    }
}

fn failure_of(error: &DownloadError) -> DownloadFailure {
    let kind = match error {
        DownloadError::Network(_) => FailureKind::Network,
        DownloadError::Stalled => FailureKind::Stalled,
        DownloadError::BadRange(_) => FailureKind::BadRange,
        DownloadError::SizeMismatch { .. } => FailureKind::SizeMismatch,
        DownloadError::Corrupted => FailureKind::Corrupted,
        DownloadError::Storage(_) | DownloadError::Cancelled => FailureKind::Storage,
    };
    DownloadFailure {
        kind,
        detail: error.to_string(),
        needed_bytes: None,
    }
}

#[cfg(test)]
mod tests;
