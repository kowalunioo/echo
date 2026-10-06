//! The **Microphone**: the real [`AudioSource`] a Recording listens to, and the choice of which
//! input device that is (`docs/specs/microphone.md`).
//!
//! Hardware sits behind the small [`InputDevices`] seam — list the devices, report the Windows
//! privacy switch, open one device for one Recording — so the selection and fallback logic in
//! [`Microphone`] is tested with [`FakeInputDevices`]. [`CpalInputDevices`] is the real one
//! (WASAPI through cpal).
//!
//! The pipeline gets a Microphone from [`Microphones::source`]: it reads the Microphone setting
//! afresh at every [`AudioSource::start`], so a change applies from the next Recording (rule 9)
//! and a change of the Windows default device is picked up without restarting Echo (rule 3).

pub mod commands;
mod cpal_devices;
mod fake;
mod privacy;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{AudioSink, AudioSource, AudioSourceError, AudioStream};

pub use cpal_devices::CpalInputDevices;
pub use fake::FakeInputDevices;
pub use privacy::{PrivacySwitches, access_from_switches};

/// The Microphone setting (`microphone.md` rules 1–2): follow the Windows default recording
/// device, or one specific device identified by its name as Windows reports it.
///
/// Stored as `{"kind": "default"}` or `{"kind": "device", "name": "Microphone (USB Audio)"}`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MicrophoneChoice {
    /// Whatever Windows considers the default recording device when a Recording starts.
    #[default]
    Default,
    /// One specific device, by name.
    Device { name: String },
}

/// Whether Windows privacy settings let desktop apps use the Microphone (rule 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MicrophoneAccess {
    Allowed,
    Denied,
}

/// The input devices present right now.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeviceList {
    /// Names of every input device, in the order Windows lists them.
    pub devices: Vec<String>,
    /// Name of the Windows default recording device, if there is one.
    pub default: Option<String>,
}

/// Something worth telling the user about a Recording that still went ahead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MicrophoneNotice {
    /// The selected device was not present, so this Recording uses the default device; the
    /// selection is kept (rule 5). The Overlay shows "Selected microphone not found — using the
    /// default microphone".
    #[serde(rename_all = "camelCase")]
    SelectedNotFound { selected: String, used: String },
}

/// The hardware seam under [`Microphone`]: everything that touches real audio devices.
pub trait InputDevices: Send + Sync {
    /// Whether Windows privacy settings allow desktop apps to use the Microphone.
    fn access(&self) -> MicrophoneAccess;
    /// A fresh list of the input devices present now (rule 8: never cached).
    fn list(&self) -> Result<DeviceList, AudioSourceError>;
    /// Opens the device called `name` for one Recording, with the [`AudioSource`] contract:
    /// audio goes to `sink` until the returned stream stops, a lost device ends with
    /// [`AudioEvent::Failed`](super::AudioEvent::Failed), and the device is released on stop.
    fn open(&self, name: &str, sink: AudioSink) -> Result<AudioStream, AudioSourceError>;
}

/// Which device a Recording uses, as decided by [`resolve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The device to open.
    pub device: String,
    /// Set when the selected device was missing and the default is used instead (rule 5).
    pub notice: Option<MicrophoneNotice>,
}

/// Picks the device for a Recording from the Microphone setting and the devices present now
/// (rules 3–6).
pub fn resolve(
    choice: &MicrophoneChoice,
    present: &DeviceList,
) -> Result<Resolved, AudioSourceError> {
    // Windows always names a default while any input device exists; should it ever not, the
    // first device listed stands in for it.
    let default = present
        .default
        .clone()
        .or_else(|| present.devices.first().cloned())
        .ok_or(AudioSourceError::NotFound)?;
    match choice {
        MicrophoneChoice::Default => Ok(Resolved {
            device: default,
            notice: None,
        }),
        MicrophoneChoice::Device { name } if present.devices.contains(name) => Ok(Resolved {
            device: name.clone(),
            notice: None,
        }),
        MicrophoneChoice::Device { name } => Ok(Resolved {
            notice: Some(MicrophoneNotice::SelectedNotFound {
                selected: name.clone(),
                used: default.clone(),
            }),
            device: default,
        }),
    }
}

/// The Microphone as an [`AudioSource`]. Every [`start`](AudioSource::start) checks the Windows
/// privacy switch, reads the Microphone setting, lists the devices, picks one with [`resolve`]
/// and opens it; nothing is held open between Recordings (rule 12).
pub struct Microphone {
    devices: Arc<dyn InputDevices>,
    choice: Box<dyn Fn() -> MicrophoneChoice + Send>,
    notices: Box<dyn Fn(MicrophoneNotice) + Send>,
}

impl Microphone {
    /// `choice` returns the current Microphone setting; `notices` receives a
    /// [`MicrophoneNotice`] when a Recording starts on a fallback device.
    pub fn new(
        devices: Arc<dyn InputDevices>,
        choice: impl Fn() -> MicrophoneChoice + Send + 'static,
        notices: impl Fn(MicrophoneNotice) + Send + 'static,
    ) -> Self {
        Self {
            devices,
            choice: Box::new(choice),
            notices: Box::new(notices),
        }
    }
}

impl AudioSource for Microphone {
    fn start(&mut self, sink: AudioSink) -> Result<AudioStream, AudioSourceError> {
        if self.devices.access() == MicrophoneAccess::Denied {
            return Err(AudioSourceError::AccessDenied);
        }
        let choice = (self.choice)();
        let resolved = resolve(&choice, &self.devices.list()?)?;
        log::info!(
            "opening microphone ({})",
            if resolved.notice.is_some() {
                "selected device missing, using the default"
            } else {
                match choice {
                    MicrophoneChoice::Default => "default device",
                    MicrophoneChoice::Device { .. } => "selected device",
                }
            }
        );
        let stream = self.devices.open(&resolved.device, sink)?;
        if let Some(notice) = resolved.notice {
            (self.notices)(notice);
        }
        Ok(stream)
    }
}

/// The input devices of this computer, kept in Tauri's managed state. It serves the Microphone
/// commands and hands the pipeline a [`Microphone`] for its Recordings.
#[derive(Clone)]
pub struct Microphones {
    devices: Arc<dyn InputDevices>,
}

impl Microphones {
    pub fn new(devices: Arc<dyn InputDevices>) -> Self {
        Self { devices }
    }

    /// The real devices (WASAPI).
    pub fn system() -> Self {
        Self::new(Arc::new(CpalInputDevices))
    }

    /// The device seam itself, for checks outside a Recording.
    pub fn devices(&self) -> &Arc<dyn InputDevices> {
        &self.devices
    }

    /// An [`AudioSource`] for Recordings. `choice` returns the current Microphone setting (e.g.
    /// `move || store.get().microphone`); `notices` forwards fallback notices to the Overlay.
    pub fn source(
        &self,
        choice: impl Fn() -> MicrophoneChoice + Send + 'static,
        notices: impl Fn(MicrophoneNotice) + Send + 'static,
    ) -> Microphone {
        Microphone::new(Arc::clone(&self.devices), choice, notices)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::super::{AudioEvent, AudioFormat, AudioFrames};
    use super::*;
    use crate::settings::{Settings, SettingsStore};

    fn list(devices: &[&str], default: Option<&str>) -> DeviceList {
        DeviceList {
            devices: devices.iter().map(|d| d.to_string()).collect(),
            default: default.map(str::to_owned),
        }
    }

    fn device(name: &str) -> MicrophoneChoice {
        MicrophoneChoice::Device { name: name.into() }
    }

    /// A Microphone over `fake` whose setting is `choice` and whose notices are collected.
    fn microphone(
        fake: &FakeInputDevices,
        choice: Arc<Mutex<MicrophoneChoice>>,
    ) -> (Microphone, Arc<Mutex<Vec<MicrophoneNotice>>>) {
        let notices = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&notices);
        let mic = Microphones::new(Arc::new(fake.clone())).source(
            move || choice.lock().unwrap().clone(),
            move |n| sink.lock().unwrap().push(n),
        );
        (mic, notices)
    }

    fn ignore() -> AudioSink {
        Box::new(|_| {})
    }

    #[test]
    fn the_default_setting_is_default() {
        assert_eq!(
            Settings::defaults(None).microphone,
            MicrophoneChoice::Default
        );
    }

    #[test]
    fn the_setting_is_stored_as_a_tagged_object() {
        assert_eq!(
            serde_json::to_value(device("Mic (USB)")).unwrap(),
            serde_json::json!({"kind": "device", "name": "Mic (USB)"})
        );
        assert_eq!(
            serde_json::to_value(MicrophoneChoice::Default).unwrap(),
            serde_json::json!({"kind": "default"})
        );
        assert!(serde_json::from_value::<MicrophoneChoice>(serde_json::json!("Mic")).is_err());
    }

    #[test]
    fn resolve_follows_the_rules() {
        let present = list(&["A", "B"], Some("A"));
        assert_eq!(
            resolve(&MicrophoneChoice::Default, &present)
                .unwrap()
                .device,
            "A"
        );
        assert_eq!(resolve(&device("B"), &present).unwrap().device, "B");
        let missing = resolve(&device("C"), &present).unwrap();
        assert_eq!(missing.device, "A");
        assert_eq!(
            missing.notice,
            Some(MicrophoneNotice::SelectedNotFound {
                selected: "C".into(),
                used: "A".into()
            })
        );
        assert_eq!(
            resolve(&MicrophoneChoice::Default, &list(&[], None)),
            Err(AudioSourceError::NotFound)
        );
        assert_eq!(
            resolve(&device("C"), &list(&[], None)),
            Err(AudioSourceError::NotFound)
        );
        assert_eq!(
            resolve(&MicrophoneChoice::Default, &list(&["B"], None))
                .unwrap()
                .device,
            "B"
        );
    }

    // microphone.md acceptance test 1.
    #[test]
    fn default_follows_the_windows_default_between_recordings() {
        let fake = FakeInputDevices::new(list(&["A", "B"], Some("A")));
        let (mut mic, notices) = microphone(&fake, Arc::new(Mutex::new(MicrophoneChoice::Default)));

        mic.start(ignore()).unwrap().stop();
        fake.set_devices(list(&["A", "B"], Some("B")));
        mic.start(ignore()).unwrap().stop();

        assert_eq!(fake.opened(), ["A", "B"]);
        assert!(notices.lock().unwrap().is_empty());
    }

    // microphone.md acceptance test 2.
    #[test]
    fn a_specific_device_is_used_even_when_it_is_not_the_default() {
        let fake = FakeInputDevices::new(list(&["A", "B"], Some("A")));
        let (mut mic, notices) = microphone(&fake, Arc::new(Mutex::new(device("B"))));
        mic.start(ignore()).unwrap().stop();
        assert_eq!(fake.opened(), ["B"]);
        assert!(notices.lock().unwrap().is_empty());
    }

    // microphone.md acceptance test 3, with the real settings store to show the selection stays.
    #[test]
    fn a_missing_device_falls_back_for_one_recording_and_keeps_the_selection() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) =
            SettingsStore::open(dir.path().join("settings.json"), Settings::defaults(None));
        let store = Arc::new(store);
        store.update(|s| s.microphone = device("C")).unwrap();

        let fake = FakeInputDevices::new(list(&["A", "B"], Some("A")));
        let notices = Arc::new(Mutex::new(Vec::new()));
        let (reader, collected) = (Arc::clone(&store), Arc::clone(&notices));
        let mut mic = Microphones::new(Arc::new(fake.clone())).source(
            move || reader.get().microphone,
            move |n| collected.lock().unwrap().push(n),
        );

        mic.start(ignore()).unwrap().stop();
        assert_eq!(fake.opened(), ["A"]);
        assert_eq!(
            *notices.lock().unwrap(),
            [MicrophoneNotice::SelectedNotFound {
                selected: "C".into(),
                used: "A".into()
            }]
        );
        assert_eq!(store.get().microphone, device("C"));

        fake.set_devices(list(&["A", "B", "C"], Some("A")));
        mic.start(ignore()).unwrap().stop();
        assert_eq!(fake.opened(), ["A", "C"]);
        assert_eq!(notices.lock().unwrap().len(), 1);
    }

    // microphone.md acceptance test 4.
    #[test]
    fn no_device_at_all_means_no_recording() {
        let fake = FakeInputDevices::new(list(&[], None));
        let (mut mic, _) = microphone(&fake, Arc::new(Mutex::new(MicrophoneChoice::Default)));
        assert_eq!(mic.start(ignore()).unwrap_err(), AudioSourceError::NotFound);
        assert!(fake.opened().is_empty());
    }

    // microphone.md acceptance test 8.
    #[test]
    fn a_privacy_block_stops_the_recording_before_any_device_is_opened() {
        let fake = FakeInputDevices::new(list(&["A"], Some("A")));
        fake.set_access(MicrophoneAccess::Denied);
        let (mut mic, _) = microphone(&fake, Arc::new(Mutex::new(MicrophoneChoice::Default)));
        assert_eq!(
            mic.start(ignore()).unwrap_err(),
            AudioSourceError::AccessDenied
        );
        assert!(fake.opened().is_empty());
    }

    #[test]
    fn an_open_failure_is_reported_with_its_cause() {
        let fake = FakeInputDevices::new(list(&["A"], Some("A")));
        fake.fail_next_open(AudioSourceError::Failed("busy".into()));
        let (mut mic, _) = microphone(&fake, Arc::new(Mutex::new(MicrophoneChoice::Default)));
        assert_eq!(
            mic.start(ignore()).unwrap_err(),
            AudioSourceError::Failed("busy".into())
        );
    }

    // Rule 9: the setting is read when a Recording starts; a change mid-Recording waits.
    #[test]
    fn a_change_of_microphone_applies_from_the_next_recording() {
        let fake = FakeInputDevices::new(list(&["A", "B"], Some("A")));
        let choice = Arc::new(Mutex::new(MicrophoneChoice::Default));
        let (mut mic, _) = microphone(&fake, Arc::clone(&choice));

        let stream = mic.start(ignore()).unwrap();
        *choice.lock().unwrap() = device("B");
        assert_eq!(fake.open_streams(), ["A"]);
        stream.stop();
        assert!(
            fake.open_streams().is_empty(),
            "device released after the Recording (rule 12)"
        );

        mic.start(ignore()).unwrap().stop();
        assert_eq!(fake.opened(), ["A", "B"]);
    }

    // microphone.md acceptance test 5 (Microphone side) and rule 11: audio before the loss is
    // delivered, the loss is terminal, and the next Recording opens the device afresh.
    #[test]
    fn after_a_device_loss_the_next_recording_opens_the_device_again() {
        let fake = FakeInputDevices::new(list(&["A"], Some("A")));
        let (mut mic, _) = microphone(&fake, Arc::new(Mutex::new(MicrophoneChoice::Default)));
        let events = Arc::new(Mutex::new(Vec::new()));
        let collected = Arc::clone(&events);

        let stream = mic
            .start(Box::new(move |e| collected.lock().unwrap().push(e)))
            .unwrap();
        let speech = AudioFrames {
            format: AudioFormat {
                sample_rate: 48_000,
                channels: 2,
            },
            samples: vec![0.25; 2 * 48_000 * 2],
        };
        fake.deliver(AudioEvent::Frames(speech.clone()));
        fake.deliver(AudioEvent::Failed(AudioSourceError::Disconnected));
        stream.stop();
        assert_eq!(
            *events.lock().unwrap(),
            [
                AudioEvent::Frames(speech),
                AudioEvent::Failed(AudioSourceError::Disconnected)
            ]
        );

        mic.start(ignore()).unwrap().stop();
        assert_eq!(fake.opened(), ["A", "A"]);
    }
}
