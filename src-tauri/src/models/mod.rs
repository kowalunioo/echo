//! **Models**: the built-in list, downloading with resume and verification, the active Model,
//! idle unload and deletion (`docs/specs/models.md`).
//!
//! - [`catalog`] — the three Models with sizes, pinned URLs and SHA-256 (rules 1–3);
//! - [`ModelStorage`] — the models folder and what counts as downloaded (rules 4–6);
//! - [`download`] — one resumable, verified download (rules 9–14);
//! - [`ModelManager`] — the queue, the active Model and its Engine, idle unload, delete, and the
//!   [`DictationGuard`] the dictation pipeline holds while a Dictation runs (rules 7, 15–28).

pub mod catalog;
pub mod commands;
mod disk;
pub mod download;
mod manager;
mod storage;
#[cfg(test)]
pub(crate) mod test_server;

pub use catalog::{CATALOG, ModelId, ModelInfo};
pub use disk::{DiskSpace, SystemDiskSpace};
pub use manager::{
    ActiveModelState, Clock, DictationGuard, DownloadFailure, DownloadState, EngineFactory,
    FailureKind, FakeClock, LoadFailure, ModelEntry, ModelError, ModelManager, ModelProblem,
    ModelProblemKind, ModelSettings, ModelsState, NoActiveModel, SharedEngine, SystemClock,
    TranscribeCppEngines,
};
pub use storage::{ModelFile, ModelStorage, PARTIAL_SUFFIX};

/// Free space that must remain on top of the Model's size before a download starts (rule 8).
pub const DISK_SPACE_MARGIN: u64 = 100 * 1024 * 1024;
