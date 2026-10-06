//! The Record Shortcut inside the Tauri app: commands for the Dictation page, the events it
//! listens to, and start-up wiring.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use super::modes::{RecordIntent, ShortcutMode};
use super::record::{
    InMemoryConfigStore, RecordShortcut, RecordShortcutHandle, RecordShortcutState,
    ShortcutChangeError, spawn,
};
use super::{KeyAction, ShortcutListener};

/// A key pressed or released during shortcut capture, by its capture name (`"LeftCtrl"`,
/// `"RightAlt"`, `"Space"`, `"F9"`).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct CapturedKeyEvent {
    pub key: String,
    pub pressed: bool,
}

/// TEMPORARY (remove with #13): every Record Shortcut intent, so the hook can be checked by hand
/// on the Dictation page until the dictation pipeline consumes the intents.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct RecordIntentEvent {
    pub intent: RecordIntent,
}

/// Starts the Record Shortcut: installs the keyboard hook, binds the configured combination and
/// makes the [`RecordShortcutHandle`] available to commands (and later the pipeline).
pub fn install(app: &AppHandle) {
    #[cfg(windows)]
    let listener: Box<dyn ShortcutListener> = Box::new(super::WindowsShortcutListener::new());
    #[cfg(not(windows))]
    let listener: Box<dyn ShortcutListener> = Box::new(super::FakeShortcutListener::new());

    // The settings store (#23) replaces the in-memory store once it lands.
    let core = RecordShortcut::new(listener, Box::new(InMemoryConfigStore::default()));
    let intents_app = app.clone();
    let keys_app = app.clone();
    let (handle, started) = spawn(
        core,
        // TEMPORARY: until the pipeline (#13) exists, intents only drive the dev indicator.
        Box::new(move |intent| {
            let _ = RecordIntentEvent { intent }.emit(&intents_app);
        }),
        Box::new(move |key| {
            let _ = CapturedKeyEvent {
                key: key.key,
                pressed: key.action == KeyAction::Pressed,
            }
            .emit(&keys_app);
        }),
    );
    if let Err(error) = started {
        eprintln!("Echo: the Record Shortcut is unavailable: {error}");
    }
    app.manage(handle);
}

/// The current Record Shortcut and mode.
#[tauri::command]
#[specta::specta]
pub fn record_shortcut(shortcut: State<'_, RecordShortcutHandle>) -> RecordShortcutState {
    shortcut.state()
}

/// Validates and activates a new Record Shortcut given in canonical text form (`"Ctrl+Space"`).
/// On failure the previous one stays active. Ends a capture in progress.
#[tauri::command]
#[specta::specta]
pub fn set_record_shortcut(
    shortcut: State<'_, RecordShortcutHandle>,
    combination: String,
) -> Result<RecordShortcutState, ShortcutChangeError> {
    shortcut.set_combination(&combination)
}

/// Restores the default Record Shortcut, Ctrl+Space.
#[tauri::command]
#[specta::specta]
pub fn reset_record_shortcut(
    shortcut: State<'_, RecordShortcutHandle>,
) -> Result<RecordShortcutState, ShortcutChangeError> {
    shortcut.reset()
}

/// Switches between Push-to-Talk Mode and Toggle Mode.
#[tauri::command]
#[specta::specta]
pub fn set_shortcut_mode(
    shortcut: State<'_, RecordShortcutHandle>,
    mode: ShortcutMode,
) -> RecordShortcutState {
    shortcut.set_mode(mode)
}

/// Starts shortcut capture: the Record Shortcut is suspended and keys arrive as
/// [`CapturedKeyEvent`]s instead of reaching applications.
#[tauri::command]
#[specta::specta]
pub fn begin_shortcut_capture(shortcut: State<'_, RecordShortcutHandle>) -> Result<(), String> {
    shortcut.begin_capture().map_err(|e| e.to_string())
}

/// A key pressed or released in Echo's own window, by capture name. Windows does not run Echo's
/// keyboard hook while Echo's window has focus, so the window forwards its keys here.
#[tauri::command]
#[specta::specta]
pub fn own_window_key(shortcut: State<'_, RecordShortcutHandle>, key: String, pressed: bool) {
    let action = if pressed {
        KeyAction::Pressed
    } else {
        KeyAction::Released
    };
    shortcut.own_window_key(&key, action);
}

/// Ends shortcut capture without changes.
#[tauri::command]
#[specta::specta]
pub fn end_shortcut_capture(shortcut: State<'_, RecordShortcutHandle>) {
    shortcut.end_capture();
}
