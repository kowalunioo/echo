//! The Model commands and events shared with the frontend (`src/store/models.ts`).

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};

use super::{ModelId, ModelManager, ModelProblem, ModelSettings, ModelsState};
use crate::settings::{SettingsStore, UnloadModelAfter};

/// Sent to every window after any change to the Models: downloads and their progress, the
/// active Model and its state.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ModelsChanged(pub ModelsState);

/// Sent when a Model download or load fails (the error indication, `dictation-pipeline.md`
/// rules 39a–39d).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ModelProblemOccurred(pub ModelProblem);

/// Returns the Models and their state.
#[tauri::command]
#[specta::specta]
pub fn get_models(models: State<'_, ModelManager>) -> ModelsState {
    models.state()
}

/// Starts downloading a Model, or queues it behind the running download.
#[tauri::command]
#[specta::specta]
pub fn download_model(models: State<'_, ModelManager>, model: ModelId) {
    models.download(model);
}

/// Cancels a running or queued download; the partial file is kept for "Resume".
#[tauri::command]
#[specta::specta]
pub fn cancel_model_download(models: State<'_, ModelManager>, model: ModelId) {
    models.cancel_download(model);
}

/// Makes a downloaded Model active. Returns once loading has started; the result arrives as a
/// `ModelsChanged` event (active, or a `loadFailure`).
#[tauri::command]
#[specta::specta]
pub fn activate_model(models: State<'_, ModelManager>, model: ModelId) -> Result<(), String> {
    models.activate(model).map_err(|e| e.to_string())
}

/// Deletes a downloaded Model or a paused download.
#[tauri::command]
#[specta::specta]
pub fn delete_model(models: State<'_, ModelManager>, model: ModelId) -> Result<(), String> {
    models.delete(model).map_err(|e| e.to_string())
}

/// The settings as the Model manager sees them, read from the managed [`SettingsStore`].
pub struct AppModelSettings(pub AppHandle);

impl ModelSettings for AppModelSettings {
    fn active_model(&self) -> Option<ModelId> {
        self.0.state::<SettingsStore>().active_model()
    }

    fn set_active_model(&self, model: Option<ModelId>) {
        self.0.state::<SettingsStore>().set_active_model(model);
    }

    fn unload_model_after(&self) -> UnloadModelAfter {
        self.0.state::<SettingsStore>().unload_model_after()
    }
}
