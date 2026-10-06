//! The restart marker (`updater.md` rule 6): written right before the
//! update installer ends Echo, read once on the next start to restore the main window's visibility
//! and show "Echo was updated to <version>".

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::data_dir::write_atomic;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestartMarker {
    /// The version being installed.
    pub version: String,
    /// Whether the main window was visible when the update started installing.
    pub window_visible: bool,
}

/// What the start after an update should do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AfterRestart {
    /// Show the main window (it was visible before the update).
    pub show_window: bool,
    /// The version to announce, when the running Echo is the one that was installed.
    pub updated_to: Option<String>,
}

pub fn write(path: &Path, marker: &RestartMarker) -> std::io::Result<()> {
    let json = serde_json::to_vec(marker).map_err(std::io::Error::other)?;
    write_atomic(path, &json)
}

/// Writes the marker right before the installer ends Echo. A development build never
/// installs, so it never writes one either.
pub fn write_for_restart(
    path: &Path,
    marker: &RestartMarker,
    development: bool,
) -> std::io::Result<()> {
    if development {
        return Ok(());
    }
    write(path, marker)
}

/// Removes the marker: the install did not start, so the next start is an ordinary one.
pub fn remove(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => log::warn!("Couldn't remove the update restart marker: {error}"),
    }
}

/// Whether the main window counts as shown for the marker: a minimized window does not, so
/// it is not brought back focused after the update.
pub fn window_shown(visible: bool, minimized: bool) -> bool {
    visible && !minimized
}

/// Reads and removes the marker. `None` when there is none, it is unreadable, or it is for
/// another version than the running one (the install did not happen).
pub fn take(path: &Path, running_version: &str) -> Option<AfterRestart> {
    let bytes = std::fs::read(path).ok()?;
    if let Err(error) = std::fs::remove_file(path) {
        log::warn!("Couldn't remove the update restart marker: {error}");
    }
    let marker: RestartMarker = match serde_json::from_slice(&bytes) {
        Ok(marker) => marker,
        Err(error) => {
            log::warn!("Ignoring an unreadable update restart marker: {error}");
            return None;
        }
    };
    if marker.version != running_version {
        log::warn!(
            "Ignoring an update restart marker for {} while {running_version} runs",
            marker.version
        );
        return None;
    }
    Some(AfterRestart {
        show_window: marker.window_visible,
        updated_to: Some(marker.version),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(version: &str, window_visible: bool) -> RestartMarker {
        RestartMarker {
            version: version.into(),
            window_visible,
        }
    }

    #[test]
    fn rule_6_a_hidden_window_stays_hidden_and_the_new_version_is_announced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-restart.json");
        write(&path, &marker("0.2.0", false)).unwrap();

        assert_eq!(
            take(&path, "0.2.0"),
            Some(AfterRestart {
                show_window: false,
                updated_to: Some("0.2.0".into())
            })
        );
        // Read once: the next start is an ordinary one.
        assert!(!path.exists());
        assert_eq!(take(&path, "0.2.0"), None);
    }

    #[test]
    fn a_visible_window_is_shown_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-restart.json");
        write(&path, &marker("0.2.0", true)).unwrap();
        assert!(take(&path, "0.2.0").unwrap().show_window);
    }

    #[test]
    fn a_marker_for_another_version_is_removed_and_ignored() {
        // The install did not happen: the start is an ordinary one (show_at_launch decides).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-restart.json");
        write(&path, &marker("0.2.0", true)).unwrap();
        assert_eq!(take(&path, "0.1.0"), None);
        assert!(!path.exists());
    }

    #[test]
    fn the_marker_is_written_only_by_release_builds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-restart.json");
        write_for_restart(&path, &marker("0.2.0", true), true).unwrap();
        assert!(!path.exists());
        write_for_restart(&path, &marker("0.2.0", true), false).unwrap();
        assert!(take(&path, "0.2.0").unwrap().show_window);
    }

    #[test]
    fn remove_deletes_the_marker_after_a_failed_install() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-restart.json");
        write(&path, &marker("0.2.0", true)).unwrap();
        remove(&path);
        assert!(!path.exists());
        // Nothing to remove is fine too.
        remove(&path);
    }

    #[test]
    fn a_minimized_window_counts_as_hidden() {
        assert!(window_shown(true, false));
        assert!(!window_shown(true, true));
        assert!(!window_shown(false, false));
        assert!(!window_shown(false, true));
    }

    #[test]
    fn an_unreadable_marker_is_removed_and_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-restart.json");
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(take(&path, "0.2.0"), None);
        assert!(!path.exists());
    }
}
