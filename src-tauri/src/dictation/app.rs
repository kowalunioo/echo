//! The Dictation pipeline inside the Tauri app: wires the real pieces into a [`Dictation`],
//! publishes [`DictationStatusChanged`], and offers the commands the main window uses for error
//! notices (rules 39b–39d).

use std::sync::{Arc, PoisonError};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use super::vad::Earshot;
use super::{
    Dictation, DictationContext, DictationDeps, DictationModel, DictationModels, DictationStatus,
    ProblemKind, runtime::MAX_RECORDING,
};
use crate::audio::AudioSource;
use crate::audio::fake_microphone::FakeMicrophone;
use crate::audio::microphone::{MicrophoneNotice, Microphones};
use crate::engine::{EngineError, TranscriptionRequest};
use crate::history::History;
use crate::insertion::SharedInserter;
use crate::models::{DictationGuard, ModelManager, ModelProblemKind, NoActiveModel};
use crate::overlay::app::Overlay;
use crate::settings::SettingsStore;
use crate::shortcut::record::RecordShortcutHandle;

/// The dictation status changed: state, "listening", tray error or main-window notices. The one
/// event the Overlay, the tray and the main window follow.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct DictationStatusChanged(pub DictationStatus);

/// Starts the pipeline. Needs the settings, History, the [`SharedInserter`], the Microphones and
/// the [`ModelManager`] managed already; the Record Shortcut (installed afterwards) feeds it.
pub fn install(app: &AppHandle) {
    register_inserter(app);

    let overlay = app.try_state::<Overlay>().map(|o| o.inner().clone());
    let source = audio_source(app, overlay.clone());
    let history_app = app.clone();
    let shortcut_app = app.clone();
    let publish_app = app.clone();
    let dictation = Dictation::spawn(DictationDeps {
        models: Box::new(AppModels(app.state::<ModelManager>().inner().clone())),
        source,
        detector: Box::new(|| Box::new(Earshot::new())),
        // Dictation Language and Vocabulary have no settings yet; their specs' defaults apply.
        context: Box::new(DictationContext::default),
        history: Box::new(move |entry| {
            history_app
                .state::<History>()
                .add(entry)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }),
        inserter: app.state::<SharedInserter>().inner().clone(),
        shortcut_reset: Box::new(move || {
            if let Some(shortcut) = shortcut_app.try_state::<RecordShortcutHandle>() {
                shortcut.recording_ended();
            }
        }),
        publish: Box::new(move |status| {
            if let Some(overlay) = &overlay {
                overlay.status(status);
            }
            if let Err(error) = DictationStatusChanged(status.clone()).emit(&publish_app) {
                log::warn!("could not send the dictation status to the windows: {error}");
            }
        }),
        max_recording: MAX_RECORDING,
    });

    let reporter = dictation.clone();
    app.state::<ModelManager>().on_problem(move |problem| {
        let kind = match problem.kind {
            ModelProblemKind::Download => ProblemKind::ModelDownloadFailed,
            ModelProblemKind::Load => ProblemKind::ModelLoadFailed,
        };
        reporter.report_problem(kind, format!("{}: {}", problem.model_name, problem.detail));
    });
    app.manage(dictation);
}

/// The Microphone chosen in the settings or, in fake-microphone mode, the WAV Audio Source
/// (rules 40-46). Also makes [`TestAudio`] available for the marker (rule 41).
fn audio_source(app: &AppHandle, overlay: Option<Overlay>) -> Box<dyn AudioSource> {
    let fake = FakeMicrophone::from_launch(std::env::args_os(), |key| std::env::var_os(key));
    app.manage(TestAudio(fake.as_ref().map(FakeMicrophone::file_name)));
    if let Some(fake) = fake {
        log::warn!(
            "fake-microphone mode: Dictations record from {} ({:?}) instead of the Microphone",
            fake.path.display(),
            fake.options
        );
        let source = fake.source();
        if let Some(error) = source.error() {
            log::error!("fake-microphone mode: the WAV file cannot be played: {error}");
        }
        return Box::new(source);
    }
    let settings_app = app.clone();
    Box::new(app.state::<Microphones>().source(
        move || settings_app.state::<SettingsStore>().get().microphone,
        move |notice| {
            log::info!("microphone notice: {notice:?}");
            match notice {
                MicrophoneNotice::SelectedNotFound { .. } => {
                    if let Some(overlay) = &overlay {
                        overlay.microphone_fallback();
                    }
                }
            }
        },
    ))
}

/// The WAV file name while fake-microphone mode is active, `None` in normal operation.
pub struct TestAudio(pub Option<String>);

/// The test-audio marker (rule 41): the name of the WAV file that replaces the Microphone, or
/// `null` in normal operation.
#[tauri::command]
#[specta::specta]
pub fn get_test_audio(test_audio: State<'_, TestAudio>) -> Option<String> {
    test_audio.0.clone()
}

fn register_inserter(app: &AppHandle) {
    #[cfg(windows)]
    match crate::insertion::system_inserter() {
        Ok(inserter) => app.state::<SharedInserter>().set(inserter),
        Err(error) => log::error!("Insertion is unavailable: {error}"),
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// Dictations take the active Model from the [`ModelManager`].
struct AppModels(ModelManager);

impl DictationModels for AppModels {
    fn begin(&self) -> Result<Arc<dyn DictationModel>, NoActiveModel> {
        let guard = self.0.begin_dictation()?;
        Ok(Arc::new(GuardModel(guard)))
    }
}

/// One Dictation's hold on the active Model; dropping it ends the Dictation for the manager.
struct GuardModel(DictationGuard);

impl DictationModel for GuardModel {
    fn name(&self) -> String {
        self.0.model().info().name.to_owned()
    }

    fn load(&self) -> Result<(), EngineError> {
        self.0.load()
    }

    fn transcribe(&self, request: TranscriptionRequest<'_>) -> Result<String, EngineError> {
        self.0
            .engine()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .transcribe(request)
    }

    fn unload(&self) {
        self.0
            .engine()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .unload();
    }
}

/// The current dictation status, for windows that open after it was published.
#[tauri::command]
#[specta::specta]
pub fn get_dictation_status(dictation: State<'_, Dictation>) -> DictationStatus {
    dictation.status()
}

/// The main window is visible and focused: the tray error clears (rule 39c).
#[tauri::command]
#[specta::specta]
pub fn dictation_window_seen(dictation: State<'_, Dictation>) {
    dictation.window_seen();
}

/// The user dismissed the error notices in the main window.
#[tauri::command]
#[specta::specta]
pub fn dismiss_dictation_notices(dictation: State<'_, Dictation>) {
    dictation.dismiss_notices();
}
