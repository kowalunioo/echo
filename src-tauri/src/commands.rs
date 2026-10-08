//! Tauri commands exposed to the frontend. Each one is registered in [`crate::specta_builder`],
//! which also generates their typed TypeScript bindings in `src/bindings.ts`.

use serde::Serialize;
use specta::Type;

use crate::engine::Acceleration;

/// Facts about the running app that the interface needs at start-up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// Echo's version, e.g. `0.1.0`.
    pub version: String,
    /// The Windows display language as a BCP 47 tag, if Windows reports one.
    pub system_locale: Option<String>,
}

/// Returns facts about the running app.
#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        system_locale: sys_locale::get_locale(),
    }
}

/// What the Engine will compute on, as far as Echo can tell before a Model is loaded
/// (`models.md` rule 29): the onboarding recommends a Model from it (`settings-and-first-run.md`
/// rule 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ComputeHardware {
    /// A graphics card driver is installed, so the Engine tries the GPU through Vulkan.
    Gpu,
    /// No usable graphics card (no Vulkan driver, or ARM64 emulation): the Engine uses the CPU.
    Cpu,
}

impl From<Acceleration> for ComputeHardware {
    fn from(acceleration: Acceleration) -> Self {
        match acceleration {
            Acceleration::Auto => Self::Gpu,
            Acceleration::CpuOnly => Self::Cpu,
        }
    }
}

/// Whether this computer has a graphics card the Engine can use.
#[tauri::command]
#[specta::specta]
pub fn compute_hardware() -> ComputeHardware {
    Acceleration::for_this_machine().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gpu_policy_reports_a_graphics_card_and_cpu_only_reports_none() {
        assert_eq!(
            ComputeHardware::from(Acceleration::Auto),
            ComputeHardware::Gpu
        );
        assert_eq!(
            ComputeHardware::from(Acceleration::CpuOnly),
            ComputeHardware::Cpu
        );
    }

    #[test]
    fn compute_hardware_answers_on_this_machine() {
        // Whatever the machine has, the probe must answer rather than fail.
        let _ = compute_hardware();
    }

    #[test]
    fn app_info_reports_the_package_version() {
        assert_eq!(app_info().version, "0.1.0");
    }
}
