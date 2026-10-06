//! The settings commands and event shared with the frontend (`src/store/settings.ts`).

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use super::{Settings, SettingsPatch, SettingsStore};

/// Sent to every window after any change to the settings, with the complete new settings.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SettingsChanged(pub Settings);

/// Returns the current settings.
#[tauri::command]
#[specta::specta]
pub fn get_settings(store: State<'_, SettingsStore>) -> Settings {
    store.get()
}

/// Changes the settings present in `patch` (e.g. `{ uiLanguage: "en" }`) and saves them at once.
/// Returns the new settings; every window also receives them as a `SettingsChanged` event.
#[tauri::command]
#[specta::specta]
pub fn update_settings(
    store: State<'_, SettingsStore>,
    patch: SettingsPatch,
) -> Result<Settings, String> {
    store.apply_patch(patch).map_err(|error| error.to_string())
}

/// Puts the setting `key` (e.g. `"uiLanguage"`) back to its default.
#[tauri::command]
#[specta::specta]
pub fn reset_setting(store: State<'_, SettingsStore>, key: String) -> Result<Settings, String> {
    store.reset(&key).map_err(|error| error.to_string())
}
