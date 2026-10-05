//! The **Engine** seam: converts speech audio into a Transcript using a Model.
//!
//! The real Engine (transcribe-cpp, ADR 0003) arrives in a later slice; nothing outside its own
//! module will see transcribe-cpp types. [`FakeEngine`] stands in for it in tests.

mod fake;

pub use fake::{FakeEngine, ReceivedRequest};

/// Sample rate of the audio an Engine accepts.
pub const ENGINE_SAMPLE_RATE: u32 = crate::audio::TARGET_SAMPLE_RATE;

/// The effective Dictation Language for one Dictation, already resolved against the active
/// Model's capabilities (`docs/specs/dictation-language.md` rule 4) — the Engine does no
/// resolution of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationLanguage {
    /// Let the Model detect the spoken language.
    Automatic,
    /// Transcribe in this language: an ISO 639-1 code such as `"pl"` or `"en"`.
    Specific(String),
}

/// Everything the Engine needs for one Dictation (`dictation-pipeline.md` rule 22).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TranscriptionRequest<'a> {
    /// The kept speech audio: 16 kHz mono samples in `-1.0..=1.0`, at least 1.25 s long
    /// (shorter speech is padded before it reaches the Engine — rule 19).
    pub audio: &'a [f32],
    /// The effective Dictation Language.
    pub language: &'a DictationLanguage,
    /// The Vocabulary entries in list order. Engines whose Model accepts a text prompt use them
    /// as a hint (`vocabulary.md` rules 8–9); others ignore them, since spelling correction for
    /// those Models happens after the Engine returns.
    pub vocabulary: &'a [String],
}

/// Why the Engine could not produce a Transcript.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    /// The Model could not be loaded into memory (`models.md`).
    #[error("could not load the Model: {0}")]
    ModelLoad(String),
    /// Transcription itself failed; shown to the user as "Transcription failed" with this
    /// detail (`dictation-pipeline.md` rule 23).
    #[error("transcription failed: {0}")]
    Transcription(String),
}

/// Converts speech audio into a Transcript using one Model.
///
/// # Contract
///
/// - An Engine is bound to one Model; switching Models means a different Engine value.
/// - [`load`](Engine::load) brings the Model into memory. It is idempotent, may take seconds,
///   and is called when a Recording starts so loading overlaps with speaking (rule 6).
///   [`transcribe`](Engine::transcribe) loads on its own if `load` was never called.
/// - [`transcribe`](Engine::transcribe) is blocking and runs on a worker thread, never on the UI
///   thread. It returns the raw Engine text; clean-up (repeated words, whitespace, Vocabulary
///   correction) is applied by the caller (rule 25). An empty string means "no speech" and is
///   not an error.
/// - Cancellation does not interrupt an Engine: the caller discards a late result (rule 39).
/// - After an [`EngineError::Transcription`] the Engine stays usable; an implementation that
///   detects a crashed Model unloads it so the next call reloads it (rule 23).
pub trait Engine: Send {
    /// Loads the Model into memory if it is not loaded yet.
    fn load(&mut self) -> Result<(), EngineError>;

    /// Transcribes one Dictation's speech audio.
    fn transcribe(&mut self, request: TranscriptionRequest<'_>) -> Result<String, EngineError>;
}
