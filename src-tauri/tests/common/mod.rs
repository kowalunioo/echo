//! Helpers shared by the integration tests: synthetic WAV files, event collection and the
//! fixture acceptance tolerance.

#![allow(dead_code)] // each test binary uses a different subset

pub mod acceptance;

use std::io::Cursor;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use echo_lib::audio::{AudioEvent, AudioFormat, AudioSink, AudioSource, AudioStream};

/// Sample encoding of a generated WAV file.
#[derive(Clone, Copy)]
pub enum Encoding {
    Int16,
    Float32,
}

/// Generates an in-memory WAV file. `sample(frame, channel)` returns a value in `-1.0..=1.0`.
pub fn wav_bytes(
    format: AudioFormat,
    encoding: Encoding,
    frames: usize,
    sample: impl Fn(usize, usize) -> f32,
) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: format.channels,
        sample_rate: format.sample_rate,
        bits_per_sample: match encoding {
            Encoding::Int16 => 16,
            Encoding::Float32 => 32,
        },
        sample_format: match encoding {
            Encoding::Int16 => hound::SampleFormat::Int,
            Encoding::Float32 => hound::SampleFormat::Float,
        },
    };
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
        for frame in 0..frames {
            for channel in 0..format.channels as usize {
                let value = sample(frame, channel);
                match encoding {
                    Encoding::Int16 => writer
                        .write_sample((value * i16::MAX as f32).round() as i16)
                        .unwrap(),
                    Encoding::Float32 => writer.write_sample(value).unwrap(),
                }
            }
        }
        writer.finalize().unwrap();
    }
    bytes.into_inner()
}

/// A sine tone of `frequency` Hz with peak `amplitude`, as a function of the frame index.
pub fn tone(sample_rate: u32, frequency: f32, amplitude: f32) -> impl Fn(usize) -> f32 {
    move |frame| {
        amplitude
            * (2.0 * std::f32::consts::PI * frequency * frame as f32 / sample_rate as f32).sin()
    }
}

/// A sink that forwards every event into a channel.
pub fn channel_sink() -> (AudioSink, Receiver<AudioEvent>) {
    let (tx, rx) = mpsc::channel();
    let sink: AudioSink = Box::new(move |event| {
        let _ = tx.send(event);
    });
    (sink, rx)
}

/// Starts `source` and collects events until a terminal event arrives (or `timeout` passes).
pub fn record_until_end(
    source: &mut dyn AudioSource,
    timeout: Duration,
) -> (Vec<AudioEvent>, AudioStream) {
    let (sink, rx) = channel_sink();
    let stream = source.start(sink).expect("source starts");
    let mut events = Vec::new();
    while let Ok(event) = rx.recv_timeout(timeout) {
        let terminal = !matches!(event, AudioEvent::Frames(_));
        events.push(event);
        if terminal {
            break;
        }
    }
    (events, stream)
}

/// All interleaved samples of the `Frames` events, plus their (single) format.
pub fn samples_of(events: &[AudioEvent]) -> (Option<AudioFormat>, Vec<f32>) {
    let mut format = None;
    let mut samples = Vec::new();
    for event in events {
        if let AudioEvent::Frames(frames) = event {
            assert!(
                format.is_none_or(|f| f == frames.format),
                "format changed mid-stream"
            );
            format = Some(frames.format);
            samples.extend_from_slice(&frames.samples);
        }
    }
    (format, samples)
}
