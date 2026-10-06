//! Echo's settings: the typed model, its defaults, and the [`SettingsStore`] that loads, salvages,
//! saves and broadcasts them (`settings-and-first-run.md` rules 11–15).
//!
//! **Adding a setting** (see `docs/settings.md`): add one line to the `settings_model!` list below
//! with a type whose `Deserialize` accepts exactly the valid values, give it a value in
//! [`Settings::defaults`], and run `bun run bindings`. Loading, salvage, saving, change events and
//! the frontend store pick it up without further code.

pub mod commands;
mod store;

pub use commands::SettingsChanged;
pub use store::{LoadOutcome, SETTINGS_FORMAT_VERSION, SettingsError, SettingsStore};

use serde::{Deserialize, Serialize};
use specta::Type;

/// Generates [`Settings`] and its all-optional twin [`SettingsPatch`] from one list of fields, so
/// the two can never drift apart.
macro_rules! settings_model {
    ($( $(#[doc = $doc:literal])* $field:ident: $ty:ty, )*) => {
        /// Every persisted setting, one top-level field per setting. Each field is salvaged on
        /// its own: an invalid value resets only that field to its default (rule 13).
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
        #[serde(rename_all = "camelCase")]
        pub struct Settings {
            $( $(#[doc = $doc])* pub $field: $ty, )*
        }

        /// A change to some settings: only the fields that are present change.
        #[derive(Debug, Clone, Default, PartialEq, Deserialize, Type)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct SettingsPatch {
            $(
                $(#[doc = $doc])*
                #[serde(default)]
                #[specta(optional)]
                pub $field: Option<$ty>,
            )*
        }

        impl SettingsPatch {
            /// Copies every present field onto `settings`.
            pub fn apply_to(self, settings: &mut Settings) {
                $( if let Some(value) = self.$field { settings.$field = value; } )*
            }
        }
    };
}

settings_model! {
    /// The language of Echo's own interface (owned by `settings-and-first-run.md`).
    ui_language: UiLanguage,
    /// The user has moved past the Welcome step of the onboarding (`settings-and-first-run.md`
    /// rules 1–4). The Microphone access and Choose a Model steps are not stored: they are
    /// complete when Windows allows microphone access and when a Model is active.
    onboarding_welcome_done: bool,
    /// The user pressed "Finish" in the onboarding; it is never shown again (rule 5).
    onboarding_completed: bool,
}

impl Settings {
    /// The defaults for a fresh install. `system_locale` is the Windows display language as a
    /// BCP 47 tag; it decides the default UI Language (rule 7).
    pub fn defaults(system_locale: Option<&str>) -> Self {
        Self {
            ui_language: UiLanguage::for_locale(system_locale),
            onboarding_welcome_done: false,
            onboarding_completed: false,
        }
    }
}

/// The language of Echo's own interface. Independent of the Dictation Language (rule 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
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

#[cfg(test)]
mod tests {
    use super::*;

    // settings-and-first-run.md acceptance test 6.
    #[test]
    fn polish_windows_gives_a_polish_ui() {
        assert_eq!(UiLanguage::for_locale(Some("pl-PL")), UiLanguage::Pl);
        assert_eq!(UiLanguage::for_locale(Some("pl")), UiLanguage::Pl);
        assert_eq!(UiLanguage::for_locale(Some("PL_pl")), UiLanguage::Pl);
        assert_eq!(
            Settings::defaults(Some("pl-PL")).ui_language,
            UiLanguage::Pl
        );
    }

    #[test]
    fn any_other_windows_language_gives_an_english_ui() {
        assert_eq!(UiLanguage::for_locale(Some("de-DE")), UiLanguage::En);
        assert_eq!(UiLanguage::for_locale(Some("en-US")), UiLanguage::En);
        assert_eq!(UiLanguage::for_locale(Some("plx")), UiLanguage::En);
        assert_eq!(UiLanguage::for_locale(None), UiLanguage::En);
        assert_eq!(
            Settings::defaults(Some("de-DE")).ui_language,
            UiLanguage::En
        );
    }

    #[test]
    fn fresh_settings_start_onboarding_from_the_beginning() {
        let settings = Settings::defaults(None);
        assert!(!settings.onboarding_welcome_done);
        assert!(!settings.onboarding_completed);
    }
}
