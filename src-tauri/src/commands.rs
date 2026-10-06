//! Tauri commands exposed to the frontend. Each one is registered in [`crate::specta_builder`],
//! which also generates their typed TypeScript bindings in `src/bindings.ts`.

use serde::Serialize;
use specta::Type;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_the_package_version() {
        assert_eq!(app_info().version, "0.1.0");
    }
}
