//! The real [`InputDevices`]: Windows audio input devices through cpal (WASAPI, shared mode).
//!
//! Each Recording gets its own capture thread. It opens the device in its native format, keeps
//! the cpal stream alive until the [`AudioStream`] is stopped, and watches for a device that
//! goes quiet without reporting an error. Dropping the cpal stream releases the device, so the
//! Windows "microphone in use" indicator shows only during a Recording (rule 12).

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

use super::privacy::{access_from_switches, read_switches};
use super::{DeviceList, InputDevices, MicrophoneAccess};
use crate::audio::{
    AudioEvent, AudioFormat, AudioFrames, AudioSink, AudioSourceError, AudioStream,
};

/// A device that has not delivered its first audio within this time counts as lost. Generous,
/// because Bluetooth headsets can take a few seconds to switch into their microphone profile.
const FIRST_AUDIO_TIMEOUT: Duration = Duration::from_secs(5);
/// A device that delivered audio and then stays silent (no buffers at all, not quiet audio) for
/// this long counts as lost (rule 10). WASAPI normally delivers a buffer every 10 ms.
const STALL_TIMEOUT: Duration = Duration::from_secs(1);
/// How often the capture thread checks for a stalled device.
const WATCH_INTERVAL: Duration = Duration::from_millis(100);

/// Windows audio input devices (WASAPI through cpal).
#[derive(Debug, Clone, Copy, Default)]
pub struct CpalInputDevices;

impl InputDevices for CpalInputDevices {
    fn access(&self) -> MicrophoneAccess {
        access_from_switches(&read_switches())
    }

    fn list(&self) -> Result<DeviceList, AudioSourceError> {
        let host = cpal::default_host();
        let devices = host
            .input_devices()
            .map_err(|e| AudioSourceError::Failed(format!("cannot list microphones: {e}")))?
            .filter_map(|device| device_name(&device))
            .collect();
        let default = host.default_input_device().as_ref().and_then(device_name);
        Ok(DeviceList { devices, default })
    }

    fn open(&self, name: &str, sink: AudioSink) -> Result<AudioStream, AudioSourceError> {
        let name = name.to_owned();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let worker = thread::Builder::new()
            .name("microphone".into())
            .spawn(move || capture(&name, sink, &ready_tx, &stop_rx))
            .map_err(|e| AudioSourceError::Failed(format!("cannot start the microphone: {e}")))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(AudioStream::new(move || {
                let _ = stop_tx.send(());
                if worker.thread().id() != thread::current().id() {
                    let _ = worker.join();
                }
            })),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                Err(AudioSourceError::Failed(
                    "the microphone thread ended unexpectedly".into(),
                ))
            }
        }
    }
}

fn device_name(device: &cpal::Device) -> Option<String> {
    device.description().ok().map(|d| d.name().to_owned())
}

/// The capture thread of one Recording.
fn capture(
    name: &str,
    sink: AudioSink,
    ready: &mpsc::SyncSender<Result<(), AudioSourceError>>,
    stop: &mpsc::Receiver<()>,
) {
    let delivery = Arc::new(Mutex::new(Delivery::new(sink, Instant::now())));
    let stream = match open_stream(name, &delivery) {
        Ok(stream) => stream,
        Err(error) => {
            // The contract: a failed start never calls the sink.
            lock(&delivery).close();
            let _ = ready.send(Err(error));
            return;
        }
    };
    lock(&delivery).restart_clock(Instant::now());
    let _ = ready.send(Ok(()));

    // Runs until stopped, or until the AudioStream is dropped (the sender disconnects).
    while let Err(RecvTimeoutError::Timeout) = stop.recv_timeout(WATCH_INTERVAL) {
        lock(&delivery).check_stall(Instant::now());
    }
    // Releases the device; cpal returns once its audio thread has finished its last callback.
    drop(stream);
    lock(&delivery).close();
}

fn open_stream(
    name: &str,
    delivery: &Arc<Mutex<Delivery>>,
) -> Result<cpal::Stream, AudioSourceError> {
    let host = cpal::default_host();
    // A specific endpoint, never cpal's "default device" handle: that one follows the Windows
    // default and would end the stream if the default changed mid-Recording (rule 9).
    let device = host
        .input_devices()
        .map_err(|e| AudioSourceError::Failed(format!("cannot list microphones: {e}")))?
        .find(|device| device_name(device).as_deref() == Some(name))
        .ok_or(AudioSourceError::NotFound)?;
    let config = device.default_input_config().map_err(open_error)?;
    let format = AudioFormat {
        sample_rate: config.sample_rate(),
        channels: config.channels(),
    };
    lock(delivery).format = format;

    let stream_config = config.config();
    let stream = match config.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, stream_config, delivery),
        SampleFormat::I16 => build::<i16>(&device, stream_config, delivery),
        SampleFormat::I32 => build::<i32>(&device, stream_config, delivery),
        SampleFormat::F64 => build::<f64>(&device, stream_config, delivery),
        SampleFormat::U16 => build::<u16>(&device, stream_config, delivery),
        SampleFormat::U8 => build::<u8>(&device, stream_config, delivery),
        SampleFormat::I8 => build::<i8>(&device, stream_config, delivery),
        SampleFormat::U32 => build::<u32>(&device, stream_config, delivery),
        other => {
            return Err(AudioSourceError::Failed(format!(
                "unsupported microphone sample format {other:?}"
            )));
        }
    }
    .map_err(open_error)?;
    stream.play().map_err(open_error)?;
    log::info!(
        "microphone open: {} Hz, {} channel(s), {:?}",
        format.sample_rate,
        format.channels,
        config.sample_format()
    );
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    delivery: &Arc<Mutex<Delivery>>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let on_data = Arc::clone(delivery);
    let on_error = Arc::clone(delivery);
    device.build_input_stream::<T, _, _>(
        config,
        move |data: &[T], _| {
            if !data.is_empty() {
                let samples = data.iter().map(|&s| s.to_sample::<f32>()).collect();
                lock(&on_data).audio(samples, Instant::now());
            }
        },
        move |error| lock(&on_error).stream_error(&error),
        None,
    )
}

/// Maps a failure to open the device to the causes the user is told about
/// (`dictation-pipeline.md` rule 8).
fn open_error(error: cpal::Error) -> AudioSourceError {
    // E_ACCESSDENIED: WASAPI's answer when the privacy settings block the Microphone.
    let access_denied = error
        .message()
        .is_some_and(|m| m.contains("0x80070005") || m.to_lowercase().contains("access is denied"));
    match error.kind() {
        cpal::ErrorKind::PermissionDenied => AudioSourceError::AccessDenied,
        _ if access_denied => AudioSourceError::AccessDenied,
        cpal::ErrorKind::DeviceNotAvailable => AudioSourceError::NotFound,
        cpal::ErrorKind::DeviceBusy => AudioSourceError::Failed(format!(
            "the microphone is in use by another application ({error})"
        )),
        _ => AudioSourceError::Failed(error.to_string()),
    }
}

fn lock(delivery: &Mutex<Delivery>) -> MutexGuard<'_, Delivery> {
    delivery.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Hands one Recording's audio to its sink and enforces the end of the contract: after the
/// terminal [`AudioEvent::Failed`] or [`close`](Delivery::close), nothing more is delivered.
struct Delivery {
    sink: Option<AudioSink>,
    format: AudioFormat,
    /// When the stream started; the reference for the first-audio timeout.
    started: Instant,
    last_audio: Option<Instant>,
}

impl Delivery {
    fn new(sink: AudioSink, now: Instant) -> Self {
        Self {
            sink: Some(sink),
            format: AudioFormat {
                sample_rate: 48_000,
                channels: 1,
            },
            started: now,
            last_audio: None,
        }
    }

    /// Starts the first-audio timeout again, once the stream is actually playing.
    fn restart_clock(&mut self, now: Instant) {
        self.started = now;
    }

    fn audio(&mut self, samples: Vec<f32>, now: Instant) {
        if let Some(sink) = &mut self.sink {
            self.last_audio = Some(now);
            sink(AudioEvent::Frames(AudioFrames {
                format: self.format,
                samples,
            }));
        }
    }

    fn fail(&mut self, error: AudioSourceError) {
        if let Some(mut sink) = self.sink.take() {
            sink(AudioEvent::Failed(error));
        }
    }

    /// Handles an error the cpal stream reports (rule 10). A buffer glitch only costs a few
    /// samples, so the Recording continues; a device that really went away is still caught by
    /// its error or by [`check_stall`](Self::check_stall).
    fn stream_error(&mut self, error: &cpal::Error) {
        if error.kind() == cpal::ErrorKind::Xrun {
            log::debug!("microphone buffer glitch: {error}");
            return;
        }
        log::warn!("microphone stream error: {error}");
        self.fail(AudioSourceError::Disconnected);
    }

    /// Reports a device that stopped delivering audio without an error (rule 10).
    fn check_stall(&mut self, now: Instant) {
        let (since, limit) = match self.last_audio {
            Some(last) => (last, STALL_TIMEOUT),
            None => (self.started, FIRST_AUDIO_TIMEOUT),
        };
        if now.saturating_duration_since(since) >= limit {
            log::warn!("microphone delivered no audio for {limit:?}");
            self.fail(AudioSourceError::Disconnected);
        }
    }

    fn close(&mut self) {
        self.sink = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{AudioSource, SpeechConverter};

    fn recording() -> (Delivery, Arc<Mutex<Vec<AudioEvent>>>, Instant) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let now = Instant::now();
        let mut delivery = Delivery::new(Box::new(move |e| sink.lock().unwrap().push(e)), now);
        delivery.format = AudioFormat {
            sample_rate: 48_000,
            channels: 2,
        };
        (delivery, events, now)
    }

    #[test]
    fn audio_is_delivered_in_the_native_format() {
        let (mut delivery, events, now) = recording();
        delivery.audio(vec![0.5, -0.5], now);
        assert_eq!(
            *events.lock().unwrap(),
            [AudioEvent::Frames(AudioFrames {
                format: AudioFormat {
                    sample_rate: 48_000,
                    channels: 2
                },
                samples: vec![0.5, -0.5]
            })]
        );
    }

    // Rule 10: a stream error ends the Recording once; audio after it is not delivered.
    #[test]
    fn a_device_error_is_terminal() {
        let (mut delivery, events, now) = recording();
        delivery.audio(vec![0.1; 4], now);
        delivery.fail(AudioSourceError::Disconnected);
        delivery.fail(AudioSourceError::Disconnected);
        delivery.audio(vec![0.1; 4], now);
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[1],
            AudioEvent::Failed(AudioSourceError::Disconnected)
        );
    }

    // Rule 10: a buffer glitch is not a lost device. Some USB interfaces (Universal Audio Volt)
    // report one right after the stream starts.
    #[test]
    fn a_buffer_glitch_does_not_end_the_recording() {
        let (mut delivery, events, now) = recording();
        delivery.stream_error(&cpal::Error::new(cpal::ErrorKind::Xrun));
        delivery.audio(vec![0.1; 4], now);
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], AudioEvent::Frames(_)));
    }

    #[test]
    fn any_other_stream_error_ends_the_recording() {
        let (mut delivery, events, _) = recording();
        delivery.stream_error(&cpal::Error::new(cpal::ErrorKind::DeviceNotAvailable));
        assert_eq!(
            *events.lock().unwrap(),
            [AudioEvent::Failed(AudioSourceError::Disconnected)]
        );
    }

    // Rule 10: "stops delivering audio" without an error also ends the Recording.
    #[test]
    fn a_device_that_goes_silent_counts_as_lost() {
        let (mut delivery, events, start) = recording();
        delivery.audio(vec![0.1; 4], start);
        delivery.check_stall(start + STALL_TIMEOUT - Duration::from_millis(10));
        assert_eq!(events.lock().unwrap().len(), 1);
        delivery.check_stall(start + STALL_TIMEOUT);
        assert_eq!(
            events.lock().unwrap().last(),
            Some(&AudioEvent::Failed(AudioSourceError::Disconnected))
        );
    }

    #[test]
    fn a_device_gets_longer_to_deliver_its_first_audio() {
        let (mut delivery, events, start) = recording();
        delivery.check_stall(start + STALL_TIMEOUT * 2);
        assert!(events.lock().unwrap().is_empty());
        delivery.check_stall(start + FIRST_AUDIO_TIMEOUT);
        assert_eq!(
            *events.lock().unwrap(),
            [AudioEvent::Failed(AudioSourceError::Disconnected)]
        );
    }

    #[test]
    fn nothing_is_delivered_after_close() {
        let (mut delivery, events, now) = recording();
        delivery.close();
        delivery.audio(vec![0.1; 4], now);
        delivery.fail(AudioSourceError::Disconnected);
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn open_failures_map_to_the_causes_the_user_sees() {
        use cpal::{Error, ErrorKind};
        assert_eq!(
            open_error(Error::new(ErrorKind::PermissionDenied)),
            AudioSourceError::AccessDenied
        );
        assert_eq!(
            open_error(Error::with_message(
                ErrorKind::BackendError,
                "Access is denied. (os error -2147024891) 0x80070005"
            )),
            AudioSourceError::AccessDenied
        );
        assert_eq!(
            open_error(Error::new(ErrorKind::DeviceNotAvailable)),
            AudioSourceError::NotFound
        );
        assert!(matches!(
            open_error(Error::new(ErrorKind::DeviceBusy)),
            AudioSourceError::Failed(detail) if detail.contains("in use")
        ));
    }

    /// Records about one second from the Windows default device and checks that audio arrived
    /// and the device was released. Needs a real microphone; it only listens.
    /// Run with `cargo test --lib records_from_the_default_microphone -- --ignored`.
    #[test]
    #[ignore = "needs a real microphone"]
    fn records_from_the_default_microphone() {
        let devices = Arc::new(CpalInputDevices);
        let present = devices.list().unwrap();
        println!("input devices: {present:?}");
        assert_eq!(devices.access(), MicrophoneAccess::Allowed);

        let events = Arc::new(Mutex::new(Vec::new()));
        let collected = Arc::clone(&events);
        let mut microphone = super::super::Microphones::new(devices).source(
            || super::super::MicrophoneChoice::Default,
            |notice| panic!("unexpected notice {notice:?}"),
        );
        let stream = microphone
            .start(Box::new(move |e| collected.lock().unwrap().push(e)))
            .unwrap();
        thread::sleep(Duration::from_millis(1200));
        stream.stop();

        let events = events.lock().unwrap();
        assert!(
            !events.iter().any(|e| matches!(e, AudioEvent::Failed(_))),
            "{:?}",
            events.iter().find(|e| matches!(e, AudioEvent::Failed(_)))
        );
        let frames: Vec<&AudioFrames> = events
            .iter()
            .filter_map(|e| match e {
                AudioEvent::Frames(f) => Some(f),
                _ => None,
            })
            .collect();
        let format = frames.first().expect("no audio arrived").format;
        let mut converter = SpeechConverter::new(format).unwrap();
        let mut speech = Vec::new();
        for f in &frames {
            assert_eq!(f.format, format);
            speech.extend(converter.push(&f.samples));
        }
        speech.extend(converter.finish());
        let seconds = speech.len() as f32 / crate::audio::TARGET_SAMPLE_RATE as f32;
        println!("{format:?}: {seconds:.2} s of 16 kHz mono audio");
        assert!(seconds > 0.5, "only {seconds} s of audio");
    }
}
