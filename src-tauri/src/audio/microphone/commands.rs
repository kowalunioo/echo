//! The Microphone commands used by the Dictation page and the onboarding.

use tauri::State;

use super::{DeviceList, MicrophoneAccess, Microphones};

/// A fresh list of the input devices and the current Windows default. The Microphone picker
/// asks for it every time it opens (`microphone.md` rule 8).
#[tauri::command]
#[specta::specta]
pub async fn list_microphones(microphones: State<'_, Microphones>) -> Result<DeviceList, String> {
    let devices = std::sync::Arc::clone(microphones.devices());
    // Enumerating audio devices can take a moment; keep it off the async runtime's threads.
    tauri::async_runtime::spawn_blocking(move || devices.list())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Whether Windows privacy settings let desktop apps use the Microphone (`microphone.md` rule 7;
/// the onboarding's Microphone access step).
#[tauri::command]
#[specta::specta]
pub fn microphone_access(microphones: State<'_, Microphones>) -> MicrophoneAccess {
    microphones.devices().access()
}
