//! `Acceleration::CpuOnly` must keep the Vulkan backend from running at all — not just pick the
//! CPU — so a machine without a GPU driver, or an x64 build emulated on ARM64, never touches the
//! delay-loaded Vulkan loader (`docs/specs/models.md` rule 29).
//!
//! The backend choice is process-wide, so these tests live in their own test binary.

use std::path::PathBuf;

use echo_lib::engine::{Acceleration, ComputeDevice, Engine, TranscribeCppEngine};

#[cfg(windows)]
fn vulkan_loader_is_loaded() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut std::ffi::c_void;
    }
    let name: Vec<u16> = "vulkan-1.dll\0".encode_utf16().collect();
    // SAFETY: a NUL-terminated UTF-16 string; GetModuleHandleW does not take a reference.
    !unsafe { GetModuleHandleW(name.as_ptr()) }.is_null()
}

#[cfg(not(windows))]
fn vulkan_loader_is_loaded() -> bool {
    false
}

#[test]
fn cpu_only_never_loads_the_vulkan_loader() {
    let mut engine =
        TranscribeCppEngine::with_acceleration("missing-model.gguf", Acceleration::CpuOnly);
    assert!(engine.load().is_err());

    if let Some(model) = std::env::var_os("ECHO_TEST_MODEL").map(PathBuf::from) {
        // With a real Model (e.g. whisper-small-Q8_0.gguf), it also loads and runs on the CPU.
        let mut engine = TranscribeCppEngine::with_acceleration(model, Acceleration::CpuOnly);
        engine.load().expect("the Model loads on the CPU");
        assert_eq!(engine.device(), Some(ComputeDevice::Cpu));
    }

    assert!(
        !vulkan_loader_is_loaded(),
        "vulkan-1.dll was loaded although only the CPU was allowed"
    );
}
