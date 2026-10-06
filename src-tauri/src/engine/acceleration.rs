//! Where the Engine computes: a GPU through Vulkan when possible, otherwise the CPU
//! (`docs/specs/models.md` rule 29).

/// Which compute devices the Engine may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acceleration {
    /// A GPU through Vulkan when one is usable, otherwise the CPU.
    Auto,
    /// The CPU only.
    CpuOnly,
}

impl Acceleration {
    /// The policy for this machine: [`Auto`](Self::Auto), except when this build runs emulated on
    /// ARM64 Windows (the spec always uses the CPU there) or when no Vulkan loader is installed
    /// (no GPU driver, so the GPU path must never be touched).
    pub fn for_this_machine() -> Self {
        Self::for_machine(running_emulated_on_arm64(), vulkan_loader_installed())
    }

    fn for_machine(emulated_on_arm64: bool, vulkan_loader_installed: bool) -> Self {
        if emulated_on_arm64 || !vulkan_loader_installed {
            Self::CpuOnly
        } else {
            Self::Auto
        }
    }
}

/// The device a loaded Model computes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputeDevice {
    /// A GPU, by the name its driver reports (e.g. "NVIDIA GeForce RTX 3060").
    Gpu(String),
    /// The CPU.
    Cpu,
}

impl std::fmt::Display for ComputeDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gpu(name) => write!(f, "GPU ({name})"),
            Self::Cpu => f.write_str("CPU"),
        }
    }
}

/// `IMAGE_FILE_MACHINE_ARM64` from the Windows SDK.
const MACHINE_ARM64: u16 = 0xAA64;

/// Whether a build for a non-ARM64 architecture runs on a machine whose native architecture is
/// `native_machine` (an `IMAGE_FILE_MACHINE_*` value), i.e. under ARM64 emulation.
fn is_emulated_on_arm64(build_is_arm64: bool, native_machine: Option<u16>) -> bool {
    !build_is_arm64 && native_machine == Some(MACHINE_ARM64)
}

fn running_emulated_on_arm64() -> bool {
    is_emulated_on_arm64(cfg!(target_arch = "aarch64"), native_machine())
}

/// The native architecture of this machine, or `None` if Windows cannot tell.
#[cfg(windows)]
fn native_machine() -> Option<u16> {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn IsWow64Process2(
            process: *mut c_void,
            process_machine: *mut u16,
            native: *mut u16,
        ) -> i32;
    }

    let mut process_machine = 0u16;
    let mut native = 0u16;
    // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing, and both out
    // pointers are valid for the duration of the call. IsWow64Process2 exists on every Windows
    // version Echo supports (Windows 10 1709 and later).
    let ok = unsafe { IsWow64Process2(GetCurrentProcess(), &mut process_machine, &mut native) };
    (ok != 0).then_some(native)
}

#[cfg(not(windows))]
fn native_machine() -> Option<u16> {
    None
}

/// Whether the Vulkan loader (`vulkan-1.dll`, installed with GPU drivers) can be loaded. Echo
/// delay-loads it (see `build.rs`), so without this check a machine with no GPU driver would
/// fail on the first Vulkan call instead of using the CPU.
#[cfg(windows)]
fn vulkan_loader_installed() -> bool {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }
    const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0000_0800;

    let name: Vec<u16> = "vulkan-1.dll\0".encode_utf16().collect();
    // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call; the module, if
    // loaded, is released again right away (the delay-load helper loads it for real later).
    unsafe {
        let module = LoadLibraryExW(
            name.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
        if module.is_null() {
            return false;
        }
        FreeLibrary(module);
    }
    true
}

#[cfg(not(windows))]
fn vulkan_loader_installed() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const MACHINE_AMD64: u16 = 0x8664;

    #[test]
    fn an_x64_build_on_arm64_windows_is_emulated() {
        assert!(is_emulated_on_arm64(false, Some(MACHINE_ARM64)));
    }

    #[test]
    fn native_builds_are_not_emulated() {
        assert!(!is_emulated_on_arm64(false, Some(MACHINE_AMD64)));
        assert!(!is_emulated_on_arm64(true, Some(MACHINE_ARM64)));
        assert!(!is_emulated_on_arm64(false, None));
    }

    #[test]
    fn emulation_forces_the_cpu_and_otherwise_the_gpu_is_tried() {
        assert_eq!(Acceleration::for_machine(true, true), Acceleration::CpuOnly);
        assert_eq!(Acceleration::for_machine(false, true), Acceleration::Auto);
    }

    #[test]
    fn without_a_vulkan_loader_only_the_cpu_is_used() {
        assert_eq!(
            Acceleration::for_machine(false, false),
            Acceleration::CpuOnly
        );
    }

    #[test]
    fn this_test_machine_is_detected() {
        // CI and the developer machines are x64 on x64; the probe must answer, not fail.
        if cfg!(all(windows, target_arch = "x86_64")) {
            assert!(native_machine().is_some());
        }
    }
}
