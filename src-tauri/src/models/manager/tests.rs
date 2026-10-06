//! `models.md` acceptance tests 1, 10, 11, 13–18, 20 and 22 at the level of the Model manager,
//! with the local test server, a fake disk, fake Engines and a fake clock.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::engine::FakeEngine;
use crate::models::ModelFile;
use crate::models::test_server::{Behaviour, TestServer, sha256_hex, test_bytes};
use crate::settings::Settings;

const SIZE: usize = 120_000;
const WAIT: Duration = Duration::from_secs(10);

use ModelId::{ParakeetTdt06bV3 as Parakeet, WhisperLargeV3Turbo as Turbo, WhisperSmall as Small};

struct FakeDisk(AtomicU64);

impl DiskSpace for FakeDisk {
    fn free_bytes(&self, _path: &Path) -> io::Result<u64> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}

/// Hands out one shared [`FakeEngine`] per Model and counts the Engines created.
#[derive(Clone, Default)]
struct Engines {
    probes: Arc<Mutex<HashMap<ModelId, FakeEngine>>>,
    created: Arc<Mutex<Vec<ModelId>>>,
}

impl Engines {
    fn probe(&self, id: ModelId) -> FakeEngine {
        self.probes
            .lock()
            .unwrap()
            .entry(id)
            .or_insert_with(|| FakeEngine::returning("text"))
            .clone()
    }

    fn created(&self) -> Vec<ModelId> {
        self.created.lock().unwrap().clone()
    }
}

impl EngineFactory for Engines {
    fn create(&self, model: ModelId, _path: &Path) -> Box<dyn Engine> {
        self.created.lock().unwrap().push(model);
        Box::new(self.probe(model))
    }
}

struct Harness {
    dir: tempfile::TempDir,
    server: TestServer,
    body: Vec<u8>,
    settings: Arc<SettingsStore>,
    engines: Engines,
    clock: FakeClock,
    disk: Arc<FakeDisk>,
    manager: ModelManager,
    problems: Arc<Mutex<Vec<ModelProblem>>>,
}

impl Harness {
    fn new() -> Self {
        Self::with(|_| {}, |_| {})
    }

    /// `behaviour` shapes the test server; `before_start` prepares files and settings before
    /// the manager looks at them.
    fn with(behaviour: impl FnOnce(&mut Behaviour), before_start: impl FnOnce(&Harness)) -> Self {
        let body = test_bytes(SIZE);
        let mut b = Behaviour::serving(body.clone());
        behaviour(&mut b);
        let server = TestServer::start(b);
        let dir = tempfile::tempdir().unwrap();
        let (settings, _) =
            SettingsStore::open(dir.path().join("settings.json"), Settings::defaults(None));
        let settings = Arc::new(settings);
        let url = server.url();
        let sha = sha256_hex(&body);
        let storage = ModelStorage::with_files(dir.path().join("models"), |id| ModelFile {
            file_name: id.info().file_name.into(),
            size: SIZE as u64,
            sha256: sha.clone(),
            url: url.clone(),
        });
        fs::create_dir_all(storage.dir()).unwrap();
        let engines = Engines::default();
        let clock = FakeClock::default();
        let disk = Arc::new(FakeDisk(AtomicU64::new(u64::MAX)));
        let manager = ModelManager::new(
            storage,
            settings.clone(),
            Arc::new(engines.clone()),
            disk.clone(),
            Arc::new(clock.clone()),
            DownloadConfig {
                connect_timeout: Duration::from_secs(5),
                stall_timeout: Duration::from_secs(5),
                ..DownloadConfig::default()
            },
        );
        let problems: Arc<Mutex<Vec<ModelProblem>>> = Arc::default();
        let sink = problems.clone();
        manager.on_problem(move |p| sink.lock().unwrap().push(p.clone()));
        let harness = Self {
            dir,
            server,
            body,
            settings,
            engines,
            clock,
            disk,
            manager,
            problems,
        };
        before_start(&harness);
        harness.manager.start();
        harness
    }

    fn models_dir(&self) -> PathBuf {
        self.dir.path().join("models")
    }

    fn final_path(&self, id: ModelId) -> PathBuf {
        self.models_dir().join(id.info().file_name)
    }

    fn partial_path(&self, id: ModelId) -> PathBuf {
        self.models_dir().join(format!(
            "{}{}",
            id.info().file_name,
            crate::models::PARTIAL_SUFFIX
        ))
    }

    /// Puts a verified Model file in place, as if downloaded earlier.
    fn place(&self, id: ModelId) {
        fs::write(self.final_path(id), &self.body).unwrap();
    }

    fn state(&self) -> ModelsState {
        self.manager.state()
    }

    fn entry(&self, id: ModelId) -> ModelEntry {
        self.state().models[id.index()].clone()
    }

    fn wait(&self, what: &str, condition: impl Fn(&ModelsState) -> bool) -> ModelsState {
        let deadline = Instant::now() + WAIT;
        loop {
            let state = self.state();
            if condition(&state) {
                return state;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}: {state:#?}"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn wait_active(&self, id: ModelId) {
        self.wait("the Model to be active and ready", |s| {
            s.active == Some(id)
                && s.active_state == ActiveModelState::Ready
                && s.activating.is_none()
        });
    }

    fn problems(&self) -> Vec<ModelProblem> {
        self.problems.lock().unwrap().clone()
    }
}

fn started_with(models: &[ModelId], active: Option<ModelId>) -> Harness {
    Harness::with(
        |_| {},
        |h| {
            for id in models {
                h.place(*id);
            }
            h.settings.set_active_model(active);
        },
    )
}

// --- the list and fresh state ----------------------------------------------------------------

#[test]
fn the_three_models_are_listed_in_order_with_nothing_active() {
    let h = Harness::new();
    let state = h.state();

    let ids: Vec<_> = state.models.iter().map(|m| m.id).collect();
    assert_eq!(ids, ModelId::ALL);
    assert!(
        state
            .models
            .iter()
            .all(|m| !m.downloaded && m.download == DownloadState::Idle)
    );
    assert_eq!(state.active, None);
    assert_eq!(state.active_state, ActiveModelState::None);
    assert!(state.models[0].recommended);
}

// --- downloads -------------------------------------------------------------------------------

// Acceptance test 1 and rule 21: the first downloaded Model becomes active.
#[test]
fn a_finished_first_download_is_verified_and_becomes_active() {
    let h = Harness::new();

    h.manager.download(Small);

    h.wait_active(Small);
    assert!(h.entry(Small).downloaded);
    assert!(h.final_path(Small).exists());
    assert!(!h.partial_path(Small).exists());
    assert_eq!(h.engines.probe(Small).load_count(), 1);
    assert_eq!(h.settings.get().active_model, Some(Small));
}

// Rule 22.
#[test]
fn a_later_download_does_not_change_the_active_model() {
    let h = started_with(&[Small], Some(Small));
    h.wait_active(Small);

    h.manager.download(Turbo);

    h.wait("Turbo downloaded", |s| s.models[Turbo.index()].downloaded);
    thread::sleep(Duration::from_millis(50));
    assert_eq!(h.state().active, Some(Small));
    assert!(!h.engines.created().contains(&Turbo));
}

// Rule 7.
#[test]
fn one_download_runs_at_a_time_and_the_next_waits_in_the_queue() {
    let h = Harness::with(
        |b| {
            b.chunk = 4000;
            b.chunk_delay = Duration::from_millis(10);
        },
        |_| {},
    );

    h.manager.download(Turbo);
    h.manager.download(Parakeet);

    let state = h.wait("Turbo downloading", |s| {
        matches!(
            s.models[Turbo.index()].download,
            DownloadState::Downloading { .. }
        )
    });
    assert_eq!(
        state.models[Parakeet.index()].download,
        DownloadState::Queued
    );
    h.wait("both downloaded", |s| {
        s.models[Turbo.index()].downloaded && s.models[Parakeet.index()].downloaded
    });
    assert_eq!(h.server.received().len(), 2);
    h.wait_active(Turbo);
}

// Acceptance test 10 (manager part).
#[test]
fn cancel_pauses_with_the_partial_kept_and_resume_continues() {
    let h = Harness::with(
        |b| {
            b.chunk = 4000;
            b.chunk_delay = Duration::from_millis(20);
        },
        |_| {},
    );
    h.manager.download(Small);
    h.wait("30% downloaded", |s| {
        matches!(s.models[Small.index()].download,
            DownloadState::Downloading { downloaded, .. } if downloaded as usize >= SIZE * 3 / 10)
    });

    h.manager.cancel_download(Small);

    let state = h.wait("paused", |s| {
        matches!(
            s.models[Small.index()].download,
            DownloadState::Paused { .. }
        )
    });
    let DownloadState::Paused { downloaded, total } = state.models[Small.index()].download else {
        unreachable!()
    };
    assert_eq!(total as usize, SIZE);
    assert_eq!(
        u64::from(downloaded),
        fs::metadata(h.partial_path(Small)).unwrap().len()
    );
    assert!(h.problems().is_empty(), "a cancel is not an error");

    h.server.behaviour().chunk_delay = Duration::ZERO;
    h.manager.download(Small);
    h.wait_active(Small);
    assert_eq!(
        h.server.received()[1].range.as_deref(),
        Some(format!("bytes={downloaded}-").as_str())
    );
}

#[test]
fn cancelling_a_queued_download_takes_it_off_the_queue() {
    let h = Harness::with(
        |b| {
            b.chunk = 2000;
            b.chunk_delay = Duration::from_millis(20);
        },
        |_| {},
    );
    h.manager.download(Turbo);
    h.manager.download(Small);

    h.manager.cancel_download(Small);

    assert_eq!(h.entry(Small).download, DownloadState::Idle);
    h.wait("Turbo transferring", |s| {
        matches!(s.models[Turbo.index()].download, DownloadState::Downloading { downloaded, .. } if downloaded > 0)
    });
    h.manager.cancel_download(Turbo);
    h.wait("Turbo paused", |s| {
        matches!(
            s.models[Turbo.index()].download,
            DownloadState::Paused { .. }
        )
    });
    assert_eq!(h.server.received().len(), 1, "Small never started");
}

// Acceptance test 11.
#[test]
fn too_little_disk_space_refuses_the_download_with_the_needed_amount() {
    let h = Harness::new();
    h.disk.0.store(1000, Ordering::SeqCst);

    h.manager.download(Small);

    let DownloadState::Failed { failure, .. } = h.entry(Small).download else {
        panic!("{:?}", h.entry(Small).download)
    };
    assert_eq!(failure.kind, FailureKind::DiskSpace);
    assert_eq!(
        failure.needed_bytes,
        Some((SIZE as u64 + crate::models::DISK_SPACE_MARGIN) as u32)
    );
    assert!(h.server.received().is_empty(), "nothing was requested");
    assert_eq!(h.problems()[0].kind, ModelProblemKind::Download);
}

#[test]
fn a_partial_download_needs_space_only_for_the_rest() {
    let h = Harness::with(
        |_| {},
        |h| fs::write(h.partial_path(Small), &h.body[..SIZE / 2]).unwrap(),
    );
    h.disk.0.store(
        (SIZE / 2) as u64 + crate::models::DISK_SPACE_MARGIN,
        Ordering::SeqCst,
    );

    h.manager.download(Small);

    h.wait_active(Small);
}

// Acceptance test 22 (download half) and rule 12.
#[test]
fn a_corrupted_download_fails_with_a_reason_and_is_reported() {
    let h = Harness::with(|b| b.body[10] ^= 1, |_| {});

    h.manager.download(Small);

    let state = h.wait("failed", |s| {
        matches!(
            s.models[Small.index()].download,
            DownloadState::Failed { .. }
        )
    });
    let DownloadState::Failed {
        failure,
        downloaded,
        ..
    } = &state.models[Small.index()].download
    else {
        unreachable!()
    };
    assert_eq!(failure.kind, FailureKind::Corrupted);
    assert_eq!(*downloaded, 0, "the corrupted file was deleted");
    assert_eq!(state.active, None);
    let problems = h.problems();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].kind, ModelProblemKind::Download);
    assert_eq!(problems[0].model_name, "Whisper small");
}

#[test]
fn a_network_failure_keeps_the_partial_and_retry_resumes() {
    let h = Harness::with(|b| b.close_after = Some(SIZE / 3), |_| {});

    h.manager.download(Small);
    let state = h.wait("failed", |s| {
        matches!(
            s.models[Small.index()].download,
            DownloadState::Failed { .. }
        )
    });
    let DownloadState::Failed {
        failure,
        downloaded,
        ..
    } = &state.models[Small.index()].download
    else {
        unreachable!()
    };
    assert_eq!(failure.kind, FailureKind::Network);
    assert_eq!(*downloaded as usize, SIZE / 3);

    h.server.behaviour().close_after = None;
    h.manager.download(Small);
    h.wait_active(Small);
}

#[test]
fn a_partial_file_at_start_shows_as_paused() {
    let h = Harness::with(
        |_| {},
        |h| fs::write(h.partial_path(Parakeet), &h.body[..SIZE / 4]).unwrap(),
    );

    assert_eq!(
        h.entry(Parakeet).download,
        DownloadState::Paused {
            downloaded: (SIZE / 4) as u32,
            total: SIZE as u32
        }
    );
    assert!(!h.entry(Parakeet).downloaded);
}

// --- activation ------------------------------------------------------------------------------

// Acceptance test 13 and acceptance test 22 (load half).
#[test]
fn activating_loads_once_and_a_failed_load_keeps_the_previous_model() {
    let h = started_with(&[Small, Turbo], Some(Small));
    h.wait_active(Small);

    h.manager.activate(Turbo).unwrap();
    h.wait_active(Turbo);
    assert_eq!(h.engines.probe(Turbo).load_count(), 1);
    assert!(
        !h.engines.probe(Small).is_loaded(),
        "the previous Model was unloaded"
    );

    h.engines
        .probe(Small)
        .set_load_outcome(Err(EngineError::ModelLoad("bad file".into())));
    h.manager.activate(Small).unwrap();
    let state = h.wait("the load to fail", |s| s.load_failure.is_some());

    assert_eq!(state.active, Some(Turbo), "the previous Model stays active");
    assert_eq!(state.active_state, ActiveModelState::Ready);
    assert_eq!(
        state.load_failure,
        Some(LoadFailure {
            model: Small,
            reason: "bad file".into()
        })
    );
    assert_eq!(h.settings.get().active_model, Some(Turbo));
    let problems = h.problems();
    assert_eq!(problems.last().unwrap().kind, ModelProblemKind::Load);
    assert_eq!(problems.last().unwrap().model, Small);
}

#[test]
fn while_loading_the_state_says_which_model_is_loading() {
    let h = started_with(&[Small, Turbo], Some(Small));
    h.wait_active(Small);
    let slow = FakeEngine::returning("x").with_load_delay(Duration::from_millis(300));
    h.engines.probes.lock().unwrap().insert(Turbo, slow);

    h.manager.activate(Turbo).unwrap();

    let state = h.state();
    assert_eq!(state.activating, Some(Turbo));
    assert_eq!(state.active, Some(Small), "switches only once loaded");
    assert_eq!(h.manager.activate(Small), Err(ModelError::LoadInProgress));
    h.wait_active(Turbo);
}

#[test]
fn a_model_that_is_not_downloaded_cannot_be_activated() {
    let h = Harness::new();
    assert_eq!(h.manager.activate(Small), Err(ModelError::NotDownloaded));
}

// Acceptance test 14.
#[test]
fn during_a_dictation_switching_and_deleting_are_refused() {
    let h = started_with(&[Small, Turbo], Some(Small));
    h.wait_active(Small);

    let guard = h.manager.begin_dictation().unwrap();
    assert_eq!(guard.model(), Small);
    assert!(h.state().dictation_in_progress);

    assert_eq!(
        h.manager.activate(Turbo),
        Err(ModelError::DictationInProgress)
    );
    assert_eq!(
        h.manager.delete(Small),
        Err(ModelError::DictationInProgress)
    );
    assert!(h.final_path(Small).exists());

    drop(guard);
    assert!(!h.state().dictation_in_progress);
    h.manager.activate(Turbo).unwrap();
    h.wait_active(Turbo);
}

#[test]
fn a_dictation_without_an_active_model_is_refused() {
    let h = Harness::new();
    assert_eq!(h.manager.begin_dictation().unwrap_err(), NoActiveModel);
}

// --- delete ----------------------------------------------------------------------------------

// Acceptance test 15.
#[test]
fn deleting_the_active_model_falls_back_to_the_next_downloaded_one() {
    let h = started_with(&[Small, Parakeet], Some(Small));
    h.wait_active(Small);

    h.manager.delete(Small).unwrap();

    assert!(!h.final_path(Small).exists());
    assert!(!h.entry(Small).downloaded);
    assert!(
        !h.engines.probe(Small).is_loaded(),
        "the deleted Model was unloaded"
    );
    h.wait_active(Parakeet);

    h.manager.delete(Parakeet).unwrap();
    let state = h.wait("none active", |s| s.active.is_none());
    assert_eq!(state.active_state, ActiveModelState::None);
    assert_eq!(h.settings.get().active_model, None);
    assert!(h.manager.begin_dictation().is_err());
}

#[test]
fn deleting_a_model_that_is_not_active_keeps_the_active_one() {
    let h = started_with(&[Small, Turbo], Some(Small));
    h.wait_active(Small);

    h.manager.delete(Turbo).unwrap();

    assert_eq!(h.state().active, Some(Small));
    assert!(!h.entry(Turbo).downloaded);
}

// Rules 25 and 28.
#[test]
fn deleting_a_paused_download_or_missing_files_succeeds() {
    let h = Harness::with(
        |_| {},
        |h| fs::write(h.partial_path(Small), &h.body[..100]).unwrap(),
    );

    h.manager.delete(Small).unwrap();
    assert!(!h.partial_path(Small).exists());
    assert_eq!(h.entry(Small).download, DownloadState::Idle);

    h.manager.delete(Small).unwrap();
}

#[test]
fn deleting_a_running_download_stops_it() {
    let h = Harness::with(
        |b| {
            b.chunk = 2000;
            b.chunk_delay = Duration::from_millis(20);
        },
        |_| {},
    );
    h.manager.download(Small);
    h.wait("downloading", |s| {
        matches!(
            s.models[Small.index()].download,
            DownloadState::Downloading { .. }
        )
    });

    h.manager.delete(Small).unwrap();

    thread::sleep(Duration::from_millis(200));
    assert_eq!(h.entry(Small).download, DownloadState::Idle);
    assert!(!h.entry(Small).downloaded);
    assert!(!h.final_path(Small).exists());
}

// --- start -----------------------------------------------------------------------------------

// Acceptance test 16.
#[test]
fn a_missing_active_model_at_start_falls_back_to_the_first_downloaded() {
    let h = started_with(&[Parakeet, Small], Some(Turbo));

    h.wait_active(Parakeet);
    assert_eq!(h.settings.get().active_model, Some(Parakeet));
}

#[test]
fn a_missing_active_model_with_nothing_downloaded_leaves_none_active() {
    let h = started_with(&[], Some(Turbo));

    assert_eq!(h.state().active, None);
    assert_eq!(h.settings.get().active_model, None);
}

#[test]
fn the_active_model_is_loaded_at_start() {
    let h = started_with(&[Small], Some(Small));

    h.wait_active(Small);
    assert_eq!(h.engines.probe(Small).load_count(), 1);
}

#[test]
fn a_failed_load_at_start_keeps_the_model_active_in_the_error_state() {
    let h = Harness::with(
        |_| {},
        |h| {
            h.place(Small);
            h.settings.set_active_model(Some(Small));
            h.engines
                .probe(Small)
                .set_load_outcome(Err(EngineError::ModelLoad("no memory".into())));
        },
    );

    let state = h.wait("error", |s| s.active_state == ActiveModelState::Error);
    assert_eq!(state.active, Some(Small));
    assert_eq!(h.problems()[0].kind, ModelProblemKind::Load);

    // The next Dictation tries again.
    h.engines.probe(Small).set_load_outcome(Ok(()));
    let guard = h.manager.begin_dictation().unwrap();
    guard.load().unwrap();
    assert_eq!(h.state().active_state, ActiveModelState::Ready);
}

// --- idle unload -----------------------------------------------------------------------------

fn idle_harness(setting: UnloadModelAfter) -> Harness {
    let h = started_with(&[Small], Some(Small));
    h.settings
        .update(|s| s.unload_model_after = setting)
        .unwrap();
    h.wait_active(Small);
    h
}

fn dictate(h: &Harness, length: Duration) {
    let guard = h.manager.begin_dictation().unwrap();
    guard.load().unwrap();
    h.clock.advance(length);
    h.manager.tick();
    drop(guard);
}

// Acceptance test 17.
#[test]
fn the_model_is_unloaded_after_the_chosen_inactivity_and_stays_active() {
    let h = idle_harness(UnloadModelAfter::Minutes5);
    dictate(&h, Duration::from_secs(5));

    h.clock.advance(Duration::from_secs(4 * 60 + 59));
    h.manager.tick();
    assert!(h.engines.probe(Small).is_loaded());
    assert_eq!(h.state().active_state, ActiveModelState::Ready);

    h.clock.advance(Duration::from_secs(1));
    h.manager.tick();
    assert!(!h.engines.probe(Small).is_loaded());
    let state = h.state();
    assert_eq!(state.active, Some(Small), "still the active Model");
    assert_eq!(state.active_state, ActiveModelState::Unloaded);
}

#[test]
fn without_a_dictation_the_timer_counts_from_the_load() {
    let h = idle_harness(UnloadModelAfter::Minutes2);

    h.clock.advance(Duration::from_secs(120));
    h.manager.tick();

    assert_eq!(h.state().active_state, ActiveModelState::Unloaded);
}

// Acceptance test 18.
#[test]
fn no_unload_during_a_dictation_and_the_timer_starts_when_it_ends() {
    let h = idle_harness(UnloadModelAfter::Minutes2);

    let guard = h.manager.begin_dictation().unwrap();
    for _ in 0..180 {
        h.clock.advance(Duration::from_secs(1));
        h.manager.tick();
    }
    assert!(
        h.engines.probe(Small).is_loaded(),
        "not unloaded while recording"
    );
    drop(guard);

    h.clock.advance(Duration::from_secs(119));
    h.manager.tick();
    assert!(h.engines.probe(Small).is_loaded());
    h.clock.advance(Duration::from_secs(1));
    h.manager.tick();
    assert!(!h.engines.probe(Small).is_loaded());
}

// Acceptance test 19 (Model manager part): the next Dictation loads the Model again.
#[test]
fn after_an_idle_unload_the_next_dictation_loads_the_model_again() {
    let h = idle_harness(UnloadModelAfter::Minutes2);
    h.clock.advance(Duration::from_secs(120));
    h.manager.tick();
    assert_eq!(h.state().active_state, ActiveModelState::Unloaded);

    let guard = h
        .manager
        .begin_dictation()
        .expect("an unloaded Model is still active");
    guard.load().unwrap();

    assert!(h.engines.probe(Small).is_loaded());
    assert_eq!(h.state().active_state, ActiveModelState::Ready);
    let text = guard
        .engine()
        .lock()
        .unwrap()
        .transcribe(crate::engine::TranscriptionRequest {
            audio: &[0.0; 16_000],
            language: &crate::engine::DictationLanguage::Automatic,
            vocabulary: &[],
        })
        .unwrap();
    assert_eq!(text, "text");
}

// Acceptance test 20.
#[test]
fn never_unloads_even_after_a_day_and_switching_to_never_cancels_a_pending_unload() {
    let h = idle_harness(UnloadModelAfter::Never);
    h.clock.advance(Duration::from_secs(24 * 3600));
    h.manager.tick();
    assert!(h.engines.probe(Small).is_loaded());

    h.settings
        .update(|s| s.unload_model_after = UnloadModelAfter::Minutes5)
        .unwrap();
    h.manager.unload_setting_changed();
    h.clock.advance(Duration::from_secs(4 * 60));
    h.settings
        .update(|s| s.unload_model_after = UnloadModelAfter::Never)
        .unwrap();
    h.manager.unload_setting_changed();
    h.clock.advance(Duration::from_secs(3600));
    h.manager.tick();
    assert!(h.engines.probe(Small).is_loaded());
}

// Rule 24c.
#[test]
fn changing_the_setting_restarts_the_timer_with_the_new_value() {
    let h = idle_harness(UnloadModelAfter::Minutes10);
    h.clock.advance(Duration::from_secs(9 * 60));

    h.settings
        .update(|s| s.unload_model_after = UnloadModelAfter::Minutes2)
        .unwrap();
    h.manager.unload_setting_changed();
    h.clock.advance(Duration::from_secs(119));
    h.manager.tick();
    assert!(h.engines.probe(Small).is_loaded(), "the timer restarted");

    h.clock.advance(Duration::from_secs(1));
    h.manager.tick();
    assert!(!h.engines.probe(Small).is_loaded());

    // Choosing Never does not reload an unloaded Model.
    h.settings
        .update(|s| s.unload_model_after = UnloadModelAfter::Never)
        .unwrap();
    h.manager.unload_setting_changed();
    assert_eq!(h.state().active_state, ActiveModelState::Unloaded);
}

// --- quitting ----------------------------------------------------------------------------------

#[test]
fn shutdown_unloads_the_model_and_keeps_a_partial_download() {
    let h = Harness::with(
        |b| {
            b.chunk = 2000;
            b.chunk_delay = Duration::from_millis(20);
        },
        |h| {
            h.place(Small);
            h.settings.set_active_model(Some(Small));
        },
    );
    h.wait_active(Small);
    h.manager.download(Turbo);
    h.wait("Turbo downloading", |s| {
        matches!(s.models[Turbo.index()].download, DownloadState::Downloading { downloaded, .. } if downloaded > 0)
    });

    h.manager.shutdown();

    assert!(!h.engines.probe(Small).is_loaded());
    thread::sleep(Duration::from_millis(100));
    assert!(fs::metadata(h.partial_path(Turbo)).unwrap().len() > 0);
}

#[test]
fn listeners_hear_every_change() {
    let h = Harness::new();
    let seen: Arc<Mutex<Vec<ModelsState>>> = Arc::default();
    let sink = seen.clone();
    h.manager
        .subscribe(move |s| sink.lock().unwrap().push(s.clone()));

    h.manager.download(Small);
    h.wait_active(Small);
    thread::sleep(Duration::from_millis(20));

    let seen = seen.lock().unwrap();
    assert!(
        seen.iter()
            .any(|s| s.models[Small.index()].download == DownloadState::Verifying)
    );
    assert!(seen.iter().any(|s| s.activating == Some(Small)));
    let last = seen.last().unwrap();
    assert_eq!(last.active, Some(Small));
    assert_eq!(last.active_state, ActiveModelState::Ready);
}
