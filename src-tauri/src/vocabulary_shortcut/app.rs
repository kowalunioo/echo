//! The Vocabulary Shortcut inside the Tauri app: the Selection Reader in managed state, and
//! running each attempt the shortcut worker hands over (`shortcut::app`).
//!
//! The setting row on the Dictation page changes the combination through
//! `shortcut::app::set_vocabulary_shortcut` and the switch through the generic settings commands
//! (`vocabularyShortcutEnabled`). The Vocabulary page shows an added entry at once because the
//! settings store sends `SettingsChanged` to every window when the Vocabulary is saved.

use std::sync::{Mutex, PoisonError};

use tauri::{AppHandle, Manager};

use super::run_attempt;
use super::selection::SelectionReader;
use crate::overlay::app::Overlay;
use crate::settings::SettingsStore;
use crate::shortcut::record::VocabularyAttempt;

/// The Selection Reader, shared by the attempts (one runs at a time).
pub struct SharedSelectionReader(Mutex<Box<dyn SelectionReader>>);

impl SharedSelectionReader {
    pub fn new(reader: Box<dyn SelectionReader>) -> Self {
        Self(Mutex::new(reader))
    }
}

/// Makes the real Selection Reader available. It owns its own clipboard thread, separate from
/// the Inserter's; the two never work at once because the shortcut does nothing during a
/// Dictation (rule 5).
pub fn install(app: &AppHandle) {
    app.manage(SharedSelectionReader::new(system_reader()));
}

#[cfg(windows)]
fn system_reader() -> Box<dyn SelectionReader> {
    use super::selection::{CopySelectionReader, FakeSelectionReader};
    use super::win32::WindowsForegroundWindow;
    use crate::insertion::paste::SystemClock;
    use crate::insertion::{WindowsClipboard, WindowsDesktop};

    match WindowsClipboard::start() {
        Ok(clipboard) => Box::new(CopySelectionReader::new(
            clipboard,
            WindowsDesktop,
            WindowsForegroundWindow,
            SystemClock,
        )),
        Err(error) => {
            log::error!("the Vocabulary Shortcut cannot read selections: {error}");
            Box::new(FakeSelectionReader::new())
        }
    }
}

#[cfg(not(windows))]
fn system_reader() -> Box<dyn SelectionReader> {
    Box::new(super::selection::FakeSelectionReader::new())
}

/// Runs `attempt` on its own thread (the copy waits up to half a second) and shows its message.
/// Dropping the attempt at the end lets the next press start another.
pub fn run(app: &AppHandle, attempt: VocabularyAttempt) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("echo-vocabulary-shortcut".into())
        .spawn(move || {
            let (Some(reader), Some(store)) = (
                app.try_state::<SharedSelectionReader>(),
                app.try_state::<SettingsStore>(),
            ) else {
                return;
            };
            let mut reader = reader.0.lock().unwrap_or_else(PoisonError::into_inner);
            let message = run_attempt(
                reader.as_mut(),
                &store,
                attempt.release_by,
                &attempt.shortcut,
            );
            drop(reader);
            if let (Some(message), Some(overlay)) = (message, app.try_state::<Overlay>()) {
                overlay.notice(message);
            }
            drop(attempt);
        });
    if let Err(error) = spawned {
        log::error!("could not run the Vocabulary Shortcut: {error}");
    }
}
