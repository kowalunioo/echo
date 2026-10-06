//! **Fake-microphone mode** (`dictation-pipeline.md` rules 40–46): a developer launch option that
//! makes every Dictation record from a WAV file through the [`WavAudioSource`] instead of the
//! Microphone. It is never offered in the normal UI.
//!
//! Selected by the `--fake-mic <file.wav>` command-line argument or the `ECHO_FAKE_MIC`
//! environment variable (the argument wins). Two test options:
//!
//! - `--fake-mic-stop-at-end` / `ECHO_FAKE_MIC_STOP_AT_END=1`: the Recording stops by itself when
//!   the file ends (rule 44);
//! - `--fake-mic-fast` / `ECHO_FAKE_MIC_FAST=1`: the file is delivered as fast as possible
//!   instead of in real time (rule 45).

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use super::{
    AudioSink, AudioSource, AudioSourceError, AudioStream, Pace, WavAudioSource, WavOptions,
};

/// Command-line argument naming the WAV file (`--fake-mic <file>` or `--fake-mic=<file>`).
pub const FAKE_MIC_ARG: &str = "--fake-mic";
/// Environment variable naming the WAV file.
pub const FAKE_MIC_ENV: &str = "ECHO_FAKE_MIC";
/// Argument for rule 44: stop the Recording at the end of the file.
pub const STOP_AT_END_ARG: &str = "--fake-mic-stop-at-end";
/// Environment variable for rule 44.
pub const STOP_AT_END_ENV: &str = "ECHO_FAKE_MIC_STOP_AT_END";
/// Argument for rule 45: deliver the file as fast as possible.
pub const FAST_ARG: &str = "--fake-mic-fast";
/// Environment variable for rule 45.
pub const FAST_ENV: &str = "ECHO_FAKE_MIC_FAST";

/// The fake-microphone launch option: which WAV file replaces the Microphone, and how it plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeMicrophone {
    pub path: PathBuf,
    pub options: WavOptions,
}

impl FakeMicrophone {
    /// Reads the launch option from the process arguments (program name first, as from
    /// [`std::env::args_os`]) and the environment. `None` means normal operation.
    pub fn from_launch(
        args: impl IntoIterator<Item = OsString>,
        env: impl Fn(&str) -> Option<OsString>,
    ) -> Option<Self> {
        let args: Vec<OsString> = args.into_iter().skip(1).collect();
        let path = path_argument(&args)
            .or_else(|| env(FAKE_MIC_ENV))
            .filter(|path| !path.is_empty())?;
        let flag = |arg: &str, var: &str| {
            args.iter().any(|a| a == arg) || env(var).is_some_and(|v| is_true(&v))
        };
        Some(Self {
            path: PathBuf::from(path),
            options: WavOptions {
                pace: if flag(FAST_ARG, FAST_ENV) {
                    Pace::AsFastAsPossible
                } else {
                    Pace::RealTime
                },
                stop_at_end: flag(STOP_AT_END_ARG, STOP_AT_END_ENV),
            },
        })
    }

    /// The file name the marker shows, e.g. `pl-proste.wav`.
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_string_lossy()
            .into_owned()
    }

    /// The Audio Source that replaces the Microphone. A file that cannot be read still gives a
    /// source — one whose every Recording fails with the reason — so Echo never falls back to
    /// the real Microphone while the marker says otherwise.
    pub fn source(&self) -> FakeMicrophoneSource {
        FakeMicrophoneSource(WavAudioSource::open(&self.path, self.options))
    }
}

/// The [`AudioSource`] of fake-microphone mode.
pub struct FakeMicrophoneSource(Result<WavAudioSource, AudioSourceError>);

impl FakeMicrophoneSource {
    /// Why the WAV file cannot be played, if it cannot.
    pub fn error(&self) -> Option<&AudioSourceError> {
        self.0.as_ref().err()
    }
}

impl AudioSource for FakeMicrophoneSource {
    fn start(&mut self, sink: AudioSink) -> Result<AudioStream, AudioSourceError> {
        match &mut self.0 {
            Ok(source) => source.start(sink),
            Err(error) => Err(error.clone()),
        }
    }
}

fn path_argument(args: &[OsString]) -> Option<OsString> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == FAKE_MIC_ARG {
            return iter.next().cloned();
        }
        if let Some(value) = arg
            .to_str()
            .and_then(|a| a.strip_prefix(FAKE_MIC_ARG))
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Some(value.into());
        }
    }
    None
}

fn is_true(value: &OsStr) -> bool {
    !matches!(
        value.to_string_lossy().trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off"
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn launch(args: &[&str], env: &[(&str, &str)]) -> Option<FakeMicrophone> {
        let env: HashMap<String, OsString> = env
            .iter()
            .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
            .collect();
        FakeMicrophone::from_launch(
            std::iter::once("echo.exe")
                .chain(args.iter().copied())
                .map(OsString::from),
            move |key| env.get(key).cloned(),
        )
    }

    #[test]
    fn normal_launch_uses_the_microphone() {
        assert_eq!(launch(&[], &[]), None);
        assert_eq!(launch(&["--autostart"], &[]), None);
        assert_eq!(launch(&[], &[(FAKE_MIC_ENV, "")]), None);
    }

    #[test]
    fn the_argument_names_the_wav_file_and_plays_in_real_time() {
        let fake = launch(&["--fake-mic", r"C:\audio\pl-proste.wav"], &[]).unwrap();
        assert_eq!(fake.path, PathBuf::from(r"C:\audio\pl-proste.wav"));
        assert_eq!(fake.options, WavOptions::default());
        assert_eq!(fake.options.pace, Pace::RealTime);
        assert!(!fake.options.stop_at_end);
        assert_eq!(fake.file_name(), "pl-proste.wav");
    }

    #[test]
    fn the_argument_accepts_the_equals_form() {
        let fake = launch(&["--fake-mic=D:/x/cisza.wav"], &[]).unwrap();
        assert_eq!(fake.path, PathBuf::from("D:/x/cisza.wav"));
    }

    #[test]
    fn the_environment_variable_names_the_wav_file() {
        let fake = launch(&[], &[(FAKE_MIC_ENV, "en-proste.wav")]).unwrap();
        assert_eq!(fake.path, PathBuf::from("en-proste.wav"));
    }

    #[test]
    fn the_argument_wins_over_the_environment() {
        let fake = launch(&["--fake-mic", "a.wav"], &[(FAKE_MIC_ENV, "b.wav")]).unwrap();
        assert_eq!(fake.path, PathBuf::from("a.wav"));
    }

    #[test]
    fn a_dangling_argument_is_ignored() {
        assert_eq!(launch(&["--fake-mic"], &[]), None);
    }

    #[test]
    fn test_options_come_from_arguments_or_environment() {
        let fake = launch(&["--fake-mic", "a.wav", STOP_AT_END_ARG, FAST_ARG], &[]).unwrap();
        assert!(fake.options.stop_at_end);
        assert_eq!(fake.options.pace, Pace::AsFastAsPossible);

        let fake = launch(
            &[],
            &[
                (FAKE_MIC_ENV, "a.wav"),
                (STOP_AT_END_ENV, "1"),
                (FAST_ENV, "true"),
            ],
        )
        .unwrap();
        assert!(fake.options.stop_at_end);
        assert_eq!(fake.options.pace, Pace::AsFastAsPossible);

        let fake = launch(
            &[],
            &[
                (FAKE_MIC_ENV, "a.wav"),
                (STOP_AT_END_ENV, "0"),
                (FAST_ENV, ""),
            ],
        )
        .unwrap();
        assert_eq!(fake.options, WavOptions::default());
    }

    #[test]
    fn an_unreadable_file_fails_every_recording_instead_of_using_the_microphone() {
        let fake = launch(&["--fake-mic", "Z:/does/not/exist.wav"], &[]).unwrap();
        let mut source = fake.source();
        assert!(source.error().is_some());
        let error = source.start(Box::new(|_| {})).unwrap_err();
        assert!(
            matches!(&error, AudioSourceError::Failed(detail) if detail.contains("exist.wav")),
            "{error:?}"
        );
    }

    #[test]
    fn a_readable_file_plays_from_its_beginning_at_every_recording() {
        let path = std::env::temp_dir().join(format!("echo-fake-mic-{}.wav", std::process::id()));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..1_600 {
            writer.write_sample(i16::MAX / 2).unwrap();
        }
        writer.finalize().unwrap();

        let fake = launch(
            &[FAST_ARG, STOP_AT_END_ARG],
            &[(FAKE_MIC_ENV, path.to_str().unwrap())],
        )
        .unwrap();
        let mut source = fake.source();
        let _ = std::fs::remove_file(&path);
        assert_eq!(source.error(), None);
        for _ in 0..2 {
            let (tx, rx) = std::sync::mpsc::channel();
            let stream = source
                .start(Box::new(move |event| {
                    let _ = tx.send(event);
                }))
                .unwrap();
            let mut frames = 0;
            loop {
                match rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
                    super::super::AudioEvent::Frames(piece) => frames += piece.samples.len(),
                    super::super::AudioEvent::Ended => break,
                    other => panic!("{other:?}"),
                }
            }
            stream.stop();
            assert_eq!(frames, 1_600);
        }
    }
}
