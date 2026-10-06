use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::{DeviceList, InputDevices, MicrophoneAccess};
use crate::audio::{AudioEvent, AudioSink, AudioSourceError, AudioStream};

/// Scripted [`InputDevices`] for tests: a device list and privacy answer that a test changes at
/// will, a record of every device opened, and a way to push events into the open stream.
///
/// Clones share state, so a test keeps one clone to script and inspect the fake while the code
/// under test owns another.
#[derive(Clone)]
pub struct FakeInputDevices {
    state: Arc<Mutex<State>>,
}

struct State {
    devices: DeviceList,
    access: MicrophoneAccess,
    next_open_error: Option<AudioSourceError>,
    opened: Vec<String>,
    /// Open streams: device name and the sink of that Recording.
    streams: Vec<(u64, String, AudioSink)>,
    next_id: u64,
}

impl FakeInputDevices {
    pub fn new(devices: DeviceList) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                devices,
                access: MicrophoneAccess::Allowed,
                next_open_error: None,
                opened: Vec::new(),
                streams: Vec::new(),
                next_id: 0,
            })),
        }
    }

    /// Changes the devices present (plugging, unplugging, a new Windows default).
    pub fn set_devices(&self, devices: DeviceList) {
        self.lock().devices = devices;
    }

    /// Changes the answer of the Windows privacy check.
    pub fn set_access(&self, access: MicrophoneAccess) {
        self.lock().access = access;
    }

    /// Makes the next [`InputDevices::open`] fail with `error`.
    pub fn fail_next_open(&self, error: AudioSourceError) {
        self.lock().next_open_error = Some(error);
    }

    /// Every device opened so far, in order.
    pub fn opened(&self) -> Vec<String> {
        self.lock().opened.clone()
    }

    /// Devices whose stream has not been stopped yet.
    pub fn open_streams(&self) -> Vec<String> {
        self.lock()
            .streams
            .iter()
            .map(|(_, name, _)| name.clone())
            .collect()
    }

    /// Sends `event` to every open stream's sink, as the device would.
    pub fn deliver(&self, event: AudioEvent) {
        for (_, _, sink) in &mut self.lock().streams {
            sink(event.clone());
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl InputDevices for FakeInputDevices {
    fn access(&self) -> MicrophoneAccess {
        self.lock().access
    }

    fn list(&self) -> Result<DeviceList, AudioSourceError> {
        Ok(self.lock().devices.clone())
    }

    fn open(&self, name: &str, sink: AudioSink) -> Result<AudioStream, AudioSourceError> {
        let mut state = self.lock();
        if let Some(error) = state.next_open_error.take() {
            return Err(error);
        }
        if !state.devices.devices.iter().any(|d| d == name) {
            return Err(AudioSourceError::NotFound);
        }
        state.opened.push(name.to_owned());
        let id = state.next_id;
        state.next_id += 1;
        state.streams.push((id, name.to_owned(), sink));
        let shared = Arc::clone(&self.state);
        Ok(AudioStream::new(move || {
            let mut state = shared.lock().unwrap_or_else(PoisonError::into_inner);
            state.streams.retain(|(stream, _, _)| *stream != id);
        }))
    }
}
