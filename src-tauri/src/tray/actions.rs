//! What the tray menu items do (rules 8–10), against the app or fakes.

use crate::models::ModelId;
use crate::overlay::app::MainPage;

use super::TrayAction;

/// The parts of Echo the tray menu reaches.
pub trait ActionTarget {
    /// A Cancellation of the current Dictation (`cancel-shortcut.md`).
    fn cancel_dictation(&self);
    /// The newest History entry's text.
    fn latest_transcript(&self) -> Option<String>;
    fn copy_to_clipboard(&self, text: &str) -> Result<(), String>;
    /// Makes a downloaded Model active (`models.md`).
    fn activate_model(&self, model: ModelId);
    /// Shows the main window on `page`.
    fn open_main_page(&self, page: MainPage);
    /// Runs a manual update check (`updater.md` rule 7).
    fn check_for_updates(&self);
    /// Ends the process: downloads stop, the Model is unloaded, the tray icon is removed.
    fn exit(&self);
}

pub fn perform(action: TrayAction, target: &impl ActionTarget) {
    match action {
        TrayAction::Cancel => target.cancel_dictation(),
        TrayAction::CopyLastTranscript => {
            if let Some(text) = target.latest_transcript()
                && let Err(error) = target.copy_to_clipboard(&text)
            {
                log::warn!("could not copy the last Transcript: {error}");
            }
        }
        TrayAction::ActivateModel(model) => target.activate_model(model),
        TrayAction::Settings => target.open_main_page(MainPage::App),
        TrayAction::CheckForUpdates => {
            // The result shows on the App page.
            target.open_main_page(MainPage::App);
            target.check_for_updates();
        }
        TrayAction::Quit => {
            // A Recording is cancelled and a Transcribing abandoned, so nothing is inserted or
            // stored on the way out (rule 10).
            target.cancel_dictation();
            target.exit();
        }
    }
}
