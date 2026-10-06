//! Where Model files live and what counts as downloaded (`models.md` rules 4–6, 25, 28).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::ModelId;

/// The suffix of an in-progress download, next to its final name (rule 4).
pub const PARTIAL_SUFFIX: &str = ".partial";

/// Where one Model's file comes from and what it must look like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFile {
    pub file_name: String,
    pub size: u64,
    /// Lower-case hex SHA-256.
    pub sha256: String,
    pub url: String,
}

impl ModelFile {
    /// The built-in source of a Model (`catalog`).
    pub fn builtin(id: ModelId) -> Self {
        let info = id.info();
        Self {
            file_name: info.file_name.into(),
            size: info.size,
            sha256: info.sha256.into(),
            url: info.url.into(),
        }
    }
}

/// The models folder, `%LOCALAPPDATA%\com.enloque.echo\models\`. Echo never looks for Model files
/// anywhere else (rule 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelStorage {
    dir: PathBuf,
    files: [ModelFile; 3],
}

impl ModelStorage {
    /// The models folder with the built-in Model files.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self::with_files(dir, ModelFile::builtin)
    }

    /// The models folder with other sources and sizes (tests use small files on a local server).
    pub fn with_files(dir: impl Into<PathBuf>, file: impl Fn(ModelId) -> ModelFile) -> Self {
        Self {
            dir: dir.into(),
            files: ModelId::ALL.map(file),
        }
    }

    /// Where the Model's file comes from and what it must look like.
    pub fn file(&self, id: ModelId) -> &ModelFile {
        &self.files[id.index()]
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The verified Model file.
    pub fn final_path(&self, id: ModelId) -> PathBuf {
        self.dir.join(&self.file(id).file_name)
    }

    /// The in-progress download of the Model.
    pub fn partial_path(&self, id: ModelId) -> PathBuf {
        let mut name = self.file(id).file_name.clone();
        name.push_str(PARTIAL_SUFFIX);
        self.dir.join(name)
    }

    /// A Model is downloaded when its final file exists with the expected size; the checksum was
    /// checked when it was downloaded, not on every start (rule 5).
    pub fn is_downloaded(&self, id: ModelId) -> bool {
        fs::metadata(self.final_path(id))
            .is_ok_and(|meta| meta.is_file() && meta.len() == self.file(id).size)
    }

    /// The length of the partial download, if there is one.
    pub fn partial_len(&self, id: ModelId) -> Option<u64> {
        fs::metadata(self.partial_path(id))
            .ok()
            .filter(|meta| meta.is_file())
            .map(|meta| meta.len())
    }

    /// Removes the final file and any partial file. Files that are already gone are not an error
    /// (rule 28).
    pub fn delete(&self, id: ModelId) -> io::Result<()> {
        for path in [self.final_path(id), self.partial_path(id)] {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// The space deleting the Model frees: its final and partial files.
    pub fn bytes_on_disk(&self, id: ModelId) -> u64 {
        let final_len = fs::metadata(self.final_path(id)).map_or(0, |m| m.len());
        final_len + self.partial_len(id).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_live_in_the_models_folder_with_a_partial_suffix_while_downloading() {
        let storage = ModelStorage::new("C:/data/models");
        assert_eq!(
            storage.final_path(ModelId::WhisperSmall),
            Path::new("C:/data/models/whisper-small-Q8_0.gguf")
        );
        assert_eq!(
            storage.partial_path(ModelId::WhisperSmall),
            Path::new("C:/data/models/whisper-small-Q8_0.gguf.partial")
        );
    }

    #[test]
    fn only_a_final_file_of_the_expected_size_counts_as_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let storage = ModelStorage::new(dir.path());
        let id = ModelId::WhisperSmall;
        assert!(!storage.is_downloaded(id));

        fs::write(storage.final_path(id), b"too short").unwrap();
        assert!(!storage.is_downloaded(id));

        let file = fs::File::create(storage.final_path(id)).unwrap();
        file.set_len(id.info().size).unwrap();
        drop(file);
        assert!(storage.is_downloaded(id));

        // A partial file never counts.
        fs::remove_file(storage.final_path(id)).unwrap();
        fs::File::create(storage.partial_path(id))
            .unwrap()
            .set_len(id.info().size)
            .unwrap();
        assert!(!storage.is_downloaded(id));
    }

    #[test]
    fn delete_removes_both_files_and_succeeds_when_they_are_gone() {
        let dir = tempfile::tempdir().unwrap();
        let storage = ModelStorage::new(dir.path());
        let id = ModelId::ParakeetTdt06bV3;
        fs::write(storage.final_path(id), b"model").unwrap();
        fs::write(storage.partial_path(id), b"part").unwrap();
        assert_eq!(storage.bytes_on_disk(id), 9);

        storage.delete(id).unwrap();
        assert!(!storage.final_path(id).exists());
        assert!(!storage.partial_path(id).exists());

        storage.delete(id).unwrap();
        assert_eq!(storage.partial_len(id), None);
    }
}
