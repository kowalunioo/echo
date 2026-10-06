//! Developer check of the real Engine on a WAV file:
//! WAV → `WavAudioSource` → `SpeechConverter` → `TranscribeCppEngine`.
//!
//! ```text
//! cargo run --release --example transcribe -- <model.gguf> <file.wav> [lang|auto] [--cpu] [--vocab "Echo, GitHub"]
//! ```
//!
//! `lang` is an ISO 639-1 code such as `pl` or `en`; `auto` (the default) means automatic
//! detection. `--cpu` forces the CPU. `--vocab` takes comma-separated Vocabulary entries.
//! Prints the Transcript, the device, the load time and the real-time factor (transcription time
//! divided by audio duration; below 1.0 is faster than real time).
//!
//! The first GPU run after a driver install or update also compiles the Vulkan shaders (~10 s);
//! later runs use the driver's cache. In a deeply nested checkout (e.g. a git worktree) the
//! native Vulkan build can exceed Windows' path limit; set a short `CARGO_TARGET_DIR` then.

use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Instant;

use echo_lib::audio::{AudioEvent, AudioSource, Pace, SpeechConverter, WavAudioSource, WavOptions};
use echo_lib::engine::{
    Acceleration, DictationLanguage, ENGINE_SAMPLE_RATE, Engine, TranscribeCppEngine,
    TranscriptionRequest,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

struct Args {
    model: String,
    wav: String,
    language: DictationLanguage,
    acceleration: Acceleration,
    vocabulary: Vec<String>,
}

fn parse_args() -> Result<Args, String> {
    let usage = "usage: transcribe <model.gguf> <file.wav> [lang|auto] [--cpu] [--vocab \"a, b\"]";
    let mut positional = Vec::new();
    let mut acceleration = Acceleration::for_this_machine();
    let mut vocabulary = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cpu" => acceleration = Acceleration::CpuOnly,
            "--vocab" => {
                let list = args.next().ok_or(usage)?;
                vocabulary = list
                    .split(',')
                    .map(|e| e.trim().to_string())
                    .filter(|e| !e.is_empty())
                    .collect();
            }
            _ => positional.push(arg),
        }
    }
    let mut positional = positional.into_iter();
    let model = positional.next().ok_or(usage)?;
    let wav = positional.next().ok_or(usage)?;
    let language = match positional.next().as_deref() {
        None | Some("auto") => DictationLanguage::Automatic,
        Some(code) => DictationLanguage::Specific(code.to_string()),
    };
    Ok(Args {
        model,
        wav,
        language,
        acceleration,
        vocabulary,
    })
}

/// Plays the whole WAV file through the same conversion a Recording uses.
fn read_speech(path: &str) -> Result<Vec<f32>, String> {
    let options = WavOptions {
        pace: Pace::AsFastAsPossible,
        stop_at_end: true,
    };
    let mut source = WavAudioSource::open(path, options).map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::channel();
    let stream = source
        .start(Box::new(move |event| {
            let _ = tx.send(event);
        }))
        .map_err(|e| e.to_string())?;

    let mut converter: Option<SpeechConverter> = None;
    let mut speech = Vec::new();
    for event in rx {
        match event {
            AudioEvent::Frames(frames) => {
                if converter.is_none() {
                    converter =
                        Some(SpeechConverter::new(frames.format).map_err(|e| e.to_string())?);
                }
                let converter = converter.as_mut().expect("set above");
                speech.extend(converter.push(&frames.samples));
            }
            AudioEvent::Ended => break,
            AudioEvent::Failed(e) => return Err(e.to_string()),
        }
    }
    stream.stop();
    if let Some(converter) = converter {
        speech.extend(converter.finish());
    }
    Ok(speech)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let audio = read_speech(&args.wav)?;
    let audio_secs = audio.len() as f64 / f64::from(ENGINE_SAMPLE_RATE);

    let mut engine = TranscribeCppEngine::with_acceleration(&args.model, args.acceleration);
    let started = Instant::now();
    engine.load().map_err(|e| e.to_string())?;
    let load_secs = started.elapsed().as_secs_f64();

    let started = Instant::now();
    let transcript = engine
        .transcribe(TranscriptionRequest {
            audio: &audio,
            language: &args.language,
            vocabulary: &args.vocabulary,
        })
        .map_err(|e| e.to_string())?;
    let transcribe_secs = started.elapsed().as_secs_f64();

    let device = engine
        .device()
        .map_or_else(|| "unloaded".to_string(), |d| d.to_string());
    println!("transcript: {transcript}");
    println!("device:     {device}");
    println!("audio:      {audio_secs:.2} s");
    println!("load:       {load_secs:.2} s");
    println!(
        "transcribe: {transcribe_secs:.2} s (real-time factor {:.3})",
        transcribe_secs / audio_secs.max(f64::EPSILON)
    );
    Ok(())
}
