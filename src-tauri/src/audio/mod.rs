//! The **Audio Source** seam: anything that supplies speech audio to a Dictation.
//!
//! [`microphone::Microphone`] is the real source (cpal/WASAPI, `docs/specs/microphone.md`);
//! [`WavAudioSource`] is the fake used in tests and fake-microphone mode
//! (`docs/specs/dictation-pipeline.md`, rules 40–46).
//!
//! Audio flows *push*-style: [`AudioSource::start`] opens the source for one Recording and hands
//! every piece of audio, in the source's native format, to an [`AudioSink`] callback on the
//! source's own thread until the returned [`AudioStream`] is stopped. The pipeline turns that
//! native audio into 16 kHz mono with [`SpeechConverter`] before voice-activity detection and the
//! Engine (rules 10–11).

mod convert;
pub mod microphone;
mod wav;

pub use convert::{ConversionError, SpeechConverter, TARGET_SAMPLE_RATE, downmix};
pub use wav::{Pace, WavAudioSource, WavOptions};

/// Sample rate and channel count of the audio an [`AudioSource`] delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    /// Frames per second, e.g. 48 000.
    pub sample_rate: u32,
    /// Interleaved channels per frame, at least 1.
    pub channels: u16,
}

/// A piece of audio exactly as the source produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioFrames {
    /// Format of `samples`; constant for the lifetime of one [`AudioStream`].
    pub format: AudioFormat,
    /// Interleaved samples in `-1.0..=1.0`; its length is a multiple of `format.channels`.
    pub samples: Vec<f32>,
}

/// What an [`AudioSource`] reports to its [`AudioSink`] during a Recording.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    /// More audio. The first `Frames` of a Recording marks the moment the device actually
    /// delivers sound (the Overlay leaves its "getting ready" look then — rule 7).
    Frames(AudioFrames),
    /// The source has no more audio and the Recording should stop normally, e.g. the WAV Audio
    /// Source reached the end of its file with [`WavOptions::stop_at_end`] (rule 44).
    /// No events follow.
    Ended,
    /// The source failed after it had started, e.g. the Microphone was unplugged
    /// (`microphone.md` rule 10). Audio delivered before this is valid and should still be
    /// transcribed. No events follow.
    Failed(AudioSourceError),
}

/// Receives the [`AudioEvent`]s of one Recording. Called on the source's thread (for a
/// Microphone: the audio callback thread), so it must return quickly — typically it forwards
/// the event into a channel. It must not call [`AudioStream::stop`] on its own stream.
pub type AudioSink = Box<dyn FnMut(AudioEvent) + Send>;

/// Why an [`AudioSource`] could not open, or stopped delivering audio.
///
/// The `Display` texts are developer-facing details; the UI maps the variants to translated
/// messages (`dictation-pipeline.md` rule 8).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioSourceError {
    /// There is no audio input device at all.
    #[error("no microphone found")]
    NotFound,
    /// Windows privacy settings block microphone access for desktop apps.
    #[error("microphone access is blocked in Windows privacy settings")]
    AccessDenied,
    /// The device stopped delivering audio during a Recording.
    #[error("microphone disconnected")]
    Disconnected,
    /// Any other failure, with a detail for the user and the log.
    #[error("audio source failed: {0}")]
    Failed(String),
}

/// Anything that supplies speech audio to a Dictation: a Microphone or, for testing, a WAV file.
///
/// # Contract
///
/// - [`start`](AudioSource::start) opens the source for **one Recording** and returns once audio
///   is about to flow, or fails without ever calling the sink (rule 8). A Microphone is opened
///   only here and released when the returned [`AudioStream`] stops (rule 9).
/// - From then on the source calls `sink` from its own thread with zero or more
///   [`AudioEvent::Frames`], optionally followed by exactly one terminal [`AudioEvent::Ended`]
///   or [`AudioEvent::Failed`].
/// - The Recording ends when the caller stops (or drops) the [`AudioStream`]; after
///   [`AudioStream::stop`] returns, the sink is never called again and has been dropped.
/// - A source can be started again for the next Recording once the previous stream stopped;
///   each start begins afresh (a WAV file from its beginning, a Microphone re-opened).
/// - Audio is delivered in the source's native format; downmixing and resampling to 16 kHz mono
///   is the caller's job ([`SpeechConverter`]), so every source gets identical conversion.
pub trait AudioSource: Send {
    /// Opens the source and starts delivering audio for one Recording to `sink`.
    fn start(&mut self, sink: AudioSink) -> Result<AudioStream, AudioSourceError>;
}

/// Handle to the audio flowing from an [`AudioSource`] for one Recording.
///
/// Stopping it (explicitly or by dropping it) ends delivery and releases the device.
#[must_use = "dropping the AudioStream stops the Recording immediately"]
pub struct AudioStream {
    stop: Option<Box<dyn FnOnce() + Send>>,
}

impl AudioStream {
    /// Wraps the function an [`AudioSource`] implementation uses to stop delivery. The function
    /// must not return before the sink has been called for the last time.
    pub fn new(stop: impl FnOnce() + Send + 'static) -> Self {
        Self {
            stop: Some(Box::new(stop)),
        }
    }

    /// Stops delivery and releases the source. Returns after the last sink call.
    pub fn stop(mut self) {
        self.stop_now();
    }

    fn stop_now(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        self.stop_now();
    }
}

impl std::fmt::Debug for AudioStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioStream")
            .field("running", &self.stop.is_some())
            .finish()
    }
}
