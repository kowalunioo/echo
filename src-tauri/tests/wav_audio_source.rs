//! Behaviour of the WAV Audio Source (`docs/specs/dictation-pipeline.md` rules 42–45).

mod common;

use std::io::Cursor;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{Encoding, channel_sink, record_until_end, samples_of, wav_bytes};
use echo_lib::audio::{
    AudioEvent, AudioFormat, AudioSource, AudioSourceError, Pace, WavAudioSource, WavOptions,
};

const FAST_TO_END: WavOptions = WavOptions {
    pace: Pace::AsFastAsPossible,
    stop_at_end: true,
};

fn ramp_wav(format: AudioFormat, encoding: Encoding, frames: usize) -> Vec<u8> {
    // Distinct, exactly representable values per frame and channel.
    wav_bytes(format, encoding, frames, |frame, channel| {
        ((frame % 64) as f32 / 128.0) * if channel == 0 { 1.0 } else { -1.0 }
    })
}

#[test]
fn delivers_16_bit_integer_samples_normalised_and_interleaved() {
    let format = AudioFormat {
        sample_rate: 16_000,
        channels: 2,
    };
    let bytes = ramp_wav(format, Encoding::Int16, 100);
    let mut source = WavAudioSource::from_reader(Cursor::new(bytes), FAST_TO_END).unwrap();

    let (events, _stream) = record_until_end(&mut source, Duration::from_secs(5));

    assert_eq!(events.last(), Some(&AudioEvent::Ended));
    let (got_format, samples) = samples_of(&events);
    assert_eq!(got_format, Some(format));
    assert_eq!(samples.len(), 200);
    assert!((samples[2 * 10] - 10.0 / 128.0).abs() < 1e-4);
    assert!((samples[2 * 10 + 1] + 10.0 / 128.0).abs() < 1e-4);
}

#[test]
fn delivers_32_bit_float_samples_unchanged() {
    let format = AudioFormat {
        sample_rate: 44_100,
        channels: 1,
    };
    let bytes = ramp_wav(format, Encoding::Float32, 441);
    let mut source = WavAudioSource::from_reader(Cursor::new(bytes), FAST_TO_END).unwrap();

    let (events, _stream) = record_until_end(&mut source, Duration::from_secs(5));

    let (got_format, samples) = samples_of(&events);
    assert_eq!(got_format, Some(format));
    assert_eq!(samples.len(), 441);
    assert_eq!(samples[63], 63.0 / 128.0);
}

#[test]
fn every_recording_starts_from_the_beginning_of_the_file() {
    let format = AudioFormat {
        sample_rate: 8_000,
        channels: 1,
    };
    let bytes = ramp_wav(format, Encoding::Float32, 800);
    let mut source = WavAudioSource::from_reader(Cursor::new(bytes), FAST_TO_END).unwrap();

    let (first, stream) = record_until_end(&mut source, Duration::from_secs(5));
    stream.stop();
    let (second, _stream) = record_until_end(&mut source, Duration::from_secs(5));

    assert_eq!(samples_of(&first), samples_of(&second));
}

#[test]
fn real_time_pace_takes_as_long_as_the_audio() {
    let format = AudioFormat {
        sample_rate: 16_000,
        channels: 1,
    };
    let bytes = ramp_wav(format, Encoding::Int16, 4_800); // 300 ms
    let options = WavOptions {
        pace: Pace::RealTime,
        stop_at_end: true,
    };
    let mut source = WavAudioSource::from_reader(Cursor::new(bytes), options).unwrap();

    let started = Instant::now();
    let (events, _stream) = record_until_end(&mut source, Duration::from_secs(5));
    let elapsed = started.elapsed();

    assert_eq!(events.last(), Some(&AudioEvent::Ended));
    assert_eq!(samples_of(&events).1.len(), 4_800);
    assert!(elapsed >= Duration::from_millis(270), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(1_500), "{elapsed:?}");
}

#[test]
fn without_stop_at_end_silence_follows_the_file_until_stopped() {
    let format = AudioFormat {
        sample_rate: 16_000,
        channels: 1,
    };
    let bytes = wav_bytes(format, Encoding::Float32, 1_600, |_, _| 0.5); // 100 ms
    let options = WavOptions {
        pace: Pace::RealTime,
        stop_at_end: false,
    };
    let mut source = WavAudioSource::from_reader(Cursor::new(bytes), options).unwrap();
    let (sink, rx) = channel_sink();

    let stream = source.start(sink).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    stream.stop();

    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        events.iter().all(|e| matches!(e, AudioEvent::Frames(_))),
        "no terminal event without stop_at_end"
    );
    let (_, samples) = samples_of(&events);
    assert!(samples.len() >= 4_800, "only {} samples", samples.len());
    assert!(samples[..1_600].iter().all(|&s| s == 0.5));
    assert!(samples[1_600..].iter().all(|&s| s == 0.0));
}

#[test]
fn as_fast_as_possible_without_stop_at_end_still_paces_the_trailing_silence() {
    let format = AudioFormat {
        sample_rate: 16_000,
        channels: 1,
    };
    let bytes = wav_bytes(format, Encoding::Float32, 16_000, |_, _| 0.25); // 1 s
    let options = WavOptions {
        pace: Pace::AsFastAsPossible,
        stop_at_end: false,
    };
    let mut source = WavAudioSource::from_reader(Cursor::new(bytes), options).unwrap();
    let (sink, rx) = channel_sink();

    let stream = source.start(sink).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    stream.stop();

    let (_, samples) = samples_of(&rx.try_iter().collect::<Vec<_>>());
    assert!(samples.len() >= 16_000, "file delivered at once");
    assert!(samples.len() < 16_000 + 16_000, "silence was not flooded");
}

#[test]
fn no_events_arrive_after_stop_returns() {
    let format = AudioFormat {
        sample_rate: 48_000,
        channels: 2,
    };
    let bytes = wav_bytes(format, Encoding::Int16, 48_000, |_, _| 0.1);
    let mut source =
        WavAudioSource::from_reader(Cursor::new(bytes), WavOptions::default()).unwrap();
    let (sink, rx) = channel_sink();

    let stream = source.start(sink).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    stream.stop();
    let _ = rx.try_iter().count();

    // The sink was dropped with the stream, so the channel is disconnected and empty.
    assert!(matches!(
        rx.recv_timeout(Duration::from_millis(100)),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn dropping_the_stream_stops_the_recording() {
    let format = AudioFormat {
        sample_rate: 16_000,
        channels: 1,
    };
    let bytes = wav_bytes(format, Encoding::Int16, 16_000, |_, _| 0.1);
    let mut source =
        WavAudioSource::from_reader(Cursor::new(bytes), WavOptions::default()).unwrap();
    let (sink, rx) = channel_sink();

    drop(source.start(sink).unwrap());

    let _ = rx.try_iter().count();
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
}

#[test]
fn a_missing_file_fails_to_open_with_a_detail() {
    let result = WavAudioSource::open(Path::new("does/not/exist.wav"), WavOptions::default());
    match result {
        Err(AudioSourceError::Failed(detail)) => assert!(detail.contains("exist.wav"), "{detail}"),
        other => panic!("expected Failed, got {:?}", other.err()),
    }
}

#[test]
fn a_file_that_is_not_wav_fails_to_open() {
    let result = WavAudioSource::from_reader(Cursor::new(b"not a wav".to_vec()), FAST_TO_END);
    assert!(matches!(result, Err(AudioSourceError::Failed(_))));
}
