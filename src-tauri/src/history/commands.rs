//! The History commands and event shared with the frontend (`src/store/history.ts`).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};

use super::{History, HistoryEntry, ReinsertError, reinsert};
use crate::insertion::SharedInserter;
use crate::window;

/// Sent to every window after any change to History, with every entry, newest first (rule 16).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct HistoryChanged(pub Vec<HistoryEntry>);

/// How long Re-insert waits after hiding Echo's window for Windows to hand focus back to the
/// application the user was in.
const FOCUS_RETURN_DELAY: Duration = Duration::from_millis(250);

/// Every History entry, newest first.
#[tauri::command]
#[specta::specta]
pub fn list_history(history: State<'_, History>) -> Result<Vec<HistoryEntry>, String> {
    history.entries().map_err(|e| e.to_string())
}

/// Permanently deletes one entry and returns it (or `null` if it was already gone), so the
/// window can offer Undo for 5 s (rule 12).
#[tauri::command]
#[specta::specta]
pub fn delete_history_entry(
    history: State<'_, History>,
    id: u32,
) -> Result<Option<HistoryEntry>, String> {
    history.delete(id).map_err(|e| e.to_string())
}

/// Puts back an entry returned by `delete_history_entry`, with its original id and time (Undo).
#[tauri::command]
#[specta::specta]
pub fn restore_history_entry(
    history: State<'_, History>,
    entry: HistoryEntry,
) -> Result<(), String> {
    history.restore(&entry).map_err(|e| e.to_string())
}

/// Deletes every entry; the window asks for confirmation first (rule 14).
#[tauri::command]
#[specta::specta]
pub fn clear_history(history: State<'_, History>) -> Result<(), String> {
    history.clear().map_err(|e| e.to_string())
}

/// Hides Echo's window and inserts the entry's text into the application that had focus before
/// (rule 13). If Insertion fails, the window comes back so the user sees the error.
#[tauri::command]
#[specta::specta]
pub async fn reinsert_history_entry(app: AppHandle, id: u32) -> Result<(), String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<History>();
        let inserter = app.state::<SharedInserter>();
        let result = reinsert(&history, &inserter, id, || {
            if let Some(main) = app.get_webview_window(window::MAIN_WINDOW) {
                let _ = main.hide();
            }
            std::thread::sleep(FOCUS_RETURN_DELAY);
        });
        if let Err(ReinsertError::Insertion(error)) = &result {
            log::warn!("Re-insert failed: {error}");
            window::show_main(&app);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?;
    result.map_err(|e| e.to_string())
}
