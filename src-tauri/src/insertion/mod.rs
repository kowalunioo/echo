//! The **Inserter** seam: Insertion of a Transcript into the application that has keyboard
//! focus.
//!
//! The real Inserter pastes through the clipboard and restores it once the target has read the
//! Transcript, typing the text as the fallback (`docs/specs/dictation-pipeline.md` rules 31–37).
//! Its decisions live in [`paste`] behind small clipboard/keyboard seams; the Windows calls in
//! `win32`. Build it with [`system_inserter`]. [`FakeInserter`] records what it was given.

mod fake;
pub mod paste;
#[cfg(windows)]
mod win32;

pub use fake::FakeInserter;
#[cfg(windows)]
pub use win32::{WindowsInserter, system_inserter};

/// Why an Insertion failed. Either way the Transcript is already in History and the user is told
/// "Couldn't insert the text — it is in History" (rule 36).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InsertionError {
    /// Windows refused simulated input, e.g. into a window running as administrator.
    #[error("Windows blocked input into the focused window: {0}")]
    Blocked(String),
    /// Both clipboard paste and typing failed for another reason.
    #[error("insertion failed: {0}")]
    Failed(String),
}

/// Delivers a Transcript into whichever application has keyboard focus.
///
/// # Contract
///
/// - [`insert`](Inserter::insert) delivers `text` exactly — no added space or newline (rule 32)
///   — to the application focused at the moment of the call (rule 31).
/// - It is blocking and runs on a worker thread. It returns once the text has been delivered or
///   every method has failed; restoring the user's clipboard may finish in the background
///   afterwards (rule 34).
/// - Waiting for held modifier keys and choosing between paste and typing are the
///   implementation's business (rules 33–35); the caller only learns success or failure.
pub trait Inserter: Send {
    /// Inserts `text` into the focused application.
    fn insert(&mut self, text: &str) -> Result<(), InsertionError>;
}
