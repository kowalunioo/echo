//! Acceptance tests 5 and 6 of `docs/specs/dictation-pipeline.md`: whatever the source format,
//! the audio handed on to voice-activity detection is 16 kHz mono, multi-channel input is
//! averaged, and the duration is preserved within ±10 ms.

mod common;

use std::io::Cursor;
use std::time::Duration;

use common::{Encoding, record_until_end, samples_of, tone, wav_bytes};
use echo_lib::audio::{
    AudioFormat, Pace, SpeechConverter, TARGET_SAMPLE_RATE, WavAudioSource, WavOptions,
};

const TONE_HZ: f32 = 1_000.0;
const AMPLITUDE: f32 = 0.5;
const SECONDS: f32 = 2.0;

/// Plays a generated WAV through the WAV Audio Source and the converter, as the pipeline does.
fn speech_audio_from(format: AudioFormat, tone_channels: &[usize]) -> Vec<f32> {
    let frames = (format.sample_rate as f32 * SECONDS) as usize;
    let signal = tone(format.sample_rate, TONE_HZ, AMPLITUDE);
    let bytes = wav_bytes(format, Encoding::Int16, frames, |frame, channel| {
        if tone_channels.contains(&channel) {
            signal(frame)
        } else {
            0.0
        }
    });
    let options = WavOptions {
        pace: Pace::AsFastAsPossible,
        stop_at_end: true,
    };
    let mut source = WavAudioSource::from_reader(Cursor::new(bytes), options).unwrap();

    let (events, _stream) = record_until_end(&mut source, Duration::from_secs(10));
    let (got_format, samples) = samples_of(&events);
    assert_eq!(got_format, Some(format));

    let mut converter = SpeechConverter::new(format).unwrap();
    let mut out = converter.push(&samples);
    out.extend(converter.finish());
    out
}

fn assert_speech_audio(out: &[f32], expected_amplitude: f32) {
    // Same duration ±10 ms at 16 kHz.
    let expected_len = (TARGET_SAMPLE_RATE as f32 * SECONDS) as i64;
    let tolerance = (TARGET_SAMPLE_RATE / 100) as i64;
    assert!(
        (out.len() as i64 - expected_len).abs() <= tolerance,
        "length {} vs {expected_len}",
        out.len()
    );

    // Away from the edges (resampler ramp-in/out), the tone keeps the averaged amplitude...
    let edge = (TARGET_SAMPLE_RATE / 10) as usize;
    let middle = &out[edge..out.len() - edge];
    let peak = middle.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(
        (peak - expected_amplitude).abs() <= expected_amplitude * 0.03,
        "peak {peak} vs {expected_amplitude}"
    );

    // ...and its frequency: a 1 kHz tone crosses zero 2000 times per second.
    let crossings = middle
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count() as f32;
    let expected_crossings = 2.0 * TONE_HZ * middle.len() as f32 / TARGET_SAMPLE_RATE as f32;
    assert!(
        (crossings - expected_crossings).abs() <= expected_crossings * 0.01,
        "crossings {crossings} vs {expected_crossings}"
    );
}

/// Acceptance test 5.
#[test]
fn stereo_48_khz_with_a_tone_on_the_left_becomes_16_khz_mono_at_half_amplitude() {
    let format = AudioFormat {
        sample_rate: 48_000,
        channels: 2,
    };
    let out = speech_audio_from(format, &[0]);
    assert_speech_audio(&out, AMPLITUDE / 2.0);
}

/// Acceptance test 6, 44.1 kHz mono.
#[test]
fn mono_44_1_khz_becomes_16_khz_mono_at_full_amplitude() {
    let format = AudioFormat {
        sample_rate: 44_100,
        channels: 1,
    };
    let out = speech_audio_from(format, &[0]);
    assert_speech_audio(&out, AMPLITUDE);
}

/// Acceptance test 6, 8 kHz mono (upsampling).
#[test]
fn mono_8_khz_becomes_16_khz_mono_at_full_amplitude() {
    let format = AudioFormat {
        sample_rate: 8_000,
        channels: 1,
    };
    let out = speech_audio_from(format, &[0]);
    assert_speech_audio(&out, AMPLITUDE);
}

/// Acceptance test 6, 96 kHz 4-channel: the tone on two of four channels averages to half.
#[test]
fn four_channel_96_khz_with_the_tone_on_two_channels_becomes_16_khz_mono_at_half_amplitude() {
    let format = AudioFormat {
        sample_rate: 96_000,
        channels: 4,
    };
    let out = speech_audio_from(format, &[0, 1]);
    assert_speech_audio(&out, AMPLITUDE / 2.0);
}

/// Acceptance test 6, 4-channel: a tone on one of four channels averages to a quarter.
#[test]
fn four_channel_96_khz_with_the_tone_on_one_channel_becomes_a_quarter_amplitude() {
    let format = AudioFormat {
        sample_rate: 96_000,
        channels: 4,
    };
    let out = speech_audio_from(format, &[2]);
    assert_speech_audio(&out, AMPLITUDE / 4.0);
}
