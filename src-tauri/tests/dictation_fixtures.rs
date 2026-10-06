//! The whole Dictation pipeline — WAV Audio Source, earshot voice-activity detection, the real
//! Engine with Whisper large-v3-turbo, clean-up, History — with a fake Inserter, on the user's
//! fixture recordings (`dictation-pipeline.md` acceptance tests 1–2 and rule 50 without Notepad).
//!
//! Ignored by default (slow, needs the Model). Run with
//!
//! ```text
//! cargo test --release --test dictation_fixtures -- --ignored --nocapture
//! ```
//!
//! Models come from `ECHO_MODELS_DIR` (default `<repo>/.toolchain/models`), fixtures from
//! `ECHO_FIXTURES_DIR` (default `<repo>/tests/fixtures/audio`); anything missing is skipped.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use echo_lib::audio::{Pace, WavAudioSource, WavOptions};
use echo_lib::dictation::vad::Earshot;
use echo_lib::dictation::{
    Dictation, DictationContext, DictationDeps, DictationModel, DictationModels, DictationState,
    EngineModel,
};
use echo_lib::engine::{DictationLanguage, FakeEngine, TranscribeCppEngine};
use echo_lib::insertion::{FakeInserter, SharedInserter};
use echo_lib::models::NoActiveModel;
use echo_lib::shortcut::modes::RecordIntent;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

fn dir(var: &str, default: &[&str]) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| default.iter().fold(repo_root(), |p, part| p.join(part)))
}

fn existing(path: PathBuf, what: &str) -> Option<PathBuf> {
    if path.is_file() {
        Some(path)
    } else {
        eprintln!("skip: {what} missing: {}", path.display());
        None
    }
}

fn fixture(file: &str) -> Option<PathBuf> {
    existing(
        dir("ECHO_FIXTURES_DIR", &["tests", "fixtures", "audio"]).join(file),
        "fixture",
    )
}

struct Fixed(Arc<dyn DictationModel>);

impl DictationModels for Fixed {
    fn begin(&self) -> Result<Arc<dyn DictationModel>, NoActiveModel> {
        Ok(Arc::clone(&self.0))
    }
}

struct Outcome {
    inserted: Vec<String>,
    history: Vec<String>,
}

/// One Dictation of `wav` from press to Idle; the file's end stops the Recording.
fn dictate(model: Arc<dyn DictationModel>, wav: &Path, language: &str) -> Outcome {
    let source = WavAudioSource::open(
        wav,
        WavOptions {
            pace: Pace::AsFastAsPossible,
            stop_at_end: true,
        },
    )
    .expect("fixture decodes");
    let inserter = FakeInserter::new();
    let shared = SharedInserter::new();
    shared.set(inserter.clone());
    let history = Arc::new(Mutex::new(Vec::new()));
    let stored = Arc::clone(&history);
    let language = DictationLanguage::Specific(language.to_owned());
    let states = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&states);
    let dictation = Dictation::spawn(DictationDeps {
        models: Box::new(Fixed(model)),
        source: Box::new(source),
        detector: Box::new(|| Box::new(Earshot::new())),
        context: Box::new(move || DictationContext {
            language: language.clone(),
            vocabulary: Vec::new(),
        }),
        history: Box::new(move |entry| {
            stored.lock().unwrap().push(entry.text);
            Ok(())
        }),
        inserter: shared,
        shortcut_reset: Box::new(|| {}),
        publish: Box::new(move |status| seen.lock().unwrap().push(status.state)),
        max_recording: Duration::from_secs(600),
    });
    dictation.intent(RecordIntent::Start);
    let until = Instant::now() + Duration::from_secs(300);
    loop {
        let states = states.lock().unwrap().clone();
        if states.len() > 1 && dictation.status().state == DictationState::Idle {
            break;
        }
        assert!(Instant::now() < until, "the Dictation did not finish");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(dictation.status().error, None, "no error is indicated");
    let history = history.lock().unwrap().clone();
    Outcome {
        inserted: inserter.inserted(),
        history,
    }
}

/// Acceptance test 2 with the synthetic Engine: earshot drops `cisza.wav`'s room tone, so the
/// Engine is never called. Needs only the fixture, not the Model.
#[test]
#[ignore = "needs the user's fixture recordings"]
fn cisza_never_reaches_the_engine() {
    let Some(wav) = fixture("cisza.wav") else {
        return;
    };
    let engine = FakeEngine::returning("Dziękuję.");
    let model: Arc<dyn DictationModel> = Arc::new(EngineModel::new("Fake", engine.clone()));
    let outcome = dictate(model, &wav, "pl");
    assert!(engine.requests().is_empty(), "the Engine was called");
    assert!(outcome.inserted.is_empty());
    assert!(outcome.history.is_empty());
}

/// `cisza.wav` and the speech fixtures through the real Engine: silence stays empty, speech is
/// inserted and stored. Prints each Transcript for the WER report.
#[test]
#[ignore = "slow; needs the Model and the user's fixture recordings"]
fn fixtures_through_the_real_engine() {
    let models = dir("ECHO_MODELS_DIR", &[".toolchain", "models"]);
    let Some(file) = existing(
        models.join("whisper-large-v3-turbo-Q8_0.gguf"),
        "Model whisper-large-v3-turbo",
    ) else {
        return;
    };
    let model: Arc<dyn DictationModel> = Arc::new(EngineModel::new(
        "Whisper large-v3-turbo",
        TranscribeCppEngine::new(file),
    ));

    if let Some(wav) = fixture("cisza.wav") {
        let outcome = dictate(Arc::clone(&model), &wav, "pl");
        eprintln!("cisza.wav -> {:?}", outcome.inserted);
        assert!(outcome.inserted.is_empty(), "cisza.wav inserted text");
        assert!(outcome.history.is_empty(), "cisza.wav reached History");
    }
    for (file, language) in [
        ("pl-proste.wav", "pl"),
        ("pl-interpunkcja.wav", "pl"),
        ("pl-liczby.wav", "pl"),
        ("pl-slownik.wav", "pl"),
        ("en-proste.wav", "en"),
    ] {
        let Some(wav) = fixture(file) else {
            continue;
        };
        let outcome = dictate(Arc::clone(&model), &wav, language);
        eprintln!("{file} -> {:?}", outcome.inserted);
        assert_eq!(outcome.inserted.len(), 1, "{file}: one Insertion");
        assert_eq!(outcome.history, outcome.inserted, "{file}: History first");
        assert!(!outcome.inserted[0].is_empty());
    }
}
