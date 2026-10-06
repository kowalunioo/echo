//! Commands that open places in Windows for the user.

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

/// The Windows Settings page for microphone privacy (`microphone.md` rule 7).
pub const MICROPHONE_PRIVACY_URI: &str = "ms-settings:privacy-microphone";

/// Opens the folder with Echo's log files in File Explorer (the "Open log folder" link in the
/// App section, `settings-and-first-run.md` "UI").
#[tauri::command]
#[specta::specta]
pub fn open_log_folder(app: AppHandle) -> Result<(), String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Opens the Windows microphone privacy settings ("Open Windows privacy settings" in the
/// onboarding's Microphone access step).
#[tauri::command]
#[specta::specta]
pub fn open_microphone_privacy_settings(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(MICROPHONE_PRIVACY_URI, None::<&str>)
        .map_err(|e| e.to_string())
}
