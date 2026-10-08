//! The Overlay (`docs/specs/overlay.md`): the small always-on-top pill that shows a Dictation is
//! getting ready, listening or transcribing, offers a cancel button, and briefly shows messages.
//!
//! - [`controller`]: what the Overlay shows and when its window shows and hides, as pure logic
//!   over an [`OverlaySurface`](controller::OverlaySurface) and an injected clock.
//! - [`placement`]: which monitor and where on it, as pure geometry.
//! - `app`: the Tauri wiring — the controller's thread, the window, commands and events.
//! - `native`: the Win32 details that keep the window from ever taking keyboard focus.

pub mod app;
pub mod controller;
#[cfg(windows)]
mod native;
pub mod placement;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::dictation::ProblemKind;

/// Where on the monitor the Overlay sits (the "Overlay position" setting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum OverlayPosition {
    /// 12 logical pixels above the bottom of the work area, clear of the taskbar (rule 14).
    #[default]
    Bottom,
    /// Just below the top of the work area.
    Top,
}

/// What the Overlay shows (rules 1–5). The frontend draws it; the backend decides it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OverlayView {
    /// Nothing: the window fades out and hides (rules 1 and 6).
    Hidden,
    /// A Recording was requested but no audio has arrived yet (rule 2).
    GettingReady,
    /// Recording with audio flowing: level meter, timer and cancel button (rule 3).
    Listening,
    /// From the end of the Recording until Insertion completes (rule 4). The cancel button shows
    /// only while Transcribing, not once Inserting starts.
    #[serde(rename_all = "camelCase")]
    Transcribing { cancellable: bool },
    /// One short line of text for 2.5 s (rule 5). `actionable`: clicking it does something.
    #[serde(rename_all = "camelCase")]
    Message {
        message: OverlayMessage,
        actionable: bool,
    },
}

impl OverlayView {
    pub fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }

    /// Getting ready or listening: the views that show the timer and level meter.
    pub fn is_recording(&self) -> bool {
        matches!(self, Self::GettingReady | Self::Listening)
    }
}

/// A message the Overlay shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OverlayMessage {
    /// A Dictation error (`dictation-pipeline.md` rule 39a).
    Problem { problem: ProblemKind },
    /// "Selected microphone not found — using the default microphone" (`microphone.md` rule 5).
    MicrophoneFallback,
}

impl OverlayMessage {
    /// Errors are shown even with the Overlay turned off (rule 17).
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Problem { .. })
    }

    /// What clicking the message does, if anything (rule 5).
    pub fn action(&self) -> Option<MessageAction> {
        match self {
            Self::Problem {
                problem:
                    ProblemKind::NoModel
                    | ProblemKind::ModelLoadFailed
                    | ProblemKind::ModelDownloadFailed,
            } => Some(MessageAction::OpenModels),
            Self::Problem {
                problem: ProblemKind::MicrophoneAccessDenied,
            } => Some(MessageAction::OpenMicrophonePrivacy),
            // The main window's notice offers "Copy text" (`dictation-pipeline.md` rule 36).
            Self::Problem {
                problem: ProblemKind::InsertionFailed | ProblemKind::InsertionFailedNotInHistory,
            } => Some(MessageAction::ShowNotices),
            _ => None,
        }
    }
}

/// The action behind a clickable message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageAction {
    /// Show the main window on its Models page.
    OpenModels,
    /// Open the Windows microphone privacy settings.
    OpenMicrophonePrivacy,
    /// Show the main window, where the error notice and its actions are.
    ShowNotices,
}
