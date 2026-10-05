//! Tauri commands exposed to the frontend. Each one is registered in [`crate::specta_builder`],
//! which also generates their typed TypeScript bindings in `src/bindings.ts`.

use serde::Serialize;
use specta::Type;

/// The language of Echo's own interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum UiLanguage {
    Pl,
    En,
}

impl UiLanguage {
    /// The UI Language for a Windows display language given as a BCP 47 tag such as `"pl-PL"`:
    /// Polish for any Polish variant, otherwise English (`settings-and-first-run.md` rule 7).
    pub fn for_locale(locale: Option<&str>) -> Self {
        let primary = locale
            .and_then(|tag| tag.split(['-', '_']).next())
            .unwrap_or_default();
        if primary.eq_ignore_ascii_case("pl") {
            Self::Pl
        } else {
            Self::En
        }
    }
}

/// Facts about the running app that the interface needs at start-up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// Echo's version, e.g. `0.1.0`.
    pub version: String,
    /// The Windows display language as a BCP 47 tag, if Windows reports one.
    pub system_locale: Option<String>,
    /// The UI Language to use until the user picks one.
    pub default_ui_language: UiLanguage,
}

/// Returns facts about the running app.
#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    let system_locale = sys_locale::get_locale();
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        default_ui_language: UiLanguage::for_locale(system_locale.as_deref()),
        system_locale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polish_windows_gives_a_polish_ui() {
        assert_eq!(UiLanguage::for_locale(Some("pl-PL")), UiLanguage::Pl);
        assert_eq!(UiLanguage::for_locale(Some("pl")), UiLanguage::Pl);
        assert_eq!(UiLanguage::for_locale(Some("PL_pl")), UiLanguage::Pl);
    }

    #[test]
    fn any_other_windows_language_gives_an_english_ui() {
        assert_eq!(UiLanguage::for_locale(Some("de-DE")), UiLanguage::En);
        assert_eq!(UiLanguage::for_locale(Some("en-US")), UiLanguage::En);
        assert_eq!(UiLanguage::for_locale(Some("plx")), UiLanguage::En);
        assert_eq!(UiLanguage::for_locale(None), UiLanguage::En);
    }

    #[test]
    fn app_info_reports_the_package_version() {
        let info = app_info();
        assert_eq!(info.version, "0.1.0");
        assert_eq!(
            info.default_ui_language,
            UiLanguage::for_locale(info.system_locale.as_deref())
        );
    }
}
