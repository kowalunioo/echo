//! The Dictation Language (`docs/specs/dictation-language.md`): the stored intent, what each
//! Model supports, and the rule that resolves the intent against the active Model into the
//! effective language the Engine receives (rule 4).

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::engine::DictationLanguage;
use crate::models::ModelId;

/// The stored value that means "Automatic".
pub const AUTOMATIC: &str = "automatic";

/// The Dictation Language setting: the user's intent, `"automatic"` or one language code such
/// as `"pl"` (rules 1–3). It is stored as chosen and never rewritten because of the active Model,
/// so any language some Model offers is valid, whichever Model is active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(try_from = "String", into = "String")]
#[specta(transparent)]
pub struct DictationLanguageSetting(String);

impl DictationLanguageSetting {
    /// "Automatic", the default (rule 2).
    pub fn automatic() -> Self {
        Self(AUTOMATIC.to_owned())
    }

    /// The language code of a specific intent, `None` for Automatic.
    pub fn code(&self) -> Option<&str> {
        (self.0 != AUTOMATIC).then_some(self.0.as_str())
    }
}

impl Default for DictationLanguageSetting {
    fn default() -> Self {
        Self::automatic()
    }
}

impl TryFrom<String> for DictationLanguageSetting {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let known = value == AUTOMATIC
            || ModelId::ALL
                .iter()
                .any(|id| id.languages().languages.contains(&value.as_str()));
        if known {
            Ok(Self(value))
        } else {
            Err(format!(
                "Dictation Language must be \"automatic\" or a language a Model supports, got {value:?}"
            ))
        }
    }
}

impl From<DictationLanguageSetting> for String {
    fn from(setting: DictationLanguageSetting) -> Self {
        setting.0
    }
}

/// What one Model supports (rule 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageSupport {
    /// Language codes in the Model's own order.
    pub languages: &'static [&'static str],
    /// The Model can detect the spoken language itself.
    pub automatic: bool,
    /// A specific language reaches the Model. `false` for Parakeet, which always detects.
    pub honours_language: bool,
}

/// Whisper's languages in the order of its tokenizer; large-v3 added Cantonese (`yue`).
const WHISPER_V3_LANGUAGES: [&str; 100] = [
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it",
    "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur",
    "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn",
    "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si",
    "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo",
    "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
    "ha", "ba", "jw", "su", "yue",
];

/// The 25 European languages of Parakeet TDT 0.6B v3.
const PARAKEET_V3_LANGUAGES: [&str; 25] = [
    "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv", "mt",
    "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
];

impl ModelId {
    /// The languages this Model supports (rule 6).
    pub fn languages(self) -> LanguageSupport {
        match self {
            ModelId::WhisperLargeV3Turbo => LanguageSupport {
                languages: &WHISPER_V3_LANGUAGES,
                automatic: true,
                honours_language: true,
            },
            // Whisper small predates Cantonese, the last of the large-v3 list.
            ModelId::WhisperSmall => LanguageSupport {
                languages: &WHISPER_V3_LANGUAGES[..99],
                automatic: true,
                honours_language: true,
            },
            ModelId::ParakeetTdt06bV3 => LanguageSupport {
                languages: &PARAKEET_V3_LANGUAGES,
                automatic: true,
                honours_language: false,
            },
        }
    }
}

/// The language part of a code without its regional variant, with Norwegian "nb" read as "no"
/// (rule 5).
fn base(code: &str) -> String {
    let primary = code
        .split(['-', '_'])
        .next()
        .unwrap_or(code)
        .to_ascii_lowercase();
    if primary == "nb" {
        "no".to_owned()
    } else {
        primary
    }
}

/// The Model's own code for `intent`, ignoring regional variants (rule 5).
fn find(intent: &str, model: &LanguageSupport) -> Option<&'static str> {
    if let Some(exact) = model.languages.iter().find(|l| **l == intent) {
        return Some(exact);
    }
    let wanted = base(intent);
    model.languages.iter().copied().find(|l| base(l) == wanted)
}

/// Resolves the intent against a Model into the effective language (rule 4).
pub fn resolve(intent: &DictationLanguageSetting, model: &LanguageSupport) -> DictationLanguage {
    if let Some(code) = intent.code().and_then(|code| find(code, model)) {
        return DictationLanguage::Specific(code.to_owned());
    }
    if model.automatic {
        return DictationLanguage::Automatic;
    }
    let fallback = find("en", model).or_else(|| model.languages.first().copied());
    match fallback {
        Some(code) => DictationLanguage::Specific(code.to_owned()),
        // A Model with no languages cannot exist; let it detect rather than fail.
        None => DictationLanguage::Automatic,
    }
}

/// The effective language for a Dictation with the active Model (rule 4). Without an active
/// Model no Dictation starts; Automatic is returned then.
pub fn effective_language(
    intent: &DictationLanguageSetting,
    active: Option<ModelId>,
) -> DictationLanguage {
    active.map_or(DictationLanguage::Automatic, |model| {
        resolve(intent, &model.languages())
    })
}

/// One Model's languages for the Dictation Language picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelLanguages {
    pub model: ModelId,
    /// Language codes; the UI names and sorts them in the UI Language.
    pub languages: Vec<String>,
    /// "Automatic" is offered (rule 4.2).
    pub automatic: bool,
    /// `false`: the picker is replaced by "This Model detects the language automatically."
    pub honours_language: bool,
}

/// The languages of every Model, in list order.
#[tauri::command]
#[specta::specta]
pub fn get_model_languages() -> Vec<ModelLanguages> {
    ModelId::ALL
        .into_iter()
        .map(|model| {
            let support = model.languages();
            ModelLanguages {
                model,
                languages: support.languages.iter().map(|l| (*l).to_owned()).collect(),
                automatic: support.automatic,
                honours_language: support.honours_language,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(value: &str) -> DictationLanguageSetting {
        DictationLanguageSetting::try_from(value.to_owned()).unwrap()
    }

    fn specific(code: &str) -> DictationLanguage {
        DictationLanguage::Specific(code.to_owned())
    }

    const ENGLISH_ONLY: LanguageSupport = LanguageSupport {
        languages: &["de", "en", "fr"],
        automatic: false,
        honours_language: true,
    };
    const NO_ENGLISH: LanguageSupport = LanguageSupport {
        languages: &["fr", "de"],
        automatic: false,
        honours_language: true,
    };

    #[test]
    fn default_is_automatic() {
        assert_eq!(DictationLanguageSetting::default().code(), None);
        assert_eq!(
            String::from(DictationLanguageSetting::default()),
            "automatic"
        );
    }

    // Acceptance test 1.
    #[test]
    fn polish_resolves_to_polish_on_every_model() {
        for id in ModelId::ALL {
            assert_eq!(
                resolve(&intent("pl"), &id.languages()),
                specific("pl"),
                "{id:?}"
            );
        }
    }

    #[test]
    fn a_language_parakeet_lacks_falls_back_to_detection() {
        let parakeet = ModelId::ParakeetTdt06bV3.languages();
        assert_eq!(
            resolve(&intent("ja"), &parakeet),
            DictationLanguage::Automatic
        );
    }

    #[test]
    fn without_detection_an_unsupported_language_falls_back_to_english() {
        assert_eq!(resolve(&intent("pl"), &ENGLISH_ONLY), specific("en"));
        assert_eq!(resolve(&intent("automatic"), &ENGLISH_ONLY), specific("en"));
    }

    #[test]
    fn without_detection_or_english_the_first_language_is_used() {
        assert_eq!(resolve(&intent("automatic"), &NO_ENGLISH), specific("fr"));
    }

    #[test]
    fn automatic_stays_automatic_where_the_model_detects() {
        for id in ModelId::ALL {
            assert_eq!(
                resolve(&intent("automatic"), &id.languages()),
                DictationLanguage::Automatic
            );
        }
    }

    // Acceptance test 2 and rule 5.
    #[test]
    fn regional_variants_and_norwegian_match() {
        let model = LanguageSupport {
            languages: &["en-US", "nb-NO"],
            automatic: false,
            honours_language: true,
        };
        assert_eq!(resolve(&intent("en"), &model), specific("en-US"));
        assert_eq!(resolve(&intent("no"), &model), specific("nb-NO"));
        assert_eq!(
            resolve(&intent("no"), &ModelId::WhisperSmall.languages()),
            specific("no")
        );
    }

    #[test]
    fn language_counts_match_the_catalog() {
        for id in ModelId::ALL {
            let support = id.languages();
            assert_eq!(
                support.languages.len() as u32,
                id.info().languages,
                "{id:?}"
            );
            let mut unique = support.languages.to_vec();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(
                unique.len(),
                support.languages.len(),
                "{id:?} has duplicates"
            );
            assert!(support.languages.contains(&"pl") && support.languages.contains(&"en"));
        }
        assert!(!ModelId::ParakeetTdt06bV3.languages().honours_language);
    }

    // Acceptance test 3: switching Models never rewrites the intent.
    #[test]
    fn the_intent_survives_a_switch_to_a_model_without_it() {
        use crate::settings::{Settings, SettingsPatch};
        let mut settings = Settings::defaults(None);
        assert_eq!(
            settings.dictation_language,
            DictationLanguageSetting::automatic()
        );
        SettingsPatch {
            dictation_language: Some(intent("pl")),
            ..SettingsPatch::default()
        }
        .apply_to(&mut settings);
        let models = [ModelId::WhisperLargeV3Turbo, ModelId::ParakeetTdt06bV3];
        // A Model without Polish stands in for the second one.
        let support = |id: ModelId| match id {
            ModelId::ParakeetTdt06bV3 => ENGLISH_ONLY,
            other => other.languages(),
        };
        let mut effective = Vec::new();
        for active in [models[0], models[1], models[0]] {
            settings.active_model = Some(active);
            effective.push(resolve(&settings.dictation_language, &support(active)));
        }
        assert_eq!(settings.dictation_language.code(), Some("pl"));
        assert_eq!(effective, [specific("pl"), specific("en"), specific("pl")]);
    }

    // Acceptance test 4 (resolution half; `model_engine` tests that Whisper gets the code).
    #[test]
    fn the_active_whisper_gets_the_chosen_language_or_detection() {
        let whisper = Some(ModelId::WhisperLargeV3Turbo);
        assert_eq!(effective_language(&intent("pl"), whisper), specific("pl"));
        assert_eq!(
            effective_language(&intent("automatic"), whisper),
            DictationLanguage::Automatic
        );
        assert_eq!(
            effective_language(&intent("pl"), None),
            DictationLanguage::Automatic
        );
    }

    #[test]
    fn the_setting_accepts_automatic_and_known_languages_only() {
        let parse = |v: &str| serde_json::from_value::<DictationLanguageSetting>(v.into());
        assert_eq!(
            parse("automatic").unwrap(),
            DictationLanguageSetting::automatic()
        );
        assert_eq!(parse("pl").unwrap().code(), Some("pl"));
        assert_eq!(parse("yue").unwrap().code(), Some("yue"));
        assert!(parse("xx").is_err());
        assert!(parse("").is_err());
        assert!(serde_json::from_value::<DictationLanguageSetting>(5.into()).is_err());
    }

    #[test]
    fn the_command_lists_every_model() {
        let all = get_model_languages();
        assert_eq!(all.len(), 3);
        assert_eq!(all[1].model, ModelId::ParakeetTdt06bV3);
        assert!(!all[1].honours_language);
        assert_eq!(all[0].languages.len(), 100);
    }
}
