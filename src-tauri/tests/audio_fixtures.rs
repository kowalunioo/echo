//! The user's voice recordings in `tests/fixtures/audio/` (gitignored, absent in CI) go through
//! the WAV Audio Source and conversion like any other file. Each test skips with a "fixture
//! missing" message when its file is absent (`dictation-pipeline.md` rule 47).

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::{record_until_end, samples_of};
use echo_lib::audio::{Pace, SpeechConverter, TARGET_SAMPLE_RATE, WavAudioSource, WavOptions};

const FIXTURES: [&str; 6] = [
    "pl-proste.wav",
    "pl-interpunkcja.wav",
    "pl-slownik.wav",
    "pl-liczby.wav",
    "en-proste.wav",
    "cisza.wav",
];

fn fixture(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/audio")
        .join(name);
    if path.is_file() {
        Some(path)
    } else {
        eprintln!("fixture missing: {} — skipping", path.display());
        None
    }
}

#[test]
fn fixtures_convert_to_16_khz_mono_of_the_same_duration() {
    for name in FIXTURES {
        let Some(path) = fixture(name) else { continue };
        let options = WavOptions {
            pace: Pace::AsFastAsPossible,
            stop_at_end: true,
        };
        let mut source = WavAudioSource::open(&path, options).unwrap();
        let format = source.format();

        let (events, _stream) = record_until_end(&mut source, Duration::from_secs(30));
        let (_, samples) = samples_of(&events);
        let mut converter = SpeechConverter::new(format).unwrap();
        let mut out = converter.push(&samples);
        out.extend(converter.finish());

        let seconds_in = samples.len() as f64 / format.channels as f64 / format.sample_rate as f64;
        let seconds_out = out.len() as f64 / TARGET_SAMPLE_RATE as f64;
        assert!((seconds_in - seconds_out).abs() <= 0.01, "{name}");
        assert!(
            out.iter().all(|s| s.is_finite() && s.abs() <= 1.5),
            "{name}"
        );
    }
}
