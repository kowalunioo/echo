//! The real Engine with real Model files and the user's fixture recordings.
//!
//! Ignored by default (slow, needs ~2 GB of Models). Run with
//!
//! ```text
//! cargo test --release --test engine_models -- --ignored --nocapture
//! ```
//!
//! Model files are looked up in `ECHO_MODELS_DIR` (default `<repo>/.toolchain/models`), fixtures
//! in `ECHO_FIXTURES_DIR` (default `<repo>/tests/fixtures/audio`). Anything missing is skipped with a message, so the test
//! passes on a machine without them.

use std::path::{Path, PathBuf};

use echo_lib::audio::{AudioEvent, AudioSource, Pace};
use echo_lib::audio::{AudioFormat, SpeechConverter, WavAudioSource, WavOptions};
use echo_lib::engine::{DictationLanguage, Engine, TranscribeCppEngine, TranscriptionRequest};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

fn model(file: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("ECHO_MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join(".toolchain").join("models"));
    let path = dir.join(file);
    if path.is_file() {
        Some(path)
    } else {
        eprintln!("skip: Model missing: {}", path.display());
        None
    }
}

fn fixture(file: &str) -> Option<Vec<f32>> {
    let dir = std::env::var_os("ECHO_FIXTURES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("tests").join("fixtures").join("audio"));
    let path = dir.join(file);
    if !path.is_file() {
        eprintln!("skip: fixture missing: {}", path.display());
        return None;
    }
    let options = WavOptions {
        pace: Pace::AsFastAsPossible,
        stop_at_end: true,
    };
    let mut source = WavAudioSource::open(&path, options).expect("fixture decodes");
    let (tx, rx) = std::sync::mpsc::channel();
    let stream = source
        .start(Box::new(move |event| {
            let _ = tx.send(event);
        }))
        .expect("WAV source starts");
    let mut converter: Option<SpeechConverter> = None;
    let mut speech = Vec::new();
    for event in rx {
        match event {
            AudioEvent::Frames(frames) => {
                let format: AudioFormat = frames.format;
                let converter =
                    converter.get_or_insert_with(|| SpeechConverter::new(format).unwrap());
                speech.extend(converter.push(&frames.samples));
            }
            AudioEvent::Ended => break,
            AudioEvent::Failed(e) => panic!("WAV source failed: {e}"),
        }
    }
    stream.stop();
    speech.extend(converter.expect("audio arrived").finish());
    Some(speech)
}

fn transcribe(engine: &mut impl Engine, audio: &[f32], language: &str) -> String {
    let language = DictationLanguage::Specific(language.into());
    engine
        .transcribe(TranscriptionRequest {
            audio,
            language: &language,
            vocabulary: &[],
        })
        .expect("transcription succeeds")
}

#[test]
#[ignore = "needs Model files and fixtures; run with --ignored"]
fn every_model_transcribes_the_english_and_polish_fixtures() {
    let (Some(english), Some(polish)) = (fixture("en-proste.wav"), fixture("pl-proste.wav")) else {
        return;
    };
    for file in [
        "whisper-small-Q8_0.gguf",
        "whisper-large-v3-turbo-Q8_0.gguf",
        "parakeet-tdt-0.6b-v3-Q8_0.gguf",
    ] {
        let Some(path) = model(file) else { continue };
        let mut engine = TranscribeCppEngine::new(path);

        let en = transcribe(&mut engine, &english, "en").to_lowercase();
        let pl = transcribe(&mut engine, &polish, "pl").to_lowercase();
        eprintln!("{file} on {:?}: {en:?} / {pl:?}", engine.device());

        assert!(en.contains("dictation test"), "{file}: {en}");
        assert!(pl.contains("dzień dobry"), "{file}: {pl}");
    }
}

#[test]
#[ignore = "needs a Model file; run with --ignored"]
fn unload_and_reload_keep_the_engine_usable() {
    let (Some(path), Some(english)) = (model("whisper-small-Q8_0.gguf"), fixture("en-proste.wav"))
    else {
        return;
    };
    let mut engine = TranscribeCppEngine::new(path);
    engine.load().unwrap();
    assert!(engine.device().is_some());

    engine.unload();
    assert!(engine.device().is_none());

    let text = transcribe(&mut engine, &english, "en");
    assert!(text.to_lowercase().contains("english"), "{text}");
}
