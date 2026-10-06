//! The library-independent half of the real Engine: loading with GPU → CPU fallback, unloading
//! after a crash, and turning a [`TranscriptionRequest`] into what the Model is asked to do.
//!
//! The Model library sits behind [`Loader`] / [`LoadedModel`], so all of this is tested with a
//! scripted fake instead of real weights.

use std::panic::{self, AssertUnwindSafe};

use super::{Acceleration, ComputeDevice, DictationLanguage, Engine, EngineError};
use super::{TranscriptionRequest, VOCABULARY_SEPARATOR};

/// How many prompt tokens a Whisper Model reads (`vocabulary.md` rule 9).
pub(super) const HINT_TOKEN_BUDGET: usize = 224;

/// Which devices one load attempt may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeviceRequest {
    /// The library's automatic choice: the best GPU, else the CPU.
    Auto,
    /// The CPU only.
    Cpu,
}

/// Why a load attempt failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LoadError {
    /// The Model file itself is missing or unusable; another device would not help.
    Model(String),
    /// The device could not take the Model (GPU driver, GPU memory, backend); worth retrying on
    /// the CPU.
    Device(String),
}

/// Why one transcription run failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunError {
    /// The Model's native state can no longer be trusted (`dictation-pipeline.md` rule 23).
    pub crashed: bool,
    pub detail: String,
}

/// The kind of Model, which decides how language and Vocabulary reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModelFamily {
    /// Honours a specific language and accepts a text prompt (`vocabulary.md` rule 8).
    Whisper,
    /// Always detects the language itself and takes no prompt (Parakeet).
    AutoDetectOnly,
}

/// What one run asks of a loaded Model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunRequest {
    /// ISO 639-1 code to transcribe in, or `None` to detect.
    pub language: Option<String>,
    /// The Vocabulary hint, already cut to the Model's budget.
    pub prompt: Option<String>,
}

/// Loads one Model file.
pub(super) trait Loader: Send {
    fn load(&self, device: DeviceRequest) -> Result<Box<dyn LoadedModel>, LoadError>;
}

/// A Model in memory.
pub(super) trait LoadedModel: Send {
    fn family(&self) -> ModelFamily;
    fn device(&self) -> ComputeDevice;
    /// The number of prompt tokens `text` takes, or `None` if the Model cannot tell.
    fn count_tokens(&self, text: &str) -> Option<usize>;
    fn run(&mut self, audio: &[f32], request: &RunRequest) -> Result<String, RunError>;
}

/// An [`Engine`] over any [`Loader`].
pub(super) struct ModelEngine<L> {
    loader: L,
    acceleration: Acceleration,
    loaded: Option<Box<dyn LoadedModel>>,
    /// The family of the Model file, known since its first load; unloading keeps it.
    family: Option<ModelFamily>,
}

impl<L: Loader> ModelEngine<L> {
    pub fn new(loader: L, acceleration: Acceleration) -> Self {
        Self {
            loader,
            acceleration,
            loaded: None,
            family: None,
        }
    }

    /// The device the loaded Model computes on, or `None` while it is not loaded.
    pub fn device(&self) -> Option<ComputeDevice> {
        self.loaded.as_ref().map(|model| model.device())
    }

    fn load_model(&self) -> Result<Box<dyn LoadedModel>, EngineError> {
        let first = match self.acceleration {
            Acceleration::Auto => DeviceRequest::Auto,
            Acceleration::CpuOnly => DeviceRequest::Cpu,
        };
        match self.loader.load(first) {
            Ok(model) => Ok(model),
            Err(LoadError::Device(gpu_detail)) if first == DeviceRequest::Auto => {
                self.loader.load(DeviceRequest::Cpu).map_err(|e| {
                    let cpu_detail = match e {
                        LoadError::Model(d) | LoadError::Device(d) => d,
                    };
                    EngineError::ModelLoad(format!("{cpu_detail} (GPU attempt: {gpu_detail})"))
                })
            }
            Err(LoadError::Model(detail) | LoadError::Device(detail)) => {
                Err(EngineError::ModelLoad(detail))
            }
        }
    }
}

impl<L: Loader> Engine for ModelEngine<L> {
    fn load(&mut self) -> Result<(), EngineError> {
        if self.loaded.is_none() {
            let model = self.load_model()?;
            self.family = Some(model.family());
            self.loaded = Some(model);
        }
        Ok(())
    }

    fn accepts_prompt(&self) -> bool {
        self.family != Some(ModelFamily::AutoDetectOnly)
    }

    fn transcribe(&mut self, request: TranscriptionRequest<'_>) -> Result<String, EngineError> {
        self.load()?;
        let model = self.loaded.as_mut().expect("loaded above");
        let run = run_request(model.family(), &request, |text| model.count_tokens(text));
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| model.run(request.audio, &run)));
        match outcome {
            Ok(Ok(text)) => Ok(text),
            Ok(Err(error)) => {
                if error.crashed {
                    self.unload();
                }
                Err(EngineError::Transcription(error.detail))
            }
            Err(panic) => {
                self.unload();
                let detail = panic
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".into());
                Err(EngineError::Transcription(format!(
                    "the Engine crashed: {detail}"
                )))
            }
        }
    }

    fn unload(&mut self) {
        self.loaded = None;
    }
}

/// Builds what a Model of `family` is asked to do for one Dictation
/// (`dictation-language.md` rule 6, `vocabulary.md` rules 8–10).
fn run_request(
    family: ModelFamily,
    request: &TranscriptionRequest<'_>,
    count_tokens: impl Fn(&str) -> Option<usize>,
) -> RunRequest {
    match family {
        ModelFamily::Whisper => RunRequest {
            language: match request.language {
                DictationLanguage::Automatic => None,
                DictationLanguage::Specific(code) => Some(code.clone()),
            },
            prompt: vocabulary_hint(request.vocabulary, count_tokens),
        },
        // Parakeet detects the language itself, and its Vocabulary spelling correction happens
        // after the Engine returns (`vocabulary.md` rule 10).
        ModelFamily::AutoDetectOnly => RunRequest {
            language: None,
            prompt: None,
        },
    }
}

/// The Vocabulary entries joined in list order with ", ", stopping before the first entry that
/// would exceed [`HINT_TOKEN_BUDGET`] — so the entries beyond the budget are the ones ignored
/// (`vocabulary.md` rule 9). Tokens are counted with the Model's tokenizer when it has one,
/// otherwise estimated pessimistically as one per three characters.
fn vocabulary_hint(
    entries: &[String],
    count_tokens: impl Fn(&str) -> Option<usize>,
) -> Option<String> {
    let mut hint = String::new();
    for entry in entries.iter().map(|e| e.trim()).filter(|e| !e.is_empty()) {
        let candidate = if hint.is_empty() {
            entry.to_string()
        } else {
            format!("{hint}{VOCABULARY_SEPARATOR}{entry}")
        };
        let tokens =
            count_tokens(&candidate).unwrap_or_else(|| candidate.chars().count().div_ceil(3));
        if tokens > HINT_TOKEN_BUDGET {
            break;
        }
        hint = candidate;
    }
    (!hint.is_empty()).then_some(hint)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// A scripted [`Loader`] that records every load attempt and every run.
    #[derive(Clone, Default)]
    struct ScriptedLoader {
        log: Arc<Mutex<Log>>,
    }

    #[derive(Default)]
    struct Log {
        /// Results of the next load attempts; when empty, loads succeed.
        load_results: Vec<Result<(), LoadError>>,
        /// Results of the next runs; when empty, runs return "text".
        run_results: Vec<Result<String, RunError>>,
        family: Option<ModelFamily>,
        loads: Vec<DeviceRequest>,
        runs: Vec<RunRequest>,
        panic_next_run: bool,
    }

    impl ScriptedLoader {
        fn log(&self) -> std::sync::MutexGuard<'_, Log> {
            self.log.lock().unwrap_or_else(|e| e.into_inner())
        }
        fn script_loads(&self, results: Vec<Result<(), LoadError>>) {
            self.log().load_results = results;
        }
        fn script_runs(&self, results: Vec<Result<String, RunError>>) {
            self.log().run_results = results;
        }
        fn loads(&self) -> Vec<DeviceRequest> {
            self.log().loads.clone()
        }
        fn runs(&self) -> Vec<RunRequest> {
            self.log().runs.clone()
        }
    }

    impl Loader for ScriptedLoader {
        fn load(&self, device: DeviceRequest) -> Result<Box<dyn LoadedModel>, LoadError> {
            let mut log = self.log();
            log.loads.push(device);
            if !log.load_results.is_empty() {
                log.load_results.remove(0)?;
            }
            Ok(Box::new(ScriptedModel {
                loader: self.clone(),
                device,
                family: log.family.unwrap_or(ModelFamily::Whisper),
            }))
        }
    }

    struct ScriptedModel {
        loader: ScriptedLoader,
        device: DeviceRequest,
        family: ModelFamily,
    }

    impl LoadedModel for ScriptedModel {
        fn family(&self) -> ModelFamily {
            self.family
        }
        fn device(&self) -> ComputeDevice {
            match self.device {
                DeviceRequest::Auto => ComputeDevice::Gpu("Test GPU".into()),
                DeviceRequest::Cpu => ComputeDevice::Cpu,
            }
        }
        fn count_tokens(&self, _text: &str) -> Option<usize> {
            None
        }
        fn run(&mut self, _audio: &[f32], request: &RunRequest) -> Result<String, RunError> {
            let mut log = self.loader.log();
            log.runs.push(request.clone());
            if std::mem::take(&mut log.panic_next_run) {
                drop(log);
                panic!("native fault");
            }
            if log.run_results.is_empty() {
                Ok("text".into())
            } else {
                log.run_results.remove(0)
            }
        }
    }

    fn engine(loader: &ScriptedLoader, acceleration: Acceleration) -> ModelEngine<ScriptedLoader> {
        ModelEngine::new(loader.clone(), acceleration)
    }

    fn transcribe(
        engine: &mut impl Engine,
        language: DictationLanguage,
        vocabulary: &[String],
    ) -> Result<String, EngineError> {
        engine.transcribe(TranscriptionRequest {
            audio: &[0.0; 16_000],
            language: &language,
            vocabulary,
        })
    }

    fn crash(detail: &str) -> RunError {
        RunError {
            crashed: true,
            detail: detail.into(),
        }
    }

    // --- loading -------------------------------------------------------------------------------

    #[test]
    fn load_is_idempotent() {
        let loader = ScriptedLoader::default();
        let mut engine = engine(&loader, Acceleration::Auto);

        engine.load().unwrap();
        engine.load().unwrap();

        assert_eq!(loader.loads(), vec![DeviceRequest::Auto]);
        assert_eq!(engine.device(), Some(ComputeDevice::Gpu("Test GPU".into())));
    }

    #[test]
    fn transcribe_loads_on_its_own() {
        let loader = ScriptedLoader::default();
        let mut engine = engine(&loader, Acceleration::Auto);
        assert_eq!(engine.device(), None);

        let text = transcribe(&mut engine, DictationLanguage::Automatic, &[]).unwrap();

        assert_eq!(text, "text");
        assert_eq!(loader.loads().len(), 1);
    }

    #[test]
    fn a_failing_gpu_falls_back_to_the_cpu() {
        let loader = ScriptedLoader::default();
        loader.script_loads(vec![Err(LoadError::Device("vk lost".into()))]);
        let mut engine = engine(&loader, Acceleration::Auto);

        engine.load().unwrap();

        assert_eq!(
            loader.loads(),
            vec![DeviceRequest::Auto, DeviceRequest::Cpu]
        );
        assert_eq!(engine.device(), Some(ComputeDevice::Cpu));
    }

    #[test]
    fn a_broken_model_file_is_not_retried_on_the_cpu() {
        let loader = ScriptedLoader::default();
        loader.script_loads(vec![Err(LoadError::Model("not a GGUF".into()))]);
        let mut engine = engine(&loader, Acceleration::Auto);

        let result = engine.load();

        assert_eq!(result, Err(EngineError::ModelLoad("not a GGUF".into())));
        assert_eq!(loader.loads(), vec![DeviceRequest::Auto]);
        assert_eq!(engine.device(), None);
    }

    #[test]
    fn when_the_cpu_also_fails_both_reasons_are_reported() {
        let loader = ScriptedLoader::default();
        loader.script_loads(vec![
            Err(LoadError::Device("vk lost".into())),
            Err(LoadError::Device("out of memory".into())),
        ]);
        let mut engine = engine(&loader, Acceleration::Auto);

        let result = engine.load();

        assert_eq!(
            result,
            Err(EngineError::ModelLoad(
                "out of memory (GPU attempt: vk lost)".into()
            ))
        );
    }

    #[test]
    fn cpu_only_never_asks_for_a_gpu() {
        let loader = ScriptedLoader::default();
        loader.script_loads(vec![Err(LoadError::Device("out of memory".into()))]);
        let mut engine = engine(&loader, Acceleration::CpuOnly);

        assert_eq!(
            engine.load(),
            Err(EngineError::ModelLoad("out of memory".into()))
        );
        assert_eq!(loader.loads(), vec![DeviceRequest::Cpu]);
    }

    #[test]
    fn a_failed_load_is_retried_by_the_next_call() {
        let loader = ScriptedLoader::default();
        loader.script_loads(vec![Err(LoadError::Model("locked".into()))]);
        let mut engine = engine(&loader, Acceleration::Auto);

        assert!(engine.load().is_err());
        assert_eq!(
            transcribe(&mut engine, DictationLanguage::Automatic, &[]),
            Ok("text".into())
        );
    }

    // --- unloading and crashes -----------------------------------------------------------------

    #[test]
    fn unload_frees_the_model_and_the_next_call_reloads_it() {
        let loader = ScriptedLoader::default();
        let mut engine = engine(&loader, Acceleration::Auto);
        engine.load().unwrap();

        engine.unload();
        assert_eq!(engine.device(), None);
        transcribe(&mut engine, DictationLanguage::Automatic, &[]).unwrap();

        assert_eq!(loader.loads().len(), 2);
    }

    #[test]
    fn a_crashed_model_is_unloaded_and_reloaded_by_the_next_dictation() {
        let loader = ScriptedLoader::default();
        loader.script_runs(vec![Err(crash("device lost"))]);
        let mut engine = engine(&loader, Acceleration::Auto);

        let failed = transcribe(&mut engine, DictationLanguage::Automatic, &[]);
        assert_eq!(
            failed,
            Err(EngineError::Transcription("device lost".into()))
        );
        assert_eq!(engine.device(), None, "the crashed Model is unloaded");

        let next = transcribe(&mut engine, DictationLanguage::Automatic, &[]);
        assert_eq!(next, Ok("text".into()));
        assert_eq!(loader.loads().len(), 2);
    }

    #[test]
    fn an_ordinary_failure_keeps_the_model_loaded() {
        let loader = ScriptedLoader::default();
        loader.script_runs(vec![Err(RunError {
            crashed: false,
            detail: "input too long".into(),
        })]);
        let mut engine = engine(&loader, Acceleration::Auto);

        let failed = transcribe(&mut engine, DictationLanguage::Automatic, &[]);

        assert_eq!(
            failed,
            Err(EngineError::Transcription("input too long".into()))
        );
        assert!(engine.device().is_some());
        transcribe(&mut engine, DictationLanguage::Automatic, &[]).unwrap();
        assert_eq!(loader.loads().len(), 1);
    }

    #[test]
    fn a_panic_during_a_run_counts_as_a_crash() {
        let loader = ScriptedLoader::default();
        loader.log().panic_next_run = true;
        let mut engine = engine(&loader, Acceleration::Auto);

        let failed = transcribe(&mut engine, DictationLanguage::Automatic, &[]);

        assert_eq!(
            failed,
            Err(EngineError::Transcription(
                "the Engine crashed: native fault".into()
            ))
        );
        assert_eq!(engine.device(), None);
        assert_eq!(
            transcribe(&mut engine, DictationLanguage::Automatic, &[]),
            Ok("text".into())
        );
    }

    // --- language and Vocabulary ---------------------------------------------------------------

    fn vocabulary(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|e| e.to_string()).collect()
    }

    #[test]
    fn whisper_receives_the_specific_language() {
        let loader = ScriptedLoader::default();
        let mut engine = engine(&loader, Acceleration::Auto);

        transcribe(&mut engine, DictationLanguage::Specific("pl".into()), &[]).unwrap();
        transcribe(&mut engine, DictationLanguage::Automatic, &[]).unwrap();

        let runs = loader.runs();
        assert_eq!(runs[0].language.as_deref(), Some("pl"));
        assert_eq!(runs[1].language, None, "Automatic means detection");
    }

    #[test]
    fn whisper_receives_the_vocabulary_as_a_hint_in_list_order() {
        let loader = ScriptedLoader::default();
        let mut engine = engine(&loader, Acceleration::Auto);

        transcribe(
            &mut engine,
            DictationLanguage::Automatic,
            &vocabulary(&["Echo", "GitHub", "Tauri"]),
        )
        .unwrap();
        transcribe(&mut engine, DictationLanguage::Automatic, &[]).unwrap();

        let runs = loader.runs();
        assert_eq!(runs[0].prompt.as_deref(), Some("Echo, GitHub, Tauri"));
        assert_eq!(runs[1].prompt, None, "no Vocabulary, no hint");
    }

    #[test]
    fn parakeet_always_detects_and_gets_no_hint() {
        let loader = ScriptedLoader::default();
        loader.log().family = Some(ModelFamily::AutoDetectOnly);
        let mut engine = engine(&loader, Acceleration::Auto);

        transcribe(
            &mut engine,
            DictationLanguage::Specific("pl".into()),
            &vocabulary(&["GitHub"]),
        )
        .unwrap();

        assert_eq!(
            loader.runs()[0],
            RunRequest {
                language: None,
                prompt: None
            }
        );
    }

    #[test]
    fn the_engine_reports_whether_its_model_accepts_a_prompt_once_loaded() {
        let whisper = ScriptedLoader::default();
        let mut whisper_engine = engine(&whisper, Acceleration::Auto);
        assert!(
            whisper_engine.accepts_prompt(),
            "unknown before loading: nothing is corrected"
        );
        whisper_engine.load().unwrap();
        assert!(whisper_engine.accepts_prompt());

        let parakeet = ScriptedLoader::default();
        parakeet.log().family = Some(ModelFamily::AutoDetectOnly);
        let mut parakeet_engine = engine(&parakeet, Acceleration::Auto);
        parakeet_engine.load().unwrap();
        assert!(!parakeet_engine.accepts_prompt());
        parakeet_engine.unload();
        assert!(
            !parakeet_engine.accepts_prompt(),
            "the Model file does not change when unloaded"
        );
    }

    #[test]
    fn the_hint_stops_at_the_token_budget_and_drops_the_later_entries() {
        // One token per character makes the budget easy to reason about.
        let per_char = |text: &str| Some(text.chars().count());
        let entries: Vec<String> = (0..10).map(|i| format!("{i}{}", "x".repeat(49))).collect();

        let hint = vocabulary_hint(&entries, per_char).unwrap();

        // 50 + 2 + 50 + 2 + 50 + 2 + 50 = 206 ≤ 224; a fifth entry would make 258.
        assert_eq!(hint.chars().count(), 206);
        assert!(hint.starts_with('0') && hint.contains(", 3x"));
        assert!(!hint.contains(", 4x"));
    }

    #[test]
    fn without_a_tokenizer_the_budget_is_estimated_as_three_characters_per_token() {
        let no_tokenizer = |_: &str| None;
        // 224 tokens × 3 = 672 characters fit; entries of 10 chars + ", " → 56 entries = 670.
        let entries: Vec<String> = (0..80).map(|i| format!("entry{i:05}")).collect();

        let hint = vocabulary_hint(&entries, no_tokenizer).unwrap();

        assert_eq!(hint.split(VOCABULARY_SEPARATOR).count(), 56);
    }

    #[test]
    fn blank_entries_are_skipped() {
        let hint = vocabulary_hint(&vocabulary(&["  ", "Echo", ""]), |_| None);
        assert_eq!(hint.as_deref(), Some("Echo"));
        assert_eq!(vocabulary_hint(&vocabulary(&[" "]), |_| None), None);
    }
}
