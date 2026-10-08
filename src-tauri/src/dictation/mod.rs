//! The Dictation pipeline (`docs/specs/dictation-pipeline.md`): Record Shortcut intents in,
//! Recording → voice-activity detection → Engine → clean-up → History → Insertion out.
//!
//! - [`machine`]: the state machine as pure logic.
//! - [`vad`]: voice-activity detection with pre/post roll and padding.
//! - [`cleanup`]: Transcript clean-up.
//! - [`language`]: the Dictation Language setting and its resolution against the active Model.
//! - [`indicator`]: error indication state for the Overlay, tray and main window.
//! - [`runtime`]: the worker thread that runs the machine against the real (or fake) pieces.
//! - `app`: Tauri wiring, commands and the `DictationStatusChanged` event.

pub mod app;
pub mod cleanup;
pub mod indicator;
pub mod language;
pub mod machine;
pub mod runtime;
pub mod vad;

use serde::{Deserialize, Serialize};
use specta::Type;

pub use runtime::{
    Dictation, DictationContext, DictationDeps, DictationModel, DictationModels, EngineModel,
};

/// The state of the Dictation as the user sees it (rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DictationState {
    #[default]
    Idle,
    Recording,
    Transcribing,
    Inserting,
}

/// What went wrong (rule 39a). The UI translates the kind; `detail` is technical English.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ProblemKind {
    /// No Model is active, so the Recording did not start (rule 5).
    NoModel,
    /// There is no microphone (rule 8).
    MicrophoneNotFound,
    /// Windows privacy settings block the microphone (rule 8).
    MicrophoneAccessDenied,
    /// The microphone was lost during a Recording; what was captured is still transcribed
    /// (`microphone.md` rule 10).
    MicrophoneDisconnected,
    /// Any other microphone failure (rule 8).
    MicrophoneFailed,
    /// The Model could not be loaded for this Dictation (`models.md`).
    ModelLoadFailed,
    /// A Model download failed (`models.md`).
    ModelDownloadFailed,
    /// The Engine failed (rule 23).
    TranscriptionFailed,
    /// Insertion failed; the Transcript is in History and kept for copying (rule 36).
    InsertionFailed,
    /// Insertion failed and History keeps nothing (History limit 0); the Transcript is kept only
    /// for copying until the next Recording starts (rule 36).
    InsertionFailedNotInHistory,
}

/// One Dictation error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictationProblem {
    /// Increases with every error, so consumers can tell a new error from one already shown
    /// (the Overlay shows each one once, for 2.5 s).
    pub id: u32,
    pub kind: ProblemKind,
    /// Technical detail in English (may be empty).
    pub detail: String,
}

/// Everything the Overlay, the tray and the main window need about dictation, published as one
/// event whenever any part changes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictationStatus {
    pub state: DictationState,
    /// During a Recording: audio has arrived from the device, so the Overlay shows "listening"
    /// rather than "getting ready" (rule 7). `false` in every other state.
    pub listening: bool,
    /// The tray's error: the most recent error, until the main window is seen or a Dictation
    /// succeeds (rule 39c). `None` means no red icon.
    pub error: Option<DictationProblem>,
    /// Errors for the main window, oldest first, until the user dismisses them (rule 39d).
    pub notices: Vec<DictationProblem>,
    /// The id of the Insertion error whose Transcript Echo still holds for "Copy text", until the
    /// next Recording starts (rule 36). `None` when nothing is kept.
    pub kept_transcript: Option<u32>,
}
