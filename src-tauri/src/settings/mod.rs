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

use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::audio::microphone::MicrophoneChoice;
use crate::models::ModelId;
use crate::overlay::OverlayPosition;
use crate::shortcut::modes::ShortcutMode;
use crate::shortcut::validation::{CancelShortcutCombination, RecordShortcutCombination};

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
    /// How many Transcripts History keeps (`history.md` rules 6–9).
    history_limit: HistoryLimit,
    /// The input device Recordings listen to (`microphone.md`).
    microphone: MicrophoneChoice,
    /// The Model Dictations use, or none (`models.md` rules 17–23). Owned by the Model manager:
    /// change it with the `activate_model` command, which loads the Model first and keeps the
    /// previous one if loading fails — never through `update_settings`.
    active_model: Option<ModelId>,
    /// Unload the active Model from memory after this much time without a Dictation
    /// (`models.md` rule 24a).
    unload_model_after: UnloadModelAfter,
    /// The key combination that starts and stops a Recording (`record-shortcut.md`).
    record_shortcut: RecordShortcutCombination,
    /// Push-to-Talk Mode or Toggle Mode (`record-shortcut.md`).
    shortcut_mode: ShortcutMode,
    /// The key combination that cancels the current Dictation (`cancel-shortcut.md`).
    cancel_shortcut: CancelShortcutCombination,
    /// Start Echo, hidden, when the user signs in to Windows (`autostart.md`).
    start_with_windows: bool,
    /// Show the Overlay while recording and transcribing; errors show regardless (`overlay.md`
    /// rule 17).
    show_overlay: bool,
    /// Where the Overlay sits on the monitor (`overlay.md` rule 14).
    overlay_position: OverlayPosition,
}

impl Settings {
    /// The defaults for a fresh install. `system_locale` is the Windows display language as a
    /// BCP 47 tag; it decides the default UI Language (rule 7).
    pub fn defaults(system_locale: Option<&str>) -> Self {
        Self {
            ui_language: UiLanguage::for_locale(system_locale),
            onboarding_welcome_done: false,
            onboarding_completed: false,
            history_limit: HistoryLimit::default(),
            microphone: MicrophoneChoice::Default,
            active_model: None,
            unload_model_after: UnloadModelAfter::Never,
            record_shortcut: RecordShortcutCombination::default(),
            shortcut_mode: ShortcutMode::default(),
            cancel_shortcut: CancelShortcutCombination::default(),
            start_with_windows: false,
            show_overlay: true,
            overlay_position: OverlayPosition::Bottom,
        }
    }
}

/// The "Unload Model after inactivity" choices (`models.md` "Settings").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UnloadModelAfter {
    Never,
    Minutes2,
    Minutes5,
    Minutes10,
    Minutes15,
    Minutes60,
}

impl UnloadModelAfter {
    /// How long the Model may sit unused, or `None` for Never.
    pub fn duration(self) -> Option<Duration> {
        let minutes = match self {
            Self::Never => return None,
            Self::Minutes2 => 2,
            Self::Minutes5 => 5,
            Self::Minutes10 => 10,
            Self::Minutes15 => 15,
            Self::Minutes60 => 60,
        };
        Some(Duration::from_secs(minutes * 60))
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

/// How many entries History keeps: an integer 0–100, default 5; 0 keeps nothing (`history.md`
/// rules 6–9). Deserialising rejects anything outside the range, so invalid stored values are
/// salvaged to the default and invalid patches are refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(try_from = "u32", into = "u32")]
#[specta(transparent)]
pub struct HistoryLimit(u8);

impl HistoryLimit {
    pub const MAX: u8 = 100;
    pub const DEFAULT: u8 = 5;

    /// The number of entries to keep.
    pub fn get(self) -> u32 {
        u32::from(self.0)
    }
}

impl Default for HistoryLimit {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

impl TryFrom<u32> for HistoryLimit {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        u8::try_from(value)
            .ok()
            .filter(|v| *v <= Self::MAX)
            .map(Self)
            .ok_or_else(|| format!("History limit must be 0–{}, got {value}", Self::MAX))
    }
}

impl From<HistoryLimit> for u32 {
    fn from(limit: HistoryLimit) -> Self {
        limit.get()
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

    // history.md acceptance test 7.
    #[test]
    fn the_history_limit_accepts_0_to_100_only() {
        let parse = |v: serde_json::Value| serde_json::from_value::<HistoryLimit>(v);
        assert_eq!(parse(serde_json::json!(0)).unwrap().get(), 0);
        assert_eq!(parse(serde_json::json!(100)).unwrap().get(), 100);
        assert!(parse(serde_json::json!(-1)).is_err());
        assert!(parse(serde_json::json!(101)).is_err());
        assert!(parse(serde_json::json!(2.5)).is_err());
        assert_eq!(Settings::defaults(None).history_limit.get(), 5);
        let patch =
            serde_json::from_value::<SettingsPatch>(serde_json::json!({"historyLimit": 101}));
        assert!(patch.is_err());
    }

    // record-shortcut.md rules 5 and 18.
    #[test]
    fn the_record_shortcut_defaults_to_ctrl_space_in_push_to_talk_mode() {
        let settings = Settings::defaults(None);
        assert_eq!(
            settings.record_shortcut.combination().to_string(),
            "Ctrl+Space"
        );
        assert_eq!(settings.shortcut_mode, ShortcutMode::PushToTalk);
        // cancel-shortcut.md rule 10.
        assert_eq!(settings.cancel_shortcut.combination().to_string(), "Escape");
    }

    // overlay.md "Settings".
    #[test]
    fn the_overlay_is_shown_at_the_bottom_by_default() {
        let settings = Settings::defaults(None);
        assert!(settings.show_overlay);
        assert_eq!(settings.overlay_position, OverlayPosition::Bottom);
        let parse = |v| serde_json::from_value::<SettingsPatch>(v);
        assert!(parse(serde_json::json!({"overlayPosition": "top"})).is_ok());
        assert!(parse(serde_json::json!({"overlayPosition": "left"})).is_err());
    }

    #[test]
    fn fresh_settings_start_onboarding_from_the_beginning() {
        let settings = Settings::defaults(None);
        assert!(!settings.onboarding_welcome_done);
        assert!(!settings.onboarding_completed);
    }

    // models.md acceptance test 21, and "none until first download".
    #[test]
    fn fresh_settings_have_no_active_model_and_never_unload() {
        let settings = Settings::defaults(None);
        assert_eq!(settings.active_model, None);
        assert_eq!(settings.unload_model_after, UnloadModelAfter::Never);
        assert_eq!(UnloadModelAfter::Never.duration(), None);
        assert_eq!(
            UnloadModelAfter::Minutes5.duration(),
            Some(Duration::from_secs(300))
        );
        assert_eq!(
            UnloadModelAfter::Minutes60.duration(),
            Some(Duration::from_secs(3600))
        );
    }
}
