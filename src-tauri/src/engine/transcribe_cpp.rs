//! The real Engine on top of the transcribe-cpp library (ADR 0003). This is the only file that
//! names transcribe-cpp; everything it returns is Echo's own types.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, Once};

use transcribe_cpp as tc;

use super::model_engine::{
    DeviceRequest, LoadError, LoadedModel, Loader, ModelEngine, ModelFamily, RunError, RunRequest,
};
use super::{Acceleration, ComputeDevice, Engine, EngineError, TranscriptionRequest};

/// An [`Engine`] that runs one GGUF Model file (Whisper or Parakeet) with transcribe-cpp, on a
/// GPU through Vulkan when possible and on the CPU otherwise (`models.md` rule 29).
pub struct TranscribeCppEngine {
    inner: ModelEngine<TranscribeCppLoader>,
}

impl TranscribeCppEngine {
    /// An Engine for the Model file at `model_path` with this machine's acceleration policy
    /// ([`Acceleration::for_this_machine`]). Nothing is loaded until [`Engine::load`] or the
    /// first [`Engine::transcribe`].
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self::with_acceleration(model_path, Acceleration::for_this_machine())
    }

    /// Like [`new`](Self::new) with an explicit acceleration policy.
    pub fn with_acceleration(model_path: impl Into<PathBuf>, acceleration: Acceleration) -> Self {
        Self {
            inner: ModelEngine::new(
                TranscribeCppLoader {
                    path: model_path.into(),
                    acceleration,
                },
                acceleration,
            ),
        }
    }

    /// The device the loaded Model computes on, or `None` while it is not loaded.
    pub fn device(&self) -> Option<ComputeDevice> {
        self.inner.device()
    }
}

impl Engine for TranscribeCppEngine {
    fn load(&mut self) -> Result<(), EngineError> {
        self.inner.load()
    }

    fn transcribe(&mut self, request: TranscriptionRequest<'_>) -> Result<String, EngineError> {
        self.inner.transcribe(request)
    }

    fn unload(&mut self) {
        self.inner.unload();
    }

    fn accepts_prompt(&self) -> bool {
        self.inner.accepts_prompt()
    }
}

struct TranscribeCppLoader {
    path: PathBuf,
    acceleration: Acceleration,
}

impl Loader for TranscribeCppLoader {
    fn load(&self, device: DeviceRequest) -> Result<Box<dyn LoadedModel>, LoadError> {
        let _native = native_lock();
        init_backends(self.acceleration)?;
        let options = tc::ModelOptions {
            backend: match device {
                DeviceRequest::Auto => tc::Backend::Auto,
                DeviceRequest::Cpu => tc::Backend::Cpu,
            },
            device: None,
        };
        let model =
            tc::Model::load_with(&self.path, &options).map_err(|e| load_error(&self.path, e))?;
        let session = model.session().map_err(|e| load_error(&self.path, e))?;
        let family = family_of_arch(&model.arch());
        let device = compute_device(&model);
        Ok(Box::new(TranscribeCppModel {
            native: Some((session, model)),
            family,
            device,
        }))
    }
}

/// Serialises every native load, run and free across all Engines in the process. The library
/// only serialises runs per Model, and two Models touching the Vulkan device at once — e.g. the
/// old Model being freed while a newly activated one loads or runs — crashes the process
/// (observed: `vkCreateFence: Invalid device`, then STATUS_STACK_BUFFER_OVERRUN). Echo
/// transcribes one Dictation at a time, so this costs nothing in practice.
fn native_lock() -> MutexGuard<'static, ()> {
    static NATIVE: Mutex<()> = Mutex::new(());
    NATIVE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Registers the native compute backends once per process, before the first load. With
/// [`Acceleration::CpuOnly`] only the CPU backend is registered, so the Vulkan backend never runs
/// any code — under ARM64 emulation, or without a Vulkan loader, it must not even be probed.
/// The library fixes this choice at first registration, so the first Engine to load in a process
/// decides it; the app only ever uses [`Acceleration::for_this_machine`].
fn init_backends(acceleration: Acceleration) -> Result<(), LoadError> {
    static INIT: Once = Once::new();
    let mut result = Ok(());
    INIT.call_once(|| {
        let allowed = match acceleration {
            Acceleration::Auto => tc::BackendMask::ALL,
            Acceleration::CpuOnly => tc::BackendMask::CPU,
        };
        result = tc::init_backends_with(None::<&Path>, allowed)
            .map_err(|e| LoadError::Device(format!("no compute backend: {e}")));
    });
    result
}

struct TranscribeCppModel {
    /// The session and its Model; `None` only while being freed. The session comes first so it
    /// is released before the Model.
    native: Option<(tc::Session, tc::Model)>,
    family: ModelFamily,
    device: ComputeDevice,
}

impl TranscribeCppModel {
    fn native(&mut self) -> &mut (tc::Session, tc::Model) {
        self.native.as_mut().expect("present until dropped")
    }
}

impl Drop for TranscribeCppModel {
    fn drop(&mut self) {
        let _native = native_lock();
        drop(self.native.take());
    }
}

impl LoadedModel for TranscribeCppModel {
    fn family(&self) -> ModelFamily {
        self.family
    }

    fn device(&self) -> ComputeDevice {
        self.device.clone()
    }

    fn count_tokens(&self, text: &str) -> Option<usize> {
        let (_, model) = self.native.as_ref()?;
        let _native = native_lock();
        model.tokenize(text).ok().map(|tokens| tokens.len())
    }

    fn run(&mut self, audio: &[f32], request: &RunRequest) -> Result<String, RunError> {
        let options = tc::RunOptions {
            language: request.language.clone(),
            prompt: request.prompt.clone(),
            ..Default::default()
        };
        let _native = native_lock();
        let (session, _) = self.native();
        run_outcome(session.run(audio, &options))
    }
}

fn family_of_arch(arch: &str) -> ModelFamily {
    if arch.eq_ignore_ascii_case("whisper") {
        ModelFamily::Whisper
    } else {
        ModelFamily::AutoDetectOnly
    }
}

fn compute_device(model: &tc::Model) -> ComputeDevice {
    match model.device() {
        Ok(device)
            if matches!(
                device.device_type,
                tc::DeviceType::Gpu | tc::DeviceType::Igpu
            ) =>
        {
            ComputeDevice::Gpu(device.description)
        }
        Ok(_) => ComputeDevice::Cpu,
        // The library could not say; its backend name still tells CPU from GPU.
        Err(_) => match model.backend().as_str() {
            "cpu" => ComputeDevice::Cpu,
            other => ComputeDevice::Gpu(other.to_string()),
        },
    }
}

/// Sorts a load failure into "the file is the problem" and "the device is the problem"; only
/// the latter is worth retrying on the CPU.
fn load_error(path: &Path, error: tc::Error) -> LoadError {
    match error {
        tc::Error::ModelFileNotFound(_) => {
            LoadError::Model(format!("Model file not found: {}", path.display()))
        }
        tc::Error::ModelLoad(_)
        | tc::Error::VersionMismatch(_)
        | tc::Error::BadStructSize(_)
        | tc::Error::InvalidArgument(_)
        | tc::Error::NotImplemented(_)
        | tc::Error::Unsupported(_)
        | tc::Error::Nul(_) => LoadError::Model(error.to_string()),
        other => LoadError::Device(other.to_string()),
    }
}

/// Turns the library's result into the raw Engine text or a [`RunError`] that says whether the
/// Model must be reloaded (`dictation-pipeline.md` rule 23).
fn run_outcome(result: Result<tc::Transcript, tc::Error>) -> Result<String, RunError> {
    let failure = |crashed: bool, error: &tc::Error| RunError {
        crashed,
        detail: error.to_string(),
    };
    match result {
        Ok(transcript) => Ok(transcript.text),
        // The decode stopped early (a repetition loop, or the generation budget) but kept what
        // it had; that text is still the best Transcript there is.
        Err(
            tc::Error::OutputRepetition {
                partial: Some(partial),
                ..
            }
            | tc::Error::OutputTruncated {
                partial: Some(partial),
                ..
            },
        ) => Ok(partial.text),
        // Problems with this one request: the Model itself is fine.
        Err(
            error @ (tc::Error::InvalidArgument(_)
            | tc::Error::Unsupported(_)
            | tc::Error::InputTooLong(_)
            | tc::Error::NotImplemented(_)
            | tc::Error::Nul(_)
            | tc::Error::Busy(_)
            | tc::Error::Aborted { .. }
            | tc::Error::OutputRepetition { .. }
            | tc::Error::OutputTruncated { .. }),
        ) => Err(failure(false, &error)),
        // Out of memory, a lost GPU, or anything unexpected: the native state is suspect.
        Err(error) => Err(failure(true, &error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_is_recognised_by_its_architecture() {
        assert_eq!(family_of_arch("whisper"), ModelFamily::Whisper);
        assert_eq!(family_of_arch("parakeet"), ModelFamily::AutoDetectOnly);
    }

    #[test]
    fn a_missing_or_broken_file_is_not_retried_on_another_device() {
        let path = Path::new("C:/models/x.gguf");
        assert_eq!(
            load_error(path, tc::Error::ModelFileNotFound("x".into())),
            LoadError::Model("Model file not found: C:/models/x.gguf".into())
        );
        assert!(matches!(
            load_error(path, tc::Error::ModelLoad("bad magic".into())),
            LoadError::Model(_)
        ));
    }

    #[test]
    fn device_failures_are_retried_on_the_cpu() {
        let path = Path::new("x.gguf");
        assert!(matches!(
            load_error(path, tc::Error::Backend("vk".into())),
            LoadError::Device(_)
        ));
        assert!(matches!(
            load_error(path, tc::Error::OutOfMemory("vram".into())),
            LoadError::Device(_)
        ));
    }

    #[test]
    fn request_problems_keep_the_model_and_device_faults_unload_it() {
        let input = run_outcome(Err(tc::Error::InputTooLong("31 min".into()))).unwrap_err();
        assert!(!input.crashed);
        assert_eq!(input.detail, "input too long: 31 min");

        for fault in [
            tc::Error::OutOfMemory("vram".into()),
            tc::Error::Backend("device lost".into()),
            tc::Error::Other("status -99".into()),
        ] {
            assert!(run_outcome(Err(fault)).unwrap_err().crashed);
        }
    }

    #[test]
    fn a_stopped_decode_returns_what_it_kept() {
        let partial = tc::Transcript {
            text: "dzień dobry".into(),
            ..Default::default()
        };
        let result = run_outcome(Err(tc::Error::OutputRepetition {
            message: "loop".into(),
            partial: Some(Box::new(partial)),
        }));
        assert_eq!(result, Ok("dzień dobry".into()));

        let empty = run_outcome(Err(tc::Error::OutputTruncated {
            message: "budget".into(),
            partial: None,
        }));
        assert!(!empty.unwrap_err().crashed);
    }

    #[test]
    fn a_missing_model_file_fails_to_load_without_crashing() {
        let mut engine = TranscribeCppEngine::new("this-model-does-not-exist.gguf");
        let result = engine.load();
        assert!(
            matches!(&result, Err(EngineError::ModelLoad(d)) if d.contains("not found")),
            "{result:?}"
        );
        assert_eq!(engine.device(), None);
    }
}
