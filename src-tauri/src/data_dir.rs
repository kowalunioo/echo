//! Echo's data directory and safe file writes.
//!
//! Everything Echo stores lives under one folder, `%LOCALAPPDATA%\com.enloque.echo\`
//! (`settings-and-first-run.md` rule 12, `models.md` rule 4). Local rather than roaming
//! application data, so large files (Models, logs) never travel with the user profile. Later
//! slices add their own paths here (e.g. the History database) so the layout stays in one place.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The layout of Echo's data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The data directory itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The settings file (`settings.json`).
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// The remembered size and position of the main window.
    pub fn window_state_file(&self) -> PathBuf {
        self.root.join("window-state.json")
    }

    /// Written right before an update installs, read on the next start (`updater.md` rule 6).
    pub fn update_marker_file(&self) -> PathBuf {
        self.root.join("update-restart.json")
    }

    /// The History database (`history.md` rule 5).
    pub fn history_file(&self) -> PathBuf {
        self.root.join("history.db")
    }

    /// The folder holding the Model files and their in-progress downloads (`models.md` rule 4).
    pub fn models_dir(&self) -> PathBuf {
        self.root.join("models")
    }

    /// The folder holding the rotated diagnostic log files.
    pub fn log_dir(&self) -> PathBuf {
        self.root.join("logs")
    }
}

/// Replaces `path` with `contents` so that a crash or power loss leaves either the old or the new
/// file, never a half-written one: the data goes to a sibling temporary file, is flushed to disk,
/// and is then renamed over the target (an atomic replace on NTFS).
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp_name = path.file_name().unwrap_or_default().to_owned();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    {
        let mut file = File::create(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// The bytes of a JSON file Echo reads back, without a leading UTF-8 byte order mark. Windows
/// PowerShell 5.1 and Notepad may add one when the user edits the file by hand, and `serde_json`
/// rejects it. Echo itself always writes without one.
pub fn without_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_creates_missing_folders_and_replaces_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("file.json");

        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"second");
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("file.json")]);
    }

    #[test]
    fn a_leading_utf8_bom_is_stripped_and_nothing_else_is() {
        assert_eq!(without_utf8_bom(b"\xEF\xBB\xBF{}"), b"{}");
        assert_eq!(without_utf8_bom(b"{}"), b"{}");
        assert_eq!(without_utf8_bom(b"\xEF\xBB"), b"\xEF\xBB");
        assert_eq!(without_utf8_bom(b""), b"");
    }

    #[test]
    fn layout_keeps_everything_under_the_root() {
        let data = DataDir::new("C:/data");
        assert_eq!(data.settings_file(), Path::new("C:/data/settings.json"));
        assert_eq!(data.log_dir(), Path::new("C:/data/logs"));
        assert!(data.window_state_file().starts_with(data.root()));
        assert_eq!(data.history_file(), Path::new("C:/data/history.db"));
        assert_eq!(data.models_dir(), Path::new("C:/data/models"));
        assert_eq!(
            data.update_marker_file(),
            Path::new("C:/data/update-restart.json")
        );
    }
}
