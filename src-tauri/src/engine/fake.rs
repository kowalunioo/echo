use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use super::{DictationLanguage, Engine, EngineError, TranscriptionRequest};

/// A copy of one [`TranscriptionRequest`] the [`FakeEngine`] received.
#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedRequest {
    pub audio: Vec<f32>,
    pub language: DictationLanguage,
    pub vocabulary: Vec<String>,
}

/// An [`Engine`] with a scripted result, an optional delay (a "slow Engine") and a record of
/// every request.
///
/// Clones share state, so a test keeps one clone to script and inspect the fake while the code
/// under test owns another.
#[derive(Debug, Clone)]
pub struct FakeEngine {
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    outcome: Result<String, EngineError>,
    load_outcome: Result<(), EngineError>,
    delay: Duration,
    load_delay: Duration,
    loaded: bool,
    loads: usize,
    unloads: usize,
    requests: Vec<ReceivedRequest>,
}

impl FakeEngine {
    /// An Engine that returns `transcript` for every request.
    pub fn returning(transcript: impl Into<String>) -> Self {
        Self::with_outcome(Ok(transcript.into()))
    }

    /// An Engine that fails every request with `error`.
    pub fn failing(error: EngineError) -> Self {
        Self::with_outcome(Err(error))
    }

    /// Makes every `transcribe` call take `delay` before it returns.
    pub fn with_delay(self, delay: Duration) -> Self {
        self.state().delay = delay;
        self
    }

    /// Makes every `load` call take `delay` (a slow Model load).
    pub fn with_load_delay(self, delay: Duration) -> Self {
        self.state().load_delay = delay;
        self
    }

    /// Changes the result of later `load` calls (e.g. a Model that cannot be loaded).
    pub fn set_load_outcome(&self, outcome: Result<(), EngineError>) {
        self.state().load_outcome = outcome;
    }

    /// Whether the Model is loaded: a successful `load` (or `transcribe`) and no `unload` since.
    pub fn is_loaded(&self) -> bool {
        self.state().loaded
    }

    /// Changes the result of later requests.
    pub fn set_outcome(&self, outcome: Result<String, EngineError>) {
        self.state().outcome = outcome;
    }

    /// Every request received so far, oldest first.
    pub fn requests(&self) -> Vec<ReceivedRequest> {
        self.state().requests.clone()
    }

    /// How many times [`Engine::load`] was called.
    pub fn load_count(&self) -> usize {
        self.state().loads
    }

    /// How many times [`Engine::unload`] was called.
    pub fn unload_count(&self) -> usize {
        self.state().unloads
    }

    fn with_outcome(outcome: Result<String, EngineError>) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                outcome,
                load_outcome: Ok(()),
                delay: Duration::ZERO,
                load_delay: Duration::ZERO,
                loaded: false,
                loads: 0,
                unloads: 0,
                requests: Vec::new(),
            })),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // A panicking test thread must not hide the fake's record from the others.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Engine for FakeEngine {
    fn load(&mut self) -> Result<(), EngineError> {
        let delay = {
            let mut state = self.state();
            state.loads += 1;
            state.load_delay
        };
        if !delay.is_zero() {
            thread::sleep(delay);
        }
        let mut state = self.state();
        let outcome = state.load_outcome.clone();
        if outcome.is_ok() {
            state.loaded = true;
        }
        outcome
    }

    fn transcribe(&mut self, request: TranscriptionRequest<'_>) -> Result<String, EngineError> {
        let delay = {
            let mut state = self.state();
            state.requests.push(ReceivedRequest {
                audio: request.audio.to_vec(),
                language: request.language.clone(),
                vocabulary: request.vocabulary.to_vec(),
            });
            state.loaded = true;
            state.delay
        };
        if !delay.is_zero() {
            thread::sleep(delay);
        }
        self.state().outcome.clone()
    }

    fn unload(&mut self) {
        let mut state = self.state();
        state.unloads += 1;
        state.loaded = false;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn request<'a>(
        audio: &'a [f32],
        language: &'a DictationLanguage,
        vocabulary: &'a [String],
    ) -> TranscriptionRequest<'a> {
        TranscriptionRequest {
            audio,
            language,
            vocabulary,
        }
    }

    #[test]
    fn returns_the_scripted_transcript_and_records_the_request() {
        let probe = FakeEngine::returning("dzień dobry");
        let mut engine: Box<dyn Engine> = Box::new(probe.clone());
        let language = DictationLanguage::Specific("pl".into());
        let vocabulary = vec!["Echo".to_string(), "Tauri".to_string()];

        let text = engine
            .transcribe(request(&[0.0; 20_000], &language, &vocabulary))
            .unwrap();

        assert_eq!(text, "dzień dobry");
        let received = probe.requests();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].audio.len(), 20_000);
        assert_eq!(received[0].language, language);
        assert_eq!(received[0].vocabulary, vocabulary);
    }

    #[test]
    fn a_failing_engine_returns_its_error() {
        let error = EngineError::Transcription("out of memory".into());
        let mut engine = FakeEngine::failing(error.clone());

        let result = engine.transcribe(request(&[], &DictationLanguage::Automatic, &[]));

        assert_eq!(result, Err(error));
        assert_eq!(
            EngineError::Transcription("x".into()).to_string(),
            "transcription failed: x"
        );
    }

    #[test]
    fn a_slow_engine_takes_its_delay() {
        let mut engine = FakeEngine::returning("").with_delay(Duration::from_millis(50));
        let started = Instant::now();

        engine
            .transcribe(request(&[], &DictationLanguage::Automatic, &[]))
            .unwrap();

        assert!(started.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn counts_loads_and_unloads_and_follows_outcome_changes() {
        let probe = FakeEngine::returning("a");
        let mut engine = probe.clone();
        engine.load().unwrap();
        engine.unload();
        probe.set_outcome(Ok("b".into()));

        let text = engine
            .transcribe(request(&[], &DictationLanguage::Automatic, &[]))
            .unwrap();

        assert_eq!(probe.load_count(), 1);
        assert_eq!(probe.unload_count(), 1);
        assert_eq!(text, "b");
    }

    #[test]
    fn a_scripted_load_failure_leaves_the_model_unloaded() {
        let probe = FakeEngine::returning("a");
        let mut engine = probe.clone();
        probe.set_load_outcome(Err(EngineError::ModelLoad("bad file".into())));

        assert_eq!(
            engine.load(),
            Err(EngineError::ModelLoad("bad file".into()))
        );
        assert!(!probe.is_loaded());

        probe.set_load_outcome(Ok(()));
        engine.load().unwrap();
        assert!(probe.is_loaded());
        engine.unload();
        assert!(!probe.is_loaded());
    }
}
