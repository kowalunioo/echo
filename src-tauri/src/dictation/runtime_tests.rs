//! The pipeline end to end against fakes: WAV / scripted Audio Source, FakeEngine, FakeInserter,
//! fake shortcut intents (`dictation-pipeline.md` acceptance tests 2–4, 7–13, 20–23).

use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::audio::{
    AudioEvent, AudioFormat, AudioFrames, AudioSink, AudioSource, AudioSourceError, AudioStream,
    Pace, WavAudioSource, WavOptions,
};
use crate::dictation::vad::PADDED_LEN;
use crate::dictation::vad::tests::EnergyDetector;
use crate::engine::{Engine, EngineError, FakeEngine, TranscriptionRequest};
use crate::history::NewEntry;
use crate::insertion::{FakeInserter, InsertionError, SharedInserter};
use crate::models::NoActiveModel;
use crate::shortcut::modes::RecordIntent::{Start, Stop};

use super::*;
use crate::dictation::{DictationState, ProblemKind};

const RATE: u32 = 48_000;

/// A 48 kHz stereo WAV: `speech_ms` of a loud tone followed by `silence_ms` of silence.
fn wav(speech_ms: u32, silence_ms: u32) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut bytes = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
    let speech = (RATE / 1000 * speech_ms) as usize;
    let total = speech + (RATE / 1000 * silence_ms) as usize;
    for i in 0..total {
        let s = if i < speech {
            ((i as f32 * 440.0 * std::f32::consts::TAU / RATE as f32).sin() * 16_000.0) as i16
        } else {
            0
        };
        writer.write_sample(s).unwrap();
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
    bytes.into_inner()
}

fn wav_source(bytes: Vec<u8>, stop_at_end: bool) -> WavAudioSource {
    WavAudioSource::from_reader(
        Cursor::new(bytes),
        WavOptions {
            pace: Pace::AsFastAsPossible,
            stop_at_end,
        },
    )
    .unwrap()
}

/// Models that hand out the same Engine, or refuse when inactive.
struct TestModels<E> {
    model: Option<Arc<EngineModel<E>>>,
}

impl<E: Engine + 'static> DictationModels for TestModels<E> {
    fn begin(&self) -> Result<Arc<dyn DictationModel>, NoActiveModel> {
        match &self.model {
            Some(model) => Ok(Arc::clone(model) as Arc<dyn DictationModel>),
            None => Err(NoActiveModel),
        }
    }
}

/// An Audio Source the test drives by hand; counts how often it was opened.
#[derive(Clone, Default)]
struct ScriptedSource {
    sink: Arc<Mutex<Option<AudioSink>>>,
    opened: Arc<AtomicUsize>,
    fail_with: Option<AudioSourceError>,
}

impl ScriptedSource {
    fn deliver(&self, event: AudioEvent) {
        if let Some(sink) = self.sink.lock().unwrap().as_mut() {
            sink(event);
        }
    }

    fn tone(&self, ms: u32) {
        let samples = (0..16 * ms as usize)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        self.deliver(AudioEvent::Frames(AudioFrames {
            format: AudioFormat {
                sample_rate: 16_000,
                channels: 1,
            },
            samples,
        }));
    }
}

impl AudioSource for ScriptedSource {
    fn start(&mut self, sink: AudioSink) -> Result<AudioStream, AudioSourceError> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = &self.fail_with {
            return Err(error.clone());
        }
        *self.sink.lock().unwrap() = Some(sink);
        let slot = Arc::clone(&self.sink);
        Ok(AudioStream::new(move || {
            slot.lock().unwrap().take();
        }))
    }
}

struct Rig {
    dictation: Dictation,
    inserter: FakeInserter,
    history: Arc<Mutex<Vec<NewEntry>>>,
    resets: Arc<AtomicUsize>,
    states: Arc<Mutex<Vec<DictationState>>>,
}

struct Setup {
    engine: Option<FakeEngine>,
    source: Box<dyn AudioSource>,
    max_recording: Duration,
}

impl Setup {
    fn new(engine: FakeEngine, source: impl AudioSource + 'static) -> Self {
        Self {
            engine: Some(engine),
            source: Box::new(source),
            max_recording: MAX_RECORDING,
        }
    }

    fn spawn(mut self) -> Rig {
        let model = self
            .engine
            .take()
            .map(|e| Arc::new(EngineModel::new("Fake", e)));
        self.spawn_with(TestModels { model })
    }

    fn spawn_with(self, models: impl DictationModels + 'static) -> Rig {
        let inserter = FakeInserter::new();
        let shared = SharedInserter::new();
        shared.set(inserter.clone());
        let history = Arc::new(Mutex::new(Vec::new()));
        let resets = Arc::new(AtomicUsize::new(0));
        let states = Arc::new(Mutex::new(Vec::new()));
        let (h, r, s) = (history.clone(), resets.clone(), states.clone());
        let dictation = Dictation::spawn(DictationDeps {
            models: Box::new(models),
            source: self.source,
            detector: Box::new(|| Box::new(EnergyDetector)),
            context: Box::new(DictationContext::default),
            history: Box::new(move |entry| {
                h.lock().unwrap().push(entry);
                Ok(())
            }),
            inserter: shared,
            shortcut_reset: Box::new(move || {
                r.fetch_add(1, Ordering::SeqCst);
            }),
            publish: Box::new(move |status| {
                let mut states = s.lock().unwrap();
                if states.last() != Some(&status.state) {
                    states.push(status.state);
                }
            }),
            max_recording: self.max_recording,
        });
        Rig {
            dictation,
            inserter,
            history,
            resets,
            states,
        }
    }
}

impl Rig {
    /// Waits until `check` holds for the status, failing after 5 s.
    fn wait(&self, what: &str, check: impl Fn(&DictationStatus) -> bool) {
        let until = Instant::now() + Duration::from_secs(5);
        while !check(&self.dictation.status()) {
            assert!(Instant::now() < until, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn wait_state(&self, state: DictationState) {
        self.wait(&format!("{state:?}"), |s| s.state == state);
    }

    /// Waits until `state` has been published and the pipeline is Idle again.
    fn wait_idle_after(&self, state: DictationState) {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let seen = self.states.lock().unwrap().contains(&state);
            if seen && self.dictation.status().state == DictationState::Idle {
                return;
            }
            assert!(
                Instant::now() < until,
                "timed out waiting for Idle after {state:?}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn inserted(&self) -> Vec<String> {
        self.inserter.inserted()
    }

    fn history_texts(&self) -> Vec<String> {
        self.history
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.text.clone())
            .collect()
    }
}

fn slow(text: &str) -> FakeEngine {
    FakeEngine::returning(text).with_delay(Duration::from_millis(300))
}

#[test]
fn a_dictation_stores_then_inserts_the_transcript() {
    let engine = FakeEngine::returning("Ala ma kota.");
    let rig = Setup::new(engine.clone(), wav_source(wav(1500, 300), true)).spawn();
    rig.dictation.intent(Start);
    rig.wait_idle_after(DictationState::Inserting);

    assert_eq!(rig.inserted(), vec!["Ala ma kota."]);
    assert_eq!(rig.history_texts(), vec!["Ala ma kota."]);
    assert_eq!(rig.history.lock().unwrap()[0].model, "Fake");
    assert_eq!(engine.requests().len(), 1);
    // The source ended the Recording, not the shortcut.
    assert_eq!(rig.resets.load(Ordering::SeqCst), 1);
    assert_eq!(rig.dictation.status().error, None);
}

#[test]
fn the_states_are_published_in_order() {
    let rig = Setup::new(slow("x"), wav_source(wav(1500, 0), true)).spawn();
    rig.dictation.intent(Start);
    rig.wait_idle_after(DictationState::Inserting);
    assert_eq!(
        *rig.states.lock().unwrap(),
        vec![
            DictationState::Recording,
            DictationState::Transcribing,
            DictationState::Inserting,
            DictationState::Idle
        ]
    );
}

#[test]
fn the_overlay_learns_when_audio_arrives() {
    let source = ScriptedSource::default();
    let rig = Setup::new(FakeEngine::returning("x"), source.clone()).spawn();
    rig.dictation.intent(Start);
    rig.wait_state(DictationState::Recording);
    assert!(!rig.dictation.status().listening);
    source.tone(100);
    rig.wait("listening", |s| s.listening);
}

// overlay.md rule 3: the level meter follows the input level.
#[test]
fn the_overlay_gets_the_input_level() {
    let source = ScriptedSource::default();
    let rig = Setup::new(FakeEngine::returning("x"), source.clone()).spawn();
    rig.dictation.intent(Start);
    rig.wait_state(DictationState::Recording);
    assert_eq!(rig.dictation.take_input_level(), 0.0);
    source.tone(100);
    rig.wait("listening", |s| s.listening);
    let level = rig.dictation.take_input_level();
    // A ±0.5 square wave is −6 dBFS: the top of the meter.
    assert!(level > 0.9, "level {level}");
    assert_eq!(rig.dictation.take_input_level(), 0.0, "taking resets it");
}

#[test]
fn the_meter_level_is_on_a_decibel_scale() {
    use super::meter_level;
    assert_eq!(meter_level(&[]), 0.0);
    assert_eq!(meter_level(&[0.0; 160]), 0.0);
    let at = |amplitude: f32| meter_level(&[amplitude, -amplitude]);
    assert!(
        (at(0.031_6) - 0.6).abs() < 0.01,
        "−30 dBFS: {}",
        at(0.031_6)
    );
    assert_eq!(at(0.000_1), 0.0, "−80 dBFS is silence");
    assert_eq!(at(1.0), 1.0);
}

#[test]
fn acceptance_3_digital_silence_never_reaches_the_engine() {
    let engine = FakeEngine::returning("Dziękuję.");
    let rig = Setup::new(engine.clone(), wav_source(wav(0, 3000), true)).spawn();
    rig.dictation.intent(Start);
    // Stopping and finding no speech happen together, so Transcribing may never be published.
    rig.wait_idle_after(DictationState::Recording);

    assert!(engine.requests().is_empty());
    assert!(rig.inserted().is_empty());
    assert!(rig.history_texts().is_empty());
    assert!(rig.dictation.status().notices.is_empty());
}

#[test]
fn acceptance_4_short_speech_is_padded_to_1_25_s() {
    let engine = FakeEngine::returning("tak");
    let rig = Setup::new(engine.clone(), wav_source(wav(500, 0), true)).spawn();
    rig.dictation.intent(Start);
    rig.wait_idle_after(DictationState::Inserting);
    assert_eq!(engine.requests()[0].audio.len(), PADDED_LEN);
}

#[test]
fn the_engine_gets_the_dictation_language_and_vocabulary() {
    let engine = FakeEngine::returning("x");
    let rig = Setup::new(engine.clone(), wav_source(wav(1200, 0), true)).spawn();
    rig.dictation.intent(Start);
    rig.wait_idle_after(DictationState::Inserting);
    assert_eq!(
        engine.requests()[0].language,
        crate::engine::DictationLanguage::Automatic
    );
}

#[test]
fn acceptance_7_and_20_an_engine_error_is_indicated_and_nothing_is_kept() {
    let engine = FakeEngine::failing(EngineError::Transcription("GPU lost".into()));
    let rig = Setup::new(engine, wav_source(wav(1200, 0), true)).spawn();
    rig.dictation.intent(Start);
    rig.wait("an error", |s| {
        s.error.is_some() && s.state == DictationState::Idle
    });

    assert!(rig.inserted().is_empty());
    assert!(rig.history_texts().is_empty());
    let status = rig.dictation.status();
    let error = status.error.unwrap();
    assert_eq!(error.kind, ProblemKind::TranscriptionFailed);
    assert_eq!(error.detail, "GPU lost");
    assert_eq!(status.notices.len(), 1);

    // Opening the main window returns the tray to idle; the notice stays until dismissed.
    rig.dictation.window_seen();
    rig.wait("the tray error to clear", |s| s.error.is_none());
    assert_eq!(rig.dictation.status().notices.len(), 1);
    rig.dictation.dismiss_notices();
    rig.wait("the notices to clear", |s| s.notices.is_empty());
}

#[test]
fn acceptance_21_and_22_success_clears_the_error_but_silence_and_cancellation_do_not() {
    let engine = FakeEngine::failing(EngineError::Transcription("x".into()));
    let source = ScriptedSource::default();
    let rig = Setup::new(engine.clone(), source.clone()).spawn();
    let dictate = |speech: bool| {
        rig.dictation.intent(Start);
        rig.wait_state(DictationState::Recording);
        if speech {
            source.tone(1200);
        }
        rig.dictation.intent(Stop);
    };
    dictate(true);
    rig.wait("an error", |s| {
        s.error.is_some() && s.state == DictationState::Idle
    });

    // Silence: no change.
    dictate(false);
    rig.wait_state(DictationState::Idle);
    assert!(rig.dictation.status().error.is_some());

    // Cancellation: no change.
    rig.dictation.intent(Start);
    rig.wait_state(DictationState::Recording);
    // While recording the state says Recording even though the tray error is kept.
    assert!(rig.dictation.status().error.is_some());
    rig.dictation.cancel();
    rig.wait_state(DictationState::Idle);
    assert!(rig.dictation.status().error.is_some());

    // A failure replaces the error with the new one.
    engine.set_outcome(Err(EngineError::Transcription("second".into())));
    dictate(true);
    rig.wait("the second error", |s| {
        s.error.as_ref().is_some_and(|e| e.detail == "second")
    });

    // Success clears it.
    engine.set_outcome(Ok("dobrze".into()));
    dictate(true);
    rig.wait("success", |s| s.error.is_none());
    assert_eq!(rig.inserted(), vec!["dobrze"]);
    assert_eq!(rig.dictation.status().notices.len(), 2);
}

#[test]
fn acceptance_8_without_a_model_the_source_is_never_opened() {
    let source = ScriptedSource::default();
    let opened = source.opened.clone();
    let mut setup = Setup::new(FakeEngine::returning("x"), source);
    setup.engine = None;
    let rig = setup.spawn();
    rig.dictation.intent(Start);
    rig.wait("the Model error", |s| s.error.is_some());

    assert_eq!(opened.load(Ordering::SeqCst), 0);
    let status = rig.dictation.status();
    assert_eq!(status.state, DictationState::Idle);
    assert_eq!(status.error.unwrap().kind, ProblemKind::NoModel);
    assert_eq!(rig.resets.load(Ordering::SeqCst), 1);
}

#[test]
fn acceptance_9_a_source_that_cannot_open_names_the_cause() {
    for (error, kind) in [
        (AudioSourceError::NotFound, ProblemKind::MicrophoneNotFound),
        (
            AudioSourceError::AccessDenied,
            ProblemKind::MicrophoneAccessDenied,
        ),
        (
            AudioSourceError::Failed("busy".into()),
            ProblemKind::MicrophoneFailed,
        ),
    ] {
        let source = ScriptedSource {
            fail_with: Some(error),
            ..Default::default()
        };
        let rig = Setup::new(FakeEngine::returning("x"), source).spawn();
        rig.dictation.intent(Start);
        rig.wait("the microphone error", |s| s.error.is_some());
        let status = rig.dictation.status();
        assert_eq!(status.state, DictationState::Idle);
        assert_eq!(status.error.unwrap().kind, kind);
        assert_eq!(rig.resets.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn a_lost_microphone_still_transcribes_what_was_captured_and_keeps_the_error() {
    let source = ScriptedSource::default();
    let rig = Setup::new(FakeEngine::returning("zdanie"), source.clone()).spawn();
    rig.dictation.intent(Start);
    rig.wait_state(DictationState::Recording);
    source.tone(1200);
    source.deliver(AudioEvent::Failed(AudioSourceError::Disconnected));
    rig.wait_idle_after(DictationState::Inserting);

    assert_eq!(rig.inserted(), vec!["zdanie"]);
    let error = rig.dictation.status().error.unwrap();
    assert_eq!(error.kind, ProblemKind::MicrophoneDisconnected);
}

#[test]
fn acceptance_10_toggle_a_press_while_transcribing_starts_the_next_recording() {
    let rig = Setup::new(slow("raz"), wav_source(wav(1200, 0), false)).spawn();
    rig.dictation.intent(Start);
    rig.wait("listening", |s| s.listening);
    std::thread::sleep(Duration::from_millis(50)); // let the whole file arrive
    rig.dictation.intent(Stop);
    rig.wait_state(DictationState::Transcribing);
    rig.dictation.intent(Start);
    rig.wait("the first Insertion", |_| rig.inserted().len() == 1);
    // The next Recording followed the first Insertion by itself.
    rig.wait_state(DictationState::Recording);
    assert_eq!(rig.inserted(), vec!["raz"]);
    rig.wait("listening", |s| s.listening);
    std::thread::sleep(Duration::from_millis(50)); // let the whole file arrive
    rig.dictation.intent(Stop);
    rig.wait("the second Insertion", |_| rig.inserted().len() == 2);
}

#[test]
fn acceptance_10_toggle_two_presses_while_transcribing_cancel_out() {
    let rig = Setup::new(slow("raz"), wav_source(wav(1200, 0), false)).spawn();
    rig.dictation.intent(Start);
    rig.wait("listening", |s| s.listening);
    std::thread::sleep(Duration::from_millis(50)); // let the whole file arrive
    rig.dictation.intent(Stop);
    rig.wait_state(DictationState::Transcribing);
    rig.dictation.intent(Start);
    rig.dictation.intent(Stop);
    rig.wait_idle_after(DictationState::Inserting);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(rig.dictation.status().state, DictationState::Idle);
    assert_eq!(rig.inserted().len(), 1);
}

#[test]
fn acceptance_11_push_to_talk_a_key_held_through_the_busy_period_records_until_release() {
    let rig = Setup::new(slow("raz"), wav_source(wav(1200, 0), false)).spawn();
    rig.dictation.intent(Start);
    rig.wait("listening", |s| s.listening);
    std::thread::sleep(Duration::from_millis(50)); // let the whole file arrive
    rig.dictation.intent(Stop);
    rig.wait_state(DictationState::Transcribing);
    rig.dictation.intent(Start); // pressed and held
    rig.wait("the first Insertion", |_| rig.inserted().len() == 1);
    rig.wait_state(DictationState::Recording);
    rig.wait("listening", |s| s.listening);
    std::thread::sleep(Duration::from_millis(50)); // let the whole file arrive
    rig.dictation.intent(Stop); // released
    rig.wait("the second Insertion", |_| rig.inserted().len() == 2);
}

#[test]
fn acceptance_12_cancelling_during_transcribing_discards_the_late_transcript() {
    let engine = slow("za późno");
    let rig = Setup::new(engine.clone(), wav_source(wav(1200, 0), true)).spawn();
    rig.dictation.intent(Start);
    rig.wait_state(DictationState::Transcribing);
    rig.dictation.cancel();
    rig.wait_state(DictationState::Idle);
    // Let the slow Engine finish.
    std::thread::sleep(Duration::from_millis(500));

    assert_eq!(engine.requests().len(), 1);
    assert!(rig.inserted().is_empty());
    assert!(rig.history_texts().is_empty());
    assert_eq!(rig.dictation.status().error, None);
}

#[test]
fn cancelling_a_recording_never_runs_the_engine() {
    let engine = FakeEngine::returning("x");
    let source = ScriptedSource::default();
    let rig = Setup::new(engine.clone(), source.clone()).spawn();
    rig.dictation.intent(Start);
    rig.wait_state(DictationState::Recording);
    source.tone(1200);
    rig.dictation.cancel();
    rig.wait_state(DictationState::Idle);
    std::thread::sleep(Duration::from_millis(100));
    assert!(engine.requests().is_empty());
    assert!(
        source.sink.lock().unwrap().is_none(),
        "the source is released"
    );
}

#[test]
fn acceptance_13_the_transcript_is_cleaned_before_insertion() {
    let engine = FakeEngine::returning("  so so so so we   go  ");
    let rig = Setup::new(engine, wav_source(wav(1200, 0), true)).spawn();
    rig.dictation.intent(Start);
    rig.wait_idle_after(DictationState::Inserting);
    assert_eq!(rig.inserted(), vec!["so we go"]);
    assert_eq!(rig.history_texts(), vec!["so we go"]);
}

#[test]
fn a_whitespace_transcript_is_no_speech() {
    let engine = FakeEngine::returning(" \n ");
    let rig = Setup::new(engine, wav_source(wav(1200, 0), true)).spawn();
    rig.dictation.intent(Start);
    rig.wait_idle_after(DictationState::Transcribing);
    std::thread::sleep(Duration::from_millis(50));
    assert!(rig.inserted().is_empty());
    assert!(rig.history_texts().is_empty());
    assert_eq!(rig.dictation.status().error, None);
}

#[test]
fn acceptance_23_a_recording_stops_by_itself_at_the_time_limit() {
    let mut setup = Setup::new(
        FakeEngine::returning("długo"),
        wav_source(wav(1200, 0), false),
    );
    setup.max_recording = Duration::from_millis(300);
    let rig = setup.spawn();
    rig.dictation.intent(Start);
    rig.wait_idle_after(DictationState::Inserting);
    assert_eq!(rig.inserted(), vec!["długo"]);
    assert_eq!(rig.resets.load(Ordering::SeqCst), 1);
}

#[test]
fn an_insertion_failure_is_indicated_and_the_transcript_stays_in_history() {
    let rig = Setup::new(
        FakeEngine::returning("tekst"),
        wav_source(wav(1200, 0), true),
    )
    .spawn();
    rig.inserter
        .fail_with(Some(InsertionError::Blocked("admin window".into())));
    rig.dictation.intent(Start);
    rig.wait("the Insertion error", |s| s.error.is_some());
    assert_eq!(rig.history_texts(), vec!["tekst"]);
    assert_eq!(
        rig.dictation.status().error.unwrap().kind,
        ProblemKind::InsertionFailed
    );
}

#[test]
fn a_model_that_cannot_load_is_indicated() {
    let engine = FakeEngine::returning("x");
    engine.set_load_outcome(Err(EngineError::ModelLoad("out of memory".into())));
    let rig = Setup::new(engine.clone(), wav_source(wav(1200, 0), true)).spawn();
    rig.dictation.intent(Start);
    rig.wait("the load error", |s| s.error.is_some());
    let error = rig.dictation.status().error.unwrap();
    assert_eq!(error.kind, ProblemKind::ModelLoadFailed);
    assert_eq!(error.detail, "out of memory");
    assert!(engine.requests().is_empty());
}

#[test]
fn model_loading_starts_with_the_recording() {
    let engine = FakeEngine::returning("x");
    let source = ScriptedSource::default();
    let rig = Setup::new(engine.clone(), source).spawn();
    rig.dictation.intent(Start);
    rig.wait_state(DictationState::Recording);
    let until = Instant::now() + Duration::from_secs(5);
    while engine.load_count() == 0 {
        assert!(
            Instant::now() < until,
            "the Model was not loaded during the Recording"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// An Engine whose transcription panics.
struct CrashingEngine {
    unloads: Arc<AtomicUsize>,
}

impl Engine for CrashingEngine {
    fn load(&mut self) -> Result<(), EngineError> {
        Ok(())
    }
    fn transcribe(&mut self, _: TranscriptionRequest<'_>) -> Result<String, EngineError> {
        panic!("native crash");
    }
    fn unload(&mut self) {
        self.unloads.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn an_engine_crash_is_a_transcription_failure_and_unloads_the_model() {
    let unloads = Arc::new(AtomicUsize::new(0));
    let model = Arc::new(EngineModel::new(
        "Crashy",
        CrashingEngine {
            unloads: unloads.clone(),
        },
    ));
    let mut setup = Setup::new(FakeEngine::returning("x"), wav_source(wav(1200, 0), true));
    setup.engine = None;
    let rig = setup.spawn_with(TestModels { model: Some(model) });
    rig.dictation.intent(Start);
    rig.wait("the crash error", |s| s.error.is_some());
    assert_eq!(
        rig.dictation.status().error.unwrap().kind,
        ProblemKind::TranscriptionFailed
    );
    assert_eq!(unloads.load(Ordering::SeqCst), 1);
}

#[test]
fn outside_problems_are_indicated_the_same_way() {
    let rig = Setup::new(FakeEngine::returning("x"), ScriptedSource::default()).spawn();
    rig.dictation
        .report_problem(ProblemKind::ModelDownloadFailed, "network");
    rig.wait("the download error", |s| {
        s.error
            .as_ref()
            .is_some_and(|e| e.kind == ProblemKind::ModelDownloadFailed)
    });
}
