//! The restart marker (`updater.md` rule 6): written right before an update
//! installs, read once on the next start to restore the main window's visibility
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

/// Reads and removes the marker. `None` when there is none or it is unreadable.
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
    Some(AfterRestart {
        show_window: marker.window_visible,
        updated_to: (marker.version == running_version).then_some(marker.version),
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
    fn a_failed_install_announces_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-restart.json");
        write(&path, &marker("0.2.0", true)).unwrap();
        assert_eq!(take(&path, "0.1.0").unwrap().updated_to, None);
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
