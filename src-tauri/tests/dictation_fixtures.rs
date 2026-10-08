//! The whole Dictation pipeline — WAV Audio Source, earshot voice-activity detection, the real
//! Engine with Whisper large-v3-turbo, clean-up, History — with a fake Inserter, on the user's
//! fixture recordings (`dictation-pipeline.md` acceptance tests 1, 2 and 17 without Notepad;
//! rules 47–52 with the tolerance from `common::acceptance`).
//!
//! Ignored by default (slow, needs the Model). Run with
//!
//! ```text
//! cargo test --release --test dictation_fixtures -- --ignored --nocapture --test-threads 1
//! ```
//!
//! Models come from `ECHO_MODELS_DIR` (default `<repo>/.toolchain/models`), fixtures from
//! `ECHO_FIXTURES_DIR` (default `<repo>/tests/fixtures/audio`); anything missing is skipped.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use echo_lib::audio::{Pace, WavAudioSource, WavOptions};
use echo_lib::dictation::language::{DictationLanguageSetting, effective_language};
use echo_lib::dictation::vad::Earshot;
use echo_lib::dictation::{
    Dictation, DictationContext, DictationDeps, DictationModel, DictationModels, DictationState,
    EngineModel,
};
use echo_lib::engine::{DictationLanguage, FakeEngine, TranscribeCppEngine};
use echo_lib::insertion::{FakeInserter, SharedInserter};
use echo_lib::models::{ModelId, NoActiveModel};
use echo_lib::shortcut::modes::RecordIntent;
use echo_lib::vocabulary::Vocabulary;

use common::acceptance;

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

/// One Dictation of `wav` from press to Idle in `language` without Vocabulary.
fn dictate(model: Arc<dyn DictationModel>, wav: &Path, language: &str) -> Outcome {
    dictate_with(
        model,
        wav,
        DictationContext {
            language: DictationLanguage::Specific(language.to_owned()),
            vocabulary: Vec::new(),
        },
    )
}

/// One Dictation of `wav` from press to Idle; the file's end stops the Recording.
fn dictate_with(model: Arc<dyn DictationModel>, wav: &Path, context: DictationContext) -> Outcome {
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
    let states = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&states);
    let dictation = Dictation::spawn(DictationDeps {
        models: Box::new(Fixed(model)),
        source: Box::new(source),
        detector: Box::new(|| Box::new(Earshot::new())),
        context: Box::new(move || context.clone()),
        history: Box::new(move |entry| {
            stored.lock().unwrap().push(entry.text);
            Ok(true)
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

/// The default Model, as the user selects it for the threshold table (rule 50).
const DEFAULT_MODEL: ModelId = ModelId::WhisperLargeV3Turbo;
/// The other Models of `models.md`: `cisza.wav` stays mandatory, WER is reported (rule 52).
const OTHER_MODELS: [ModelId; 2] = [ModelId::WhisperSmall, ModelId::ParakeetTdt06bV3];

/// A real Model as a run uses it: its id decides the effective Dictation Language.
struct RealModel {
    id: ModelId,
    model: Arc<dyn DictationModel>,
}

fn real_model(id: ModelId) -> Option<RealModel> {
    let info = id.info();
    let models = dir("ECHO_MODELS_DIR", &[".toolchain", "models"]);
    let path = existing(models.join(info.file_name), &format!("Model {}", info.name))?;
    Some(RealModel {
        id,
        model: Arc::new(EngineModel::new(info.name, TranscribeCppEngine::new(path))),
    })
}

/// Which Dictation Language a run uses.
#[derive(Clone, Copy, PartialEq)]
enum Language {
    /// The row's language from the table (rule 50).
    Table,
    /// Automatic detection (rule 51).
    Automatic,
}

/// Runs every fixture of the rule 50 table that is present through `model` and returns one
/// report line per fixture plus the failures among the rows `mandatory` selects.
fn run_table(
    model: &RealModel,
    language: Language,
    mandatory: impl Fn(&acceptance::Fixture) -> bool,
) -> (Vec<String>, Vec<String>) {
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for row in acceptance::FIXTURES {
        let Some(wav) = fixture(row.file) else {
            continue;
        };
        // The setting path of the app: the intent, resolved against the Model (rule 4).
        let intent = match language {
            Language::Table => DictationLanguageSetting::try_from(row.language.to_owned())
                .expect("the table's language is one a Model supports"),
            Language::Automatic => DictationLanguageSetting::automatic(),
        };
        let context = DictationContext {
            language: effective_language(&intent, Some(model.id)),
            // The Vocabulary setting as the app stores it (`vocabulary.md` rules 1-5).
            vocabulary: Vocabulary::try_from(
                row.vocabulary
                    .iter()
                    .map(|t| (*t).to_owned())
                    .collect::<Vec<_>>(),
            )
            .expect("the table's Vocabulary is a valid setting")
            .entries()
            .to_vec(),
        };
        let outcome = dictate_with(Arc::clone(&model.model), &wav, context);
        let actual = outcome.inserted.join(" ");
        let verdict = match row.accepts(&actual) {
            Ok(()) => "ok".to_owned(),
            Err(why) => why,
        };
        let wer = if row.expected.is_empty() {
            String::from("   -  ")
        } else {
            format!("{:.3}", acceptance::wer(row.expected, &actual))
        };
        report.push(format!(
            "{:<22} WER {wer}  {actual:?}  [{verdict}]",
            row.file
        ));

        let mut problems = Vec::new();
        if verdict != "ok" {
            problems.push(verdict);
        }
        // History gets exactly what was inserted, and only non-empty Transcripts (rules 20, 38).
        if outcome.history != outcome.inserted {
            problems.push(format!(
                "{}: History {:?} differs from the Insertions",
                row.file, outcome.history
            ));
        }
        if outcome.inserted.len() > 1 {
            problems.push(format!(
                "{}: {} Insertions",
                row.file,
                outcome.inserted.len()
            ));
        }
        if mandatory(row) {
            failures.extend(problems);
        }
    }
    (report, failures)
}

fn print_report(title: &str, report: &[String]) {
    eprintln!("\n== {title}");
    for line in report {
        eprintln!("   {line}");
    }
}

/// Acceptance test 17 without Notepad, and rule 50: every fixture through the whole pipeline
/// with the default Model and the table's Dictation Language and Vocabulary meets its threshold;
/// `cisza.wav` yields no Insertion and no History entry. Insertion into Notepad is checked by hand
/// in fake-microphone mode (see the pull request of #37).
#[test]
#[ignore = "slow; needs the Model and the user's fixture recordings"]
fn default_model_meets_the_fixture_thresholds() {
    let Some(model) = real_model(DEFAULT_MODEL) else {
        return;
    };
    let (report, failures) = run_table(&model, Language::Table, |_| true);
    print_report(
        "Whisper large-v3-turbo, table language (mandatory)",
        &report,
    );
    assert!(
        failures.is_empty(),
        "rule 50 failures:\n{}",
        failures.join("\n")
    );
}

/// Rule 51: the same fixtures with automatic language detection; reported, not mandatory.
#[test]
#[ignore = "slow; needs the Model and the user's fixture recordings"]
fn default_model_with_automatic_language_is_reported() {
    let Some(model) = real_model(DEFAULT_MODEL) else {
        return;
    };
    let (report, _) = run_table(&model, Language::Automatic, |_| false);
    print_report(
        "Whisper large-v3-turbo, automatic language (reported)",
        &report,
    );
}

/// Rule 52: the other Models must keep `cisza.wav` empty; their WER rows are reported only.
#[test]
#[ignore = "slow; needs the Models and the user's fixture recordings"]
fn other_models_keep_silence_empty() {
    let mut failures = Vec::new();
    for spec in OTHER_MODELS {
        let Some(model) = real_model(spec) else {
            continue;
        };
        let (report, failed) = run_table(&model, Language::Table, |row| row.max_wer.is_none());
        print_report(
            &format!("{}, table language (cisza mandatory)", spec.info().name),
            &report,
        );
        failures.extend(
            failed
                .into_iter()
                .map(|f| format!("{}: {f}", spec.info().name)),
        );
    }
    assert!(
        failures.is_empty(),
        "rule 52 failures:\n{}",
        failures.join("\n")
    );
}
