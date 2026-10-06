//! The Dictation state machine (`dictation-pipeline.md` rules 1–3, 12, 18, 20, 23, 27–30, 38–39)
//! as pure logic: [`Machine::handle`] takes what happened and returns what the runtime must do.
//! No threads, no clock, no devices — the runtime (`runtime.rs`) does those and reports back.

use crate::shortcut::modes::RecordIntent;

use super::DictationState;

/// Something that happened, reported to the [`Machine`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// The Record Shortcut asked to start or stop.
    Intent(RecordIntent),
    /// Cancellation from any source (the Cancel Shortcut, #15).
    Cancel,
    /// The runtime could not start the Recording it was told to start: no Model active, or the
    /// Audio Source failed to open (rules 5, 8). The runtime reports the problem itself.
    StartFailed,
    /// The first audio of the Recording arrived (rule 7: "getting ready" → "listening").
    AudioArrived,
    /// The Recording ended without the shortcut: the source ended or was lost, or the 10-minute
    /// limit was reached (rule 12, `microphone.md` rule 10). A normal stop, not a Cancellation.
    RecordingEnded,
    /// After the stop, no speech audio was kept, so the Engine is not run (rule 18).
    NoSpeech,
    /// The Engine finished: the cleaned Transcript, or the error detail (rule 23).
    Transcribed(Result<String, String>),
    /// Insertion finished, successfully or not (rule 37).
    Inserted,
}

/// What the runtime must do, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Begin a Dictation: take the active Model, start loading it, open the Audio Source.
    /// Answered with [`Input::AudioArrived`], [`Input::StartFailed`] or later inputs.
    StartRecording,
    /// Stop the Audio Source and transcribe what was kept (or answer [`Input::NoSpeech`]).
    StopRecording,
    /// Stop the Audio Source and throw the audio away (Cancellation during Recording).
    DiscardRecording,
    /// Add the Transcript to History, then insert it (rule 38). Answered with
    /// [`Input::Inserted`].
    StoreAndInsert(String),
    /// The Dictation is over: release the Model and record the outcome for error indication.
    Finish(Outcome),
    /// Tell the Record Shortcut the Recording ended without it, so its next press starts afresh.
    ResetShortcut,
}

/// How a Dictation ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The Transcript reached History and Insertion ran (the runtime knows whether it succeeded).
    Inserted,
    /// Silence or an empty Transcript (rules 18, 20). Not an error.
    NoSpeech,
    /// Cancelled. Not an error.
    Cancelled,
    /// The Engine failed with this detail (rule 23).
    TranscriptionFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Recording {
        listening: bool,
    },
    /// `cancelled`: the user cancelled, so the late Engine result is thrown away. To the user
    /// this already looks Idle; internally the Model is still busy until the Engine returns.
    Transcribing {
        cancelled: bool,
    },
    Inserting,
}

/// The one Dictation of the app (rule 3) and the remembered busy request (rules 27–30).
#[derive(Debug)]
pub struct Machine {
    phase: Phase,
    /// A Record Shortcut start that arrived while busy, to run when the Dictation ends.
    remembered: bool,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            remembered: false,
        }
    }

    /// The state the user sees (Overlay, tray).
    pub fn state(&self) -> DictationState {
        match self.phase {
            Phase::Idle | Phase::Transcribing { cancelled: true } => DictationState::Idle,
            Phase::Recording { .. } => DictationState::Recording,
            Phase::Transcribing { cancelled: false } => DictationState::Transcribing,
            Phase::Inserting => DictationState::Inserting,
        }
    }

    /// Whether audio has arrived in the current Recording (rule 7).
    pub fn listening(&self) -> bool {
        matches!(self.phase, Phase::Recording { listening: true })
    }

    /// Whether a busy-period start request is remembered (rule 27).
    pub fn has_remembered_start(&self) -> bool {
        self.remembered
    }

    pub fn handle(&mut self, input: Input) -> Vec<Command> {
        use Command::*;
        match (self.phase, input) {
            (Phase::Idle, Input::Intent(RecordIntent::Start)) => {
                self.phase = Phase::Recording { listening: false };
                vec![StartRecording]
            }
            (Phase::Recording { .. }, Input::Intent(RecordIntent::Stop)) => {
                self.phase = Phase::Transcribing { cancelled: false };
                vec![StopRecording]
            }
            (Phase::Recording { .. }, Input::RecordingEnded) => {
                self.phase = Phase::Transcribing { cancelled: false };
                vec![StopRecording, ResetShortcut]
            }
            (Phase::Recording { .. }, Input::AudioArrived) => {
                self.phase = Phase::Recording { listening: true };
                vec![]
            }
            (Phase::Recording { .. }, Input::StartFailed) => {
                self.phase = Phase::Idle;
                vec![ResetShortcut]
            }
            // Busy (rules 27–29): in both modes the Record Shortcut sends Start for the press
            // that should begin a Recording and Stop for the press/release that undoes it.
            (Phase::Transcribing { .. } | Phase::Inserting, Input::Intent(intent)) => {
                self.remembered = intent == RecordIntent::Start;
                vec![]
            }
            (_, Input::Cancel) => {
                // Rule 30: any Cancellation forgets the remembered request.
                self.remembered = false;
                match self.phase {
                    Phase::Recording { .. } => {
                        self.phase = Phase::Idle;
                        vec![DiscardRecording, ResetShortcut, Finish(Outcome::Cancelled)]
                    }
                    Phase::Transcribing { .. } => {
                        self.phase = Phase::Transcribing { cancelled: true };
                        vec![ResetShortcut]
                    }
                    // Once Inserting has begun Cancellation has no effect (rule 39) beyond
                    // forgetting the remembered request.
                    Phase::Inserting => vec![ResetShortcut],
                    Phase::Idle => vec![],
                }
            }
            (Phase::Transcribing { cancelled }, Input::NoSpeech) => {
                let outcome = if cancelled {
                    Outcome::Cancelled
                } else {
                    Outcome::NoSpeech
                };
                self.end(outcome)
            }
            (Phase::Transcribing { cancelled: true }, Input::Transcribed(_)) => {
                self.end(Outcome::Cancelled)
            }
            (Phase::Transcribing { cancelled: false }, Input::Transcribed(Ok(text))) => {
                if text.trim().is_empty() {
                    self.end(Outcome::NoSpeech)
                } else {
                    self.phase = Phase::Inserting;
                    vec![StoreAndInsert(text)]
                }
            }
            (Phase::Transcribing { cancelled: false }, Input::Transcribed(Err(detail))) => {
                self.end(Outcome::TranscriptionFailed(detail))
            }
            (Phase::Inserting, Input::Inserted) => self.end(Outcome::Inserted),
            // Everything else cannot happen in that state or means nothing there (a stop with
            // nothing recording, a late report from a finished Recording).
            _ => vec![],
        }
    }

    /// Back to Idle; a remembered start begins the next Recording right away (rule 27).
    fn end(&mut self, outcome: Outcome) -> Vec<Command> {
        let mut commands = vec![Command::Finish(outcome)];
        self.phase = Phase::Idle;
        if std::mem::take(&mut self.remembered) {
            self.phase = Phase::Recording { listening: false };
            commands.push(Command::StartRecording);
        }
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Command::*;
    use RecordIntent::{Start, Stop};

    fn recording() -> Machine {
        let mut m = Machine::new();
        assert_eq!(m.handle(Input::Intent(Start)), vec![StartRecording]);
        m
    }

    fn transcribing() -> Machine {
        let mut m = recording();
        assert_eq!(m.handle(Input::Intent(Stop)), vec![StopRecording]);
        m
    }

    fn inserting() -> Machine {
        let mut m = transcribing();
        assert_eq!(
            m.handle(Input::Transcribed(Ok("tekst".into()))),
            vec![StoreAndInsert("tekst".into())]
        );
        m
    }

    #[test]
    fn a_full_dictation_goes_through_every_state() {
        let mut m = Machine::new();
        assert_eq!(m.state(), DictationState::Idle);
        m.handle(Input::Intent(Start));
        assert_eq!(m.state(), DictationState::Recording);
        assert!(!m.listening());
        m.handle(Input::AudioArrived);
        assert!(m.listening());
        m.handle(Input::Intent(Stop));
        assert_eq!(m.state(), DictationState::Transcribing);
        m.handle(Input::Transcribed(Ok("Ala ma kota".into())));
        assert_eq!(m.state(), DictationState::Inserting);
        assert_eq!(m.handle(Input::Inserted), vec![Finish(Outcome::Inserted)]);
        assert_eq!(m.state(), DictationState::Idle);
    }

    #[test]
    fn a_failed_start_returns_to_idle_and_resets_the_shortcut() {
        let mut m = recording();
        assert_eq!(m.handle(Input::StartFailed), vec![ResetShortcut]);
        assert_eq!(m.state(), DictationState::Idle);
    }

    #[test]
    fn a_recording_that_ends_by_itself_is_transcribed() {
        let mut m = recording();
        assert_eq!(
            m.handle(Input::RecordingEnded),
            vec![StopRecording, ResetShortcut]
        );
        assert_eq!(m.state(), DictationState::Transcribing);
    }

    #[test]
    fn no_speech_and_empty_transcripts_end_quietly() {
        let mut m = transcribing();
        assert_eq!(m.handle(Input::NoSpeech), vec![Finish(Outcome::NoSpeech)]);
        let mut m = transcribing();
        assert_eq!(
            m.handle(Input::Transcribed(Ok("  ".into()))),
            vec![Finish(Outcome::NoSpeech)]
        );
        assert_eq!(m.state(), DictationState::Idle);
    }

    #[test]
    fn an_engine_error_ends_the_dictation_as_failed() {
        let mut m = transcribing();
        assert_eq!(
            m.handle(Input::Transcribed(Err("boom".into()))),
            vec![Finish(Outcome::TranscriptionFailed("boom".into()))]
        );
        assert_eq!(m.state(), DictationState::Idle);
    }

    #[test]
    fn a_second_start_while_recording_is_ignored() {
        let mut m = recording();
        assert!(m.handle(Input::Intent(Start)).is_empty());
        assert_eq!(m.state(), DictationState::Recording);
    }

    #[test]
    fn a_stop_while_idle_is_ignored() {
        assert!(Machine::new().handle(Input::Intent(Stop)).is_empty());
    }

    #[test]
    fn cancelling_a_recording_discards_it() {
        let mut m = recording();
        assert_eq!(
            m.handle(Input::Cancel),
            vec![DiscardRecording, ResetShortcut, Finish(Outcome::Cancelled)]
        );
        assert_eq!(m.state(), DictationState::Idle);
    }

    #[test]
    fn acceptance_12_a_late_transcript_after_cancellation_is_thrown_away() {
        let mut m = transcribing();
        assert_eq!(m.handle(Input::Cancel), vec![ResetShortcut]);
        assert_eq!(m.state(), DictationState::Idle);
        assert_eq!(
            m.handle(Input::Transcribed(Ok("late".into()))),
            vec![Finish(Outcome::Cancelled)]
        );
    }

    #[test]
    fn cancelling_while_inserting_has_no_effect() {
        let mut m = inserting();
        assert_eq!(m.handle(Input::Cancel), vec![ResetShortcut]);
        assert_eq!(m.state(), DictationState::Inserting);
        assert_eq!(m.handle(Input::Inserted), vec![Finish(Outcome::Inserted)]);
    }

    #[test]
    fn a_start_while_transcribing_runs_after_the_dictation_ends() {
        let mut m = transcribing();
        assert!(m.handle(Input::Intent(Start)).is_empty());
        assert!(m.has_remembered_start());
        m.handle(Input::Transcribed(Ok("tekst".into())));
        assert_eq!(
            m.handle(Input::Inserted),
            vec![Finish(Outcome::Inserted), StartRecording]
        );
        assert_eq!(m.state(), DictationState::Recording);
    }

    #[test]
    fn a_start_while_inserting_is_remembered_too() {
        let mut m = inserting();
        m.handle(Input::Intent(Start));
        assert_eq!(
            m.handle(Input::Inserted),
            vec![Finish(Outcome::Inserted), StartRecording]
        );
    }

    #[test]
    fn a_remembered_start_also_follows_failures_and_silence() {
        let mut m = transcribing();
        m.handle(Input::Intent(Start));
        assert_eq!(
            m.handle(Input::Transcribed(Err("x".into()))),
            vec![
                Finish(Outcome::TranscriptionFailed("x".into())),
                StartRecording
            ]
        );
        let mut m = transcribing();
        m.handle(Input::Intent(Start));
        assert_eq!(
            m.handle(Input::NoSpeech),
            vec![Finish(Outcome::NoSpeech), StartRecording]
        );
    }

    #[test]
    fn toggle_two_busy_presses_cancel_out_and_a_third_is_remembered_again() {
        let mut m = transcribing();
        m.handle(Input::Intent(Start));
        m.handle(Input::Intent(Stop));
        assert!(!m.has_remembered_start());
        m.handle(Input::Intent(Start));
        assert!(m.has_remembered_start());
    }

    #[test]
    fn push_to_talk_release_while_busy_forgets_and_a_held_key_starts_then_stops() {
        // Released before the end: forgotten.
        let mut m = transcribing();
        m.handle(Input::Intent(Start));
        m.handle(Input::Intent(Stop));
        assert_eq!(m.handle(Input::NoSpeech), vec![Finish(Outcome::NoSpeech)]);
        assert_eq!(m.state(), DictationState::Idle);
        // Still held at the end: the Recording starts and the release stops it.
        let mut m = transcribing();
        m.handle(Input::Intent(Start));
        m.handle(Input::NoSpeech);
        assert_eq!(m.state(), DictationState::Recording);
        assert_eq!(m.handle(Input::Intent(Stop)), vec![StopRecording]);
    }

    #[test]
    fn cancellation_while_busy_forgets_the_remembered_start() {
        let mut m = inserting();
        m.handle(Input::Intent(Start));
        m.handle(Input::Cancel);
        assert_eq!(m.handle(Input::Inserted), vec![Finish(Outcome::Inserted)]);
        assert_eq!(m.state(), DictationState::Idle);
    }

    #[test]
    fn a_start_after_cancelling_a_transcription_waits_for_the_engine() {
        let mut m = transcribing();
        m.handle(Input::Cancel);
        assert!(m.handle(Input::Intent(Start)).is_empty());
        assert_eq!(
            m.handle(Input::Transcribed(Ok("late".into()))),
            vec![Finish(Outcome::Cancelled), StartRecording]
        );
    }

    #[test]
    fn late_reports_from_an_old_recording_are_ignored() {
        let mut m = Machine::new();
        for input in [
            Input::AudioArrived,
            Input::RecordingEnded,
            Input::StartFailed,
            Input::NoSpeech,
            Input::Inserted,
            Input::Transcribed(Ok("x".into())),
        ] {
            assert!(m.handle(input).is_empty());
            assert_eq!(m.state(), DictationState::Idle);
        }
    }
}
