//! Conversion of any Audio Source format to the 16 kHz mono audio that voice-activity detection
//! and the Engine expect (`dictation-pipeline.md` rules 10–11).

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use super::AudioFormat;

/// Sample rate of the audio given to voice-activity detection and the Engine.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// The source format cannot be converted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot convert audio with {sample_rate} Hz and {channels} channel(s): {detail}")]
pub struct ConversionError {
    pub sample_rate: u32,
    pub channels: u16,
    pub detail: String,
}

/// Mixes interleaved multi-channel audio down to mono by averaging all channels of each frame.
/// A trailing incomplete frame is ignored.
pub fn downmix(interleaved: &[f32], channels: u16) -> Vec<f32> {
    match channels {
        0 => Vec::new(),
        1 => interleaved.to_vec(),
        n => {
            let n = n as usize;
            interleaved
                .chunks_exact(n)
                .map(|frame| frame.iter().sum::<f32>() / n as f32)
                .collect()
        }
    }
}

/// Input frames per resampler chunk: about 20 ms at 48 kHz, small enough to keep latency low.
const CHUNK_FRAMES: usize = 1024;

/// Streaming converter from one source format to 16 kHz mono: downmix by averaging, then
/// band-limited resampling (rubato's FFT resampler).
///
/// Feed it every piece of a Recording with [`push`](Self::push) and call
/// [`finish`](Self::finish) at the end. The concatenated output has the input's duration exactly
/// (rounded to the nearest 16 kHz sample) and no resampler start-up delay; how the input was cut
/// into pieces does not change the result.
pub struct SpeechConverter {
    channels: u16,
    sample_rate: u32,
    resampler: Option<Resampling>,
    input_frames: u64,
    emitted: u64,
}

struct Resampling {
    resampler: Fft<f32>,
    /// Mono input waiting for a full chunk.
    pending: Vec<f32>,
    /// Scratch output buffer, sized for the largest chunk.
    output: Vec<f32>,
    /// Start-up delay frames not yet dropped from the output.
    delay_left: usize,
}

impl SpeechConverter {
    /// Creates a converter for audio in `format`.
    pub fn new(format: AudioFormat) -> Result<Self, ConversionError> {
        let error = |detail: String| ConversionError {
            sample_rate: format.sample_rate,
            channels: format.channels,
            detail,
        };
        if format.sample_rate == 0 || format.channels == 0 {
            return Err(error(
                "sample rate and channel count must be positive".into(),
            ));
        }
        let resampler = if format.sample_rate == TARGET_SAMPLE_RATE {
            None
        } else {
            let resampler = Fft::<f32>::new(
                format.sample_rate as usize,
                TARGET_SAMPLE_RATE as usize,
                CHUNK_FRAMES,
                1,
                FixedSync::Input,
            )
            .map_err(|e| error(e.to_string()))?;
            Some(Resampling {
                pending: Vec::with_capacity(resampler.input_frames_max()),
                output: vec![0.0; resampler.output_frames_max()],
                delay_left: resampler.output_delay(),
                resampler,
            })
        };
        Ok(Self {
            channels: format.channels,
            sample_rate: format.sample_rate,
            resampler,
            input_frames: 0,
            emitted: 0,
        })
    }

    /// Converts the next piece of interleaved source audio and returns the 16 kHz mono samples
    /// that are ready. Some audio is held back until the next call or [`finish`](Self::finish).
    pub fn push(&mut self, interleaved: &[f32]) -> Vec<f32> {
        let mono = downmix(interleaved, self.channels);
        self.input_frames += mono.len() as u64;
        let out = match &mut self.resampler {
            None => mono,
            Some(r) => {
                r.pending.extend_from_slice(&mono);
                let mut out = Vec::new();
                while r.pending.len() >= r.resampler.input_frames_next() {
                    r.process(&mut out, None);
                }
                out
            }
        };
        self.emitted += out.len() as u64;
        out
    }

    /// Flushes the audio still held back at the end of a Recording.
    pub fn finish(mut self) -> Vec<f32> {
        let expected = (self.input_frames as f64 * TARGET_SAMPLE_RATE as f64
            / self.sample_rate as f64)
            .round() as u64;
        let mut out = Vec::new();
        if let Some(r) = &mut self.resampler {
            if !r.pending.is_empty() {
                let partial = r.pending.len();
                r.process(&mut out, Some(partial));
            }
            // The resampler holds its start-up delay worth of audio; pump silence through it.
            while self.emitted + (out.len() as u64) < expected {
                r.process(&mut out, Some(0));
            }
        }
        out.truncate(expected.saturating_sub(self.emitted) as usize);
        out
    }
}

impl Resampling {
    /// Resamples one chunk from `pending` (all of it is real audio unless `partial_len` says how
    /// much is) and appends the output, minus any remaining start-up delay, to `out`.
    fn process(&mut self, out: &mut Vec<f32>, partial_len: Option<usize>) {
        let needed = self.resampler.input_frames_next();
        let real = partial_len.unwrap_or(needed).min(self.pending.len());
        if self.pending.len() < needed {
            self.pending.resize(needed, 0.0); // only for the final, partial chunk
        }

        let input = InterleavedSlice::new(&self.pending[..needed], 1, needed)
            .expect("input slice holds exactly one chunk");
        let capacity = self.output.len();
        let mut output = InterleavedSlice::new_mut(&mut self.output, 1, capacity)
            .expect("output slice holds the largest chunk");
        let indexing = partial_len.map(|_| Indexing::new().partial_len(real));
        let (_, produced) = self
            .resampler
            .process_into_buffer(&input, &mut output, indexing.as_ref())
            .expect("buffers are sized from the resampler's own limits");

        let skip = self.delay_left.min(produced);
        self.delay_left -= skip;
        out.extend_from_slice(&self.output[skip..produced]);
        match partial_len {
            Some(_) => self.pending.clear(),
            None => drop(self.pending.drain(..needed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_all_channels_of_each_frame() {
        let stereo = [1.0, 0.0, 0.5, 0.5, -1.0, 0.0];
        assert_eq!(downmix(&stereo, 2), vec![0.5, 0.5, -0.5]);

        let quad = [1.0, 1.0, 0.0, 0.0, 0.4, 0.4, 0.4, 0.4];
        assert_eq!(downmix(&quad, 4), vec![0.5, 0.4]);
    }

    #[test]
    fn downmix_of_mono_is_identity() {
        assert_eq!(downmix(&[0.1, -0.2, 0.3], 1), vec![0.1, -0.2, 0.3]);
    }

    #[test]
    fn downmix_ignores_a_trailing_partial_frame() {
        assert_eq!(downmix(&[0.2, 0.4, 0.6], 2), vec![0.3]);
    }

    fn format(sample_rate: u32, channels: u16) -> AudioFormat {
        AudioFormat {
            sample_rate,
            channels,
        }
    }

    fn convert_all(fmt: AudioFormat, interleaved: &[f32]) -> Vec<f32> {
        let mut converter = SpeechConverter::new(fmt).unwrap();
        let mut out = converter.push(interleaved);
        out.extend(converter.finish());
        out
    }

    #[test]
    fn output_length_matches_input_duration_at_16_khz() {
        for rate in [8_000, 16_000, 22_050, 44_100, 48_000, 96_000] {
            let frames = rate as usize * 3 / 2; // 1.5 s
            let out = convert_all(format(rate, 1), &vec![0.0; frames]);
            assert_eq!(out.len(), 24_000, "rate {rate}");
        }
    }

    #[test]
    fn audio_already_at_16_khz_mono_passes_through_unchanged() {
        let input: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0) - 0.5).collect();
        assert_eq!(convert_all(format(16_000, 1), &input), input);
    }

    #[test]
    fn streaming_in_uneven_pieces_equals_converting_at_once() {
        let fmt = format(44_100, 2);
        let input: Vec<f32> = (0..44_100 * 2)
            .map(|i| ((i as f32) * 0.013).sin() * 0.3)
            .collect();
        let whole = convert_all(fmt, &input);

        let mut converter = SpeechConverter::new(fmt).unwrap();
        let mut pieced = Vec::new();
        let mut rest = &input[..];
        for size in [2usize, 882, 7, 4096].iter().cycle().map(|s| s * 2) {
            if rest.is_empty() {
                break;
            }
            let (piece, tail) = rest.split_at(size.min(rest.len()));
            pieced.extend(converter.push(piece));
            rest = tail;
        }
        pieced.extend(converter.finish());

        assert_eq!(pieced.len(), whole.len());
        for (a, b) in pieced.iter().zip(&whole) {
            assert!((a - b).abs() < 1e-5, "{a} != {b}");
        }
    }

    #[test]
    fn rejects_formats_without_rate_or_channels() {
        assert!(SpeechConverter::new(format(0, 1)).is_err());
        assert!(SpeechConverter::new(format(48_000, 0)).is_err());
    }
}
