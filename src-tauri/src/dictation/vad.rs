//! Voice-activity detection during a Recording (`dictation-pipeline.md` rules 14–19).
//!
//! [`SpeechGate`] receives the Recording's 16 kHz mono audio as it arrives and keeps only speech
//! plus its pre- and post-roll. The detector itself sits behind [`VoiceDetector`]: [`Earshot`]
//! is the real one, tests use simple fakes.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::audio::TARGET_SAMPLE_RATE;

/// Samples in one detector frame: 16 ms at 16 kHz (earshot's fixed frame size).
pub const FRAME: usize = 256;

/// Scores at or above this count as speech (earshot's recommended default).
const SPEECH_THRESHOLD: f32 = 0.5;

const fn samples(ms: u32) -> usize {
    (TARGET_SAMPLE_RATE / 1000 * ms) as usize
}

/// Consecutive speech needed before speech is considered started (rule 15): 60 ms, rounded up
/// to whole frames (4 × 16 ms).
const SPEECH_START_FRAMES: usize = samples(60).div_ceil(FRAME);
/// Audio kept before speech starts (rule 15).
const PRE_ROLL: usize = samples(450);
/// Audio kept after speech stops (rule 16).
const POST_ROLL: usize = samples(450);

/// Kept speech shorter than this is padded (rule 19).
const PAD_BELOW: usize = samples(1000);
/// Length short speech is padded to with trailing silence (rule 19).
pub const PADDED_LEN: usize = samples(1250);

/// Judges one [`FRAME`] of 16 kHz mono audio.
pub trait VoiceDetector: Send {
    /// The probability that the frame is speech, in `0.0..=1.0`, or `None` when the detector
    /// failed on it (the frame is then treated as speech — rule 17).
    fn score(&mut self, frame: &[f32]) -> Option<f32>;
}

/// The real detector: earshot's neural voice-activity detector.
pub struct Earshot(Box<earshot::Detector>);

impl Earshot {
    pub fn new() -> Self {
        Self(earshot::Detector::default_boxed())
    }
}

impl Default for Earshot {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceDetector for Earshot {
    fn score(&mut self, frame: &[f32]) -> Option<f32> {
        // earshot expects samples in -1..=1; converters can overshoot slightly.
        let clamped: Vec<f32> = frame.iter().map(|s| s.clamp(-1.0, 1.0)).collect();
        let score = self.0.predict_f32(&clamped);
        // earshot reports bad input as -1.0.
        (0.0..=1.0).contains(&score).then_some(score)
    }
}

/// Keeps the speech of one Recording and drops the silence around it.
pub struct SpeechGate {
    detector: Box<dyn VoiceDetector>,
    /// Samples waiting for a full frame.
    partial: Vec<f32>,
    /// Recent dropped audio, at most [`PRE_ROLL`] samples, kept in case speech starts.
    pre_roll: VecDeque<f32>,
    /// Frames of a speech run that has not lasted long enough yet.
    candidate: Vec<f32>,
    candidate_frames: usize,
    /// Whether speech is in progress; then `post_roll_left` samples of non-speech are still kept.
    in_speech: bool,
    post_roll_left: usize,
    kept: Vec<f32>,
}

impl SpeechGate {
    pub fn new(detector: Box<dyn VoiceDetector>) -> Self {
        Self {
            detector,
            partial: Vec::with_capacity(FRAME),
            pre_roll: VecDeque::with_capacity(PRE_ROLL + FRAME),
            candidate: Vec::new(),
            candidate_frames: 0,
            in_speech: false,
            post_roll_left: 0,
            kept: Vec::new(),
        }
    }

    /// Feeds the next 16 kHz mono samples of the Recording.
    pub fn push(&mut self, mut samples: &[f32]) {
        while !samples.is_empty() {
            let take = (FRAME - self.partial.len()).min(samples.len());
            self.partial.extend_from_slice(&samples[..take]);
            samples = &samples[take..];
            if self.partial.len() == FRAME {
                let frame = std::mem::take(&mut self.partial);
                self.frame(&frame);
                self.partial = frame;
                self.partial.clear();
            }
        }
    }

    /// Ends the Recording and returns the kept speech audio, unpadded; empty when no speech was
    /// found (rule 18). A trailing partial frame is judged padded with silence.
    pub fn finish(mut self) -> Vec<f32> {
        if !self.partial.is_empty() {
            let real = self.partial.len();
            let mut frame = std::mem::take(&mut self.partial);
            frame.resize(FRAME, 0.0);
            let before = self.kept.len();
            self.frame(&frame);
            // Only the real samples of the last frame belong to the Recording.
            if self.kept.len() > before {
                let end = self.kept.len() - (FRAME - real);
                self.kept.truncate(end);
            }
        }
        self.kept
    }

    fn frame(&mut self, frame: &[f32]) {
        let speech = self.is_speech(frame);
        if self.in_speech {
            if speech {
                self.kept.extend_from_slice(frame);
                self.post_roll_left = POST_ROLL;
            } else if self.post_roll_left > 0 {
                let keep = self.post_roll_left.min(FRAME);
                self.kept.extend_from_slice(&frame[..keep]);
                self.drop_audio(&frame[keep..]);
                self.post_roll_left -= keep;
            } else {
                self.in_speech = false;
                self.drop_audio(frame);
            }
        } else if speech {
            self.candidate.extend_from_slice(frame);
            self.candidate_frames += 1;
            if self.candidate_frames >= SPEECH_START_FRAMES {
                self.kept.extend(self.pre_roll.drain(..));
                self.kept.append(&mut self.candidate);
                self.candidate_frames = 0;
                self.in_speech = true;
                self.post_roll_left = POST_ROLL;
            }
        } else {
            let candidate = std::mem::take(&mut self.candidate);
            self.candidate_frames = 0;
            self.drop_audio(&candidate);
            self.drop_audio(frame);
        }
    }

    /// Audio not kept now, remembered as pre-roll for a later speech start.
    fn drop_audio(&mut self, audio: &[f32]) {
        self.pre_roll.extend(audio);
        let excess = self.pre_roll.len().saturating_sub(PRE_ROLL);
        self.pre_roll.drain(..excess);
    }

    /// Fail open: a detector error or panic counts as speech (rule 17).
    fn is_speech(&mut self, frame: &[f32]) -> bool {
        let detector = &mut self.detector;
        match catch_unwind(AssertUnwindSafe(|| detector.score(frame))) {
            Ok(Some(score)) => score >= SPEECH_THRESHOLD,
            Ok(None) | Err(_) => true,
        }
    }
}

/// Pads kept speech shorter than 1.0 s with trailing silence to 1.25 s (rule 19).
pub fn pad_short(mut audio: Vec<f32>) -> Vec<f32> {
    if audio.len() < PAD_BELOW {
        audio.resize(PADDED_LEN, 0.0);
    }
    audio
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A detector that calls any frame with energy above a small threshold speech.
    pub(crate) struct EnergyDetector;

    impl VoiceDetector for EnergyDetector {
        fn score(&mut self, frame: &[f32]) -> Option<f32> {
            let peak = frame.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            Some(if peak > 0.05 { 1.0 } else { 0.0 })
        }
    }

    /// A detector that always fails.
    struct BrokenDetector {
        panic: bool,
    }

    impl VoiceDetector for BrokenDetector {
        fn score(&mut self, _frame: &[f32]) -> Option<f32> {
            if self.panic {
                panic!("detector bug");
            }
            None
        }
    }

    fn gate() -> SpeechGate {
        SpeechGate::new(Box::new(EnergyDetector))
    }

    fn silence(ms: u32) -> Vec<f32> {
        vec![0.0; samples(ms)]
    }

    fn speech(ms: u32) -> Vec<f32> {
        (0..samples(ms))
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect()
    }

    fn run(pieces: &[Vec<f32>]) -> Vec<f32> {
        let mut gate = gate();
        for piece in pieces {
            gate.push(piece);
        }
        gate.finish()
    }

    #[test]
    fn silence_only_keeps_nothing() {
        assert!(run(&[silence(3000)]).is_empty());
    }

    #[test]
    fn speech_shorter_than_60_ms_is_dropped() {
        // 48 ms = 3 frames, one short of the 60 ms start.
        assert!(run(&[silence(1024), speech(48), silence(1024)]).is_empty());
    }

    #[test]
    fn speech_keeps_450_ms_pre_roll_and_post_roll() {
        let kept = run(&[silence(1024), speech(512), silence(1024)]);
        // The last 450 ms of the leading silence + the speech + 450 ms after it.
        assert_eq!(kept.len(), PRE_ROLL + samples(512) + POST_ROLL);
        let first_loud = kept.iter().position(|s| s.abs() > 0.1).unwrap();
        assert_eq!(first_loud, PRE_ROLL);
    }

    #[test]
    fn pre_roll_is_shorter_when_speech_starts_early() {
        let kept = run(&[silence(160), speech(320)]);
        assert_eq!(kept.len(), samples(160) + samples(320));
    }

    #[test]
    fn a_pause_shorter_than_the_post_roll_is_kept_whole() {
        let kept = run(&[speech(320), silence(320), speech(320)]);
        assert_eq!(kept.len(), samples(960));
    }

    #[test]
    fn a_long_pause_drops_the_middle_silence() {
        let kept = run(&[speech(320), silence(3200), speech(320)]);
        assert_eq!(
            kept.len(),
            samples(320) + POST_ROLL + PRE_ROLL + samples(320)
        );
    }

    #[test]
    fn how_audio_is_cut_into_pieces_does_not_matter() {
        let audio: Vec<f32> = [silence(700), speech(333), silence(900), speech(100)].concat();
        let whole = run(std::slice::from_ref(&audio));
        let pieces: Vec<Vec<f32>> = audio.chunks(97).map(<[f32]>::to_vec).collect();
        assert_eq!(run(&pieces), whole);
    }

    #[test]
    fn a_trailing_partial_frame_keeps_only_its_real_samples() {
        let mut audio = speech(320);
        audio.extend(vec![0.5; 100]);
        assert_eq!(run(&[audio]).len(), samples(320) + 100);
    }

    #[test]
    fn a_failing_detector_keeps_everything() {
        for panic in [false, true] {
            let mut gate = SpeechGate::new(Box::new(BrokenDetector { panic }));
            gate.push(&silence(1000));
            assert_eq!(gate.finish().len(), samples(1000), "panic: {panic}");
        }
    }

    #[test]
    fn earshot_finds_no_speech_in_digital_silence() {
        let mut gate = SpeechGate::new(Box::new(Earshot::new()));
        gate.push(&silence(3000));
        assert!(gate.finish().is_empty());
    }

    #[test]
    fn short_speech_is_padded_to_1_25_s_and_long_speech_is_not() {
        assert_eq!(pad_short(vec![0.1; samples(500)]).len(), PADDED_LEN);
        assert_eq!(pad_short(vec![0.1; samples(999)]).len(), PADDED_LEN);
        assert_eq!(pad_short(vec![0.1; samples(1000)]).len(), samples(1000));
    }
}
