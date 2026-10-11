//! Which key combinations may be the Record Shortcut (`record-shortcut.md` rules 19–21), the
//! Cancel Shortcut (`cancel-shortcut.md` rule 11) and the Vocabulary Shortcut
//! (`vocabulary-shortcut.md` rules 18–19), and the text form combinations are stored and
//! exchanged in.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::keys;
use super::{Key, KeyCombination, Modifiers, Shortcut};

/// Why a proposed combination cannot be the Record Shortcut (rule 20). The interface shows a
/// message for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutProblem {
    /// No keys at all.
    #[error("the combination is empty")]
    Empty,
    /// A character key, Space or another typing key without a modifier: it would stop working
    /// for typing.
    #[error("this key needs a modifier")]
    NeedsModifier,
    /// A single modifier, right Alt (AltGr) and right Ctrl included (issue #33).
    #[error("a single modifier is not enough")]
    SingleModifier,
    /// Escape alone is reserved as the default Cancel Shortcut.
    #[error("Escape is reserved for cancelling")]
    EscapeReserved,
    /// Windows handles this combination itself and never delivers it to applications.
    #[error("Windows reserves this combination")]
    ReservedByWindows,
    /// The same combination as the Cancel Shortcut.
    #[error("this is the Cancel Shortcut")]
    SameAsCancel,
    /// The same combination as the Record Shortcut (`cancel-shortcut.md` rule 11).
    #[error("this is the Record Shortcut")]
    SameAsRecord,
    /// The same combination as the Vocabulary Shortcut, even while it is turned off
    /// (`vocabulary-shortcut.md` rule 19).
    #[error("this is the Vocabulary Shortcut")]
    SameAsVocabulary,
    /// A text-editing combination every application relies on, such as Ctrl+C
    /// (`record-shortcut.md` rule 20): Echo swallows its shortcuts, so it would take it away.
    #[error("applications need this combination for editing text")]
    EditingCombination,
    /// Not a combination Echo understands (unknown key name, two main keys, …).
    #[error("not a valid combination")]
    Invalid,
}

/// Parses the canonical text form, e.g. `"Ctrl+Alt+D"`, `"Ctrl+Win"`, `"RightAlt"`. Modifiers may
/// come in any order; the empty string is the empty combination.
impl FromStr for KeyCombination {
    type Err = ShortcutProblem;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut combination = KeyCombination::default();
        if text.is_empty() {
            return Ok(combination);
        }
        for part in text.split('+') {
            let modifiers = &mut combination.modifiers;
            let flag = match part {
                "Ctrl" => &mut modifiers.ctrl,
                "Alt" => &mut modifiers.alt,
                "Shift" => &mut modifiers.shift,
                "Win" => &mut modifiers.win,
                key => {
                    if combination.key.is_some() || keys::vk_of(key).is_none() {
                        return Err(ShortcutProblem::Invalid);
                    }
                    combination.key = Some(Key(key.to_owned()));
                    continue;
                }
            };
            if *flag {
                return Err(ShortcutProblem::Invalid);
            }
            *flag = true;
        }
        Ok(combination)
    }
}

impl Modifiers {
    fn count(self) -> usize {
        [self.ctrl, self.alt, self.shift, self.win]
            .into_iter()
            .filter(|held| *held)
            .count()
    }
}

/// The default Record Shortcut, Ctrl+Space (rule 18).
pub fn default_record_shortcut() -> KeyCombination {
    KeyCombination {
        modifiers: Modifiers {
            ctrl: true,
            ..Modifiers::default()
        },
        key: Some(Key("Space".into())),
    }
}

/// The default Cancel Shortcut, Escape (`cancel-shortcut.md` rule 10).
pub fn default_cancel_shortcut() -> KeyCombination {
    KeyCombination {
        modifiers: Modifiers::default(),
        key: Some(Key("Escape".into())),
    }
}

/// The default Vocabulary Shortcut, Ctrl+Alt+D (`vocabulary-shortcut.md` rule 17).
pub fn default_vocabulary_shortcut() -> KeyCombination {
    KeyCombination {
        modifiers: Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::default()
        },
        key: Some(Key("D".into())),
    }
}

/// The combinations of all three shortcuts, which must differ from one another
/// (`vocabulary-shortcut.md` rule 19). A turned-off Vocabulary Shortcut still holds its
/// combination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutCombinations {
    pub record: KeyCombination,
    pub cancel: KeyCombination,
    pub vocabulary: KeyCombination,
}

impl Default for ShortcutCombinations {
    fn default() -> Self {
        Self {
            record: default_record_shortcut(),
            cancel: default_cancel_shortcut(),
            vocabulary: default_vocabulary_shortcut(),
        }
    }
}

impl ShortcutCombinations {
    /// The combination `shortcut` holds.
    pub fn of(&self, shortcut: Shortcut) -> &KeyCombination {
        match shortcut {
            Shortcut::Record => &self.record,
            Shortcut::Cancel => &self.cancel,
            Shortcut::Vocabulary => &self.vocabulary,
        }
    }

    /// The problem to report when another shortcut than `shortcut` holds `combination`.
    pub fn held_by_other(
        &self,
        shortcut: Shortcut,
        combination: &KeyCombination,
    ) -> Option<ShortcutProblem> {
        [
            (Shortcut::Record, ShortcutProblem::SameAsRecord),
            (Shortcut::Cancel, ShortcutProblem::SameAsCancel),
            (Shortcut::Vocabulary, ShortcutProblem::SameAsVocabulary),
        ]
        .into_iter()
        .find(|(other, _)| *other != shortcut && self.of(*other) == combination)
        .map(|(_, problem)| problem)
    }
}

/// Checks `proposal` for `shortcut`: the rules every shortcut shares (`record-shortcut.md` rules
/// 19–20; the Cancel Shortcut may also be Escape alone, `cancel-shortcut.md` rule 11), and that no
/// other shortcut in `current` holds it (`vocabulary-shortcut.md` rule 19).
pub fn validate_shortcut(
    shortcut: Shortcut,
    proposal: &KeyCombination,
    current: &ShortcutCombinations,
) -> Result<(), ShortcutProblem> {
    check_allowed(proposal, shortcut == Shortcut::Cancel)?;
    match current.held_by_other(shortcut, proposal) {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

/// The rules all shortcuts share (`record-shortcut.md` rules 19–20 without the check against
/// the other shortcuts); `escape_alone` says whether Escape without modifiers is allowed.
fn check_allowed(proposal: &KeyCombination, escape_alone: bool) -> Result<(), ShortcutProblem> {
    let modifiers = proposal.modifiers.count();
    match proposal.key.as_ref().map(|k| k.0.as_str()) {
        None if modifiers == 0 => return Err(ShortcutProblem::Empty),
        None if modifiers == 1 => return Err(ShortcutProblem::SingleModifier),
        None => {}
        Some(key) if keys::vk_of(key).is_none() => return Err(ShortcutProblem::Invalid),
        // Right Ctrl or right Alt alone is a single modifier (issue #33: right Alt is AltGr,
        // needed for Polish characters); with others they are just Ctrl/Alt.
        Some(key) if keys::is_side_modifier(key) && modifiers == 0 => {
            return Err(ShortcutProblem::SingleModifier);
        }
        Some(key) if keys::is_side_modifier(key) => return Err(ShortcutProblem::Invalid),
        Some("Escape") if modifiers == 0 && escape_alone => {}
        Some("Escape") if modifiers == 0 => return Err(ShortcutProblem::EscapeReserved),
        Some(key) if modifiers == 0 && !keys::usable_alone(key) => {
            return Err(ShortcutProblem::NeedsModifier);
        }
        Some(key) if reserved_by_windows(proposal.modifiers, key) => {
            return Err(ShortcutProblem::ReservedByWindows);
        }
        Some(key) if editing_combination(proposal.modifiers, key) => {
            return Err(ShortcutProblem::EditingCombination);
        }
        Some(_) => {}
    }
    Ok(())
}

/// The Record Shortcut setting: an allowed combination (rules 19–20) in canonical text form,
/// e.g. `"Ctrl+Space"`. Deserializing accepts exactly the allowed combinations, so an invalid
/// stored value falls back to the default (`docs/settings.md`). Settings are checked one by one,
/// so the check against the Cancel Shortcut is made where the shortcuts are activated
/// (`shortcut::app`), not here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(try_from = "String", into = "String")]
#[specta(transparent)]
pub struct RecordShortcutCombination(String);

impl RecordShortcutCombination {
    /// The combination this setting holds.
    pub fn combination(&self) -> KeyCombination {
        self.0.parse().expect("validated when created")
    }
}

impl Default for RecordShortcutCombination {
    /// Ctrl+Space (rule 18).
    fn default() -> Self {
        Self(default_record_shortcut().to_string())
    }
}

impl TryFrom<KeyCombination> for RecordShortcutCombination {
    type Error = ShortcutProblem;

    fn try_from(combination: KeyCombination) -> Result<Self, Self::Error> {
        check_allowed(&combination, false)?;
        Ok(Self(combination.to_string()))
    }
}

impl TryFrom<String> for RecordShortcutCombination {
    type Error = ShortcutProblem;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse::<KeyCombination>()?.try_into()
    }
}

impl From<RecordShortcutCombination> for String {
    fn from(setting: RecordShortcutCombination) -> Self {
        setting.0
    }
}

/// The Cancel Shortcut setting (`cancel-shortcut.md`): an allowed combination (rule 11) in
/// canonical text form, e.g. `"Escape"`. Like [`RecordShortcutCombination`] it accepts exactly
/// the allowed combinations; the check against the Record Shortcut is made where the shortcuts
/// are activated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(try_from = "String", into = "String")]
#[specta(transparent)]
pub struct CancelShortcutCombination(String);

impl CancelShortcutCombination {
    /// The combination this setting holds.
    pub fn combination(&self) -> KeyCombination {
        self.0.parse().expect("validated when created")
    }
}

impl Default for CancelShortcutCombination {
    /// Escape (rule 10).
    fn default() -> Self {
        Self(default_cancel_shortcut().to_string())
    }
}

impl TryFrom<KeyCombination> for CancelShortcutCombination {
    type Error = ShortcutProblem;

    fn try_from(combination: KeyCombination) -> Result<Self, Self::Error> {
        check_allowed(&combination, true)?;
        Ok(Self(combination.to_string()))
    }
}

impl TryFrom<String> for CancelShortcutCombination {
    type Error = ShortcutProblem;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse::<KeyCombination>()?.try_into()
    }
}

impl From<CancelShortcutCombination> for String {
    fn from(setting: CancelShortcutCombination) -> Self {
        setting.0
    }
}

/// The Vocabulary Shortcut setting (`vocabulary-shortcut.md`): an allowed combination (rule 18)
/// in canonical text form, e.g. `"Ctrl+Alt+D"`. Like [`RecordShortcutCombination`] it accepts
/// exactly the allowed combinations; the check against the other shortcuts is made where the
/// shortcuts are activated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(try_from = "String", into = "String")]
#[specta(transparent)]
pub struct VocabularyShortcutCombination(String);

impl VocabularyShortcutCombination {
    /// The combination this setting holds.
    pub fn combination(&self) -> KeyCombination {
        self.0.parse().expect("validated when created")
    }
}

impl Default for VocabularyShortcutCombination {
    /// Ctrl+Alt+D (rule 17).
    fn default() -> Self {
        Self(default_vocabulary_shortcut().to_string())
    }
}

impl TryFrom<KeyCombination> for VocabularyShortcutCombination {
    type Error = ShortcutProblem;

    fn try_from(combination: KeyCombination) -> Result<Self, Self::Error> {
        check_allowed(&combination, false)?;
        Ok(Self(combination.to_string()))
    }
}

impl TryFrom<String> for VocabularyShortcutCombination {
    type Error = ShortcutProblem;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse::<KeyCombination>()?.try_into()
    }
}

impl From<VocabularyShortcutCombination> for String {
    fn from(setting: VocabularyShortcutCombination) -> Self {
        setting.0
    }
}

/// Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+Z, Ctrl+Y, Ctrl+A, Ctrl+S, Ctrl+Insert, Shift+Insert and
/// Shift+Delete (`record-shortcut.md` rule 20). Only these exact combinations: Ctrl+Shift+C is
/// allowed.
fn editing_combination(modifiers: Modifiers, key: &str) -> bool {
    let only = |ctrl, shift| Modifiers {
        ctrl,
        shift,
        ..Modifiers::default()
    };
    (modifiers == only(true, false) && ["C", "V", "X", "Z", "Y", "A", "S", "Insert"].contains(&key))
        || (modifiers == only(false, true) && ["Insert", "Delete"].contains(&key))
}

/// Ctrl+Alt+Delete (the secure attention sequence) and Win+L (lock) never reach applications.
fn reserved_by_windows(modifiers: Modifiers, key: &str) -> bool {
    (modifiers.ctrl && modifiers.alt && key == "Delete") || (modifiers.win && key == "L")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> KeyCombination {
        text.parse().unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    fn check(text: &str) -> Result<(), ShortcutProblem> {
        validate_shortcut(
            Shortcut::Record,
            &parse(text),
            &ShortcutCombinations::default(),
        )
    }

    fn set(record: &str, cancel: &str, vocabulary: &str) -> ShortcutCombinations {
        ShortcutCombinations {
            record: parse(record),
            cancel: parse(cancel),
            vocabulary: parse(vocabulary),
        }
    }

    #[test]
    fn parses_and_prints_the_canonical_form() {
        assert_eq!(parse("Ctrl+Space"), default_record_shortcut());
        assert_eq!(
            parse("Shift+Win+Alt+Ctrl+D").to_string(),
            "Ctrl+Alt+Shift+Win+D"
        );
        assert_eq!(parse("Win+Ctrl").to_string(), "Ctrl+Win");
        assert_eq!(parse("RightAlt").to_string(), "RightAlt");
        assert_eq!(parse(""), KeyCombination::default());
    }

    #[test]
    fn rejects_text_that_is_not_a_combination() {
        for text in [
            "Ctrl+",
            "Ctrl+Ctrl+A",
            "A+B",
            "Ctrl+Spacebar",
            "ctrl+a",
            "Hyper+A",
        ] {
            assert_eq!(
                text.parse::<KeyCombination>(),
                Err(ShortcutProblem::Invalid),
                "{text}"
            );
        }
    }

    /// Acceptance test 12, rejected half.
    #[test]
    fn rejects_combinations_that_would_break_typing_or_never_arrive() {
        assert_eq!(check(""), Err(ShortcutProblem::Empty));
        assert_eq!(check("Space"), Err(ShortcutProblem::NeedsModifier));
        assert_eq!(check("A"), Err(ShortcutProblem::NeedsModifier));
        assert_eq!(check("Enter"), Err(ShortcutProblem::NeedsModifier));
        assert_eq!(check("Escape"), Err(ShortcutProblem::EscapeReserved));
        assert_eq!(check("Ctrl"), Err(ShortcutProblem::SingleModifier));
        assert_eq!(check("Shift"), Err(ShortcutProblem::SingleModifier));
        assert_eq!(check("Win"), Err(ShortcutProblem::SingleModifier));
        // Issue #33: right Alt is AltGr, needed for Polish characters; two modifiers at least.
        assert_eq!(check("RightAlt"), Err(ShortcutProblem::SingleModifier));
        assert_eq!(check("RightCtrl"), Err(ShortcutProblem::SingleModifier));
        assert_eq!(
            check("Ctrl+Alt+Delete"),
            Err(ShortcutProblem::ReservedByWindows)
        );
        assert_eq!(check("Win+L"), Err(ShortcutProblem::ReservedByWindows));
        assert_eq!(check("Shift+RightAlt"), Err(ShortcutProblem::Invalid));
    }

    #[test]
    fn the_setting_accepts_only_allowed_combinations_and_stores_them_canonically() {
        let setting: RecordShortcutCombination =
            serde_json::from_str("\"Win+Ctrl\"").expect("allowed");
        assert_eq!(serde_json::to_string(&setting).unwrap(), "\"Ctrl+Win\"");
        assert_eq!(setting.combination(), parse("Ctrl+Win"));
        for invalid in [
            "\"Space\"",
            "\"\"",
            "\"Escape\"",
            "\"Ctrl+Nope\"",
            "\"RightAlt\"",
            "\"RightCtrl\"",
            "42",
        ] {
            assert!(
                serde_json::from_str::<RecordShortcutCombination>(invalid).is_err(),
                "{invalid}"
            );
        }
        assert_eq!(
            RecordShortcutCombination::default().combination(),
            default_record_shortcut()
        );
    }

    #[test]
    fn rejects_the_current_cancel_shortcut() {
        let current = set("Ctrl+Space", "Ctrl+Q", "Ctrl+Alt+D");
        let record = |text: &str| validate_shortcut(Shortcut::Record, &parse(text), &current);
        assert_eq!(record("Ctrl+Q"), Err(ShortcutProblem::SameAsCancel));
        assert_eq!(record("Ctrl+W"), Ok(()));
    }

    // cancel-shortcut.md acceptance test 8.
    #[test]
    fn the_cancel_shortcut_accepts_escape_and_must_differ_from_the_record_shortcut() {
        let current = ShortcutCombinations::default();
        let cancel = |text: &str| validate_shortcut(Shortcut::Cancel, &parse(text), &current);
        assert_eq!(cancel("Escape"), Ok(()));
        assert_eq!(cancel("Ctrl+Q"), Ok(()));
        assert_eq!(cancel("F8"), Ok(()));
        assert_eq!(cancel("Ctrl+Space"), Err(ShortcutProblem::SameAsRecord));
        assert_eq!(
            validate_shortcut(Shortcut::Cancel, &parse("F9"), &set("F9", "Escape", "F10")),
            Err(ShortcutProblem::SameAsRecord)
        );
    }

    // cancel-shortcut.md rule 11: otherwise the Record Shortcut's rules apply.
    #[test]
    fn the_cancel_shortcut_follows_the_record_shortcut_rules() {
        let current = ShortcutCombinations::default();
        let cancel = |text: &str| validate_shortcut(Shortcut::Cancel, &parse(text), &current);
        assert_eq!(cancel(""), Err(ShortcutProblem::Empty));
        assert_eq!(cancel("A"), Err(ShortcutProblem::NeedsModifier));
        assert_eq!(cancel("Shift"), Err(ShortcutProblem::SingleModifier));
        assert_eq!(cancel("Win+L"), Err(ShortcutProblem::ReservedByWindows));
        assert_eq!(cancel("RightAlt"), Err(ShortcutProblem::SingleModifier));
        assert_eq!(cancel("RightCtrl"), Err(ShortcutProblem::SingleModifier));
    }

    #[test]
    fn the_cancel_shortcut_setting_defaults_to_escape_and_accepts_only_allowed_combinations() {
        assert_eq!(
            CancelShortcutCombination::default().combination(),
            default_cancel_shortcut()
        );
        let setting: CancelShortcutCombination =
            serde_json::from_str("\"Shift+Ctrl+Q\"").expect("allowed");
        assert_eq!(serde_json::to_string(&setting).unwrap(), "\"Ctrl+Shift+Q\"");
        assert!(serde_json::from_str::<CancelShortcutCombination>("\"Escape\"").is_ok());
        for invalid in ["\"Q\"", "\"\"", "\"Ctrl\"", "\"Ctrl+Nope\"", "7"] {
            assert!(
                serde_json::from_str::<CancelShortcutCombination>(invalid).is_err(),
                "{invalid}"
            );
        }
    }

    /// Acceptance test 12, accepted half.
    #[test]
    fn accepts_the_allowed_kinds_of_combination() {
        for text in [
            "Ctrl+Space",
            "Ctrl+Alt+R",
            "F9",
            "F24",
            "Pause",
            "ScrollLock",
            "Insert",
            "Ctrl+Win",
            "Ctrl+Shift",
            "Ctrl+Escape",
        ] {
            assert_eq!(check(text), Ok(()), "{text}");
        }
    }

    /// vocabulary-shortcut.md acceptance test 10 (unit half): all three shortcuts differ, the
    /// message names the shortcut that holds the combination, and a turned-off Vocabulary
    /// Shortcut still holds its combination (the set holds it whatever the switch says).
    #[test]
    fn the_three_shortcuts_must_differ_and_the_problem_names_the_holder() {
        let current = set("Ctrl+Space", "Escape", "Ctrl+Alt+D");
        let check = |shortcut, text: &str| validate_shortcut(shortcut, &parse(text), &current);
        assert_eq!(
            check(Shortcut::Record, "Ctrl+Alt+D"),
            Err(ShortcutProblem::SameAsVocabulary)
        );
        assert_eq!(
            check(Shortcut::Cancel, "Ctrl+Alt+D"),
            Err(ShortcutProblem::SameAsVocabulary)
        );
        assert_eq!(
            check(Shortcut::Vocabulary, "Ctrl+Space"),
            Err(ShortcutProblem::SameAsRecord)
        );
        assert_eq!(
            validate_shortcut(
                Shortcut::Vocabulary,
                &parse("Ctrl+Q"),
                &set("Ctrl+Space", "Ctrl+Q", "Ctrl+Alt+D")
            ),
            Err(ShortcutProblem::SameAsCancel)
        );
        // A shortcut never conflicts with its own current combination.
        assert_eq!(check(Shortcut::Vocabulary, "Ctrl+Alt+D"), Ok(()));
        assert_eq!(check(Shortcut::Vocabulary, "Ctrl+Shift+F"), Ok(()));
        // Escape alone stays reserved for the Cancel Shortcut.
        assert_eq!(
            validate_shortcut(
                Shortcut::Vocabulary,
                &parse("Escape"),
                &set("Ctrl+Space", "Ctrl+Q", "Ctrl+Alt+D")
            ),
            Err(ShortcutProblem::EscapeReserved)
        );
    }

    /// vocabulary-shortcut.md acceptance test 11.
    #[test]
    fn editing_combinations_are_rejected_for_every_shortcut() {
        let current = set("F9", "F10", "F11");
        for shortcut in [Shortcut::Record, Shortcut::Cancel, Shortcut::Vocabulary] {
            for text in [
                "Ctrl+C",
                "Ctrl+V",
                "Ctrl+X",
                "Ctrl+Z",
                "Ctrl+Y",
                "Ctrl+A",
                "Ctrl+S",
                "Ctrl+Insert",
                "Shift+Insert",
                "Shift+Delete",
            ] {
                assert_eq!(
                    validate_shortcut(shortcut, &parse(text), &current),
                    Err(ShortcutProblem::EditingCombination),
                    "{shortcut:?} {text}"
                );
            }
            for text in ["Ctrl+Alt+D", "Ctrl+Shift+C"] {
                assert_eq!(
                    validate_shortcut(shortcut, &parse(text), &current),
                    Ok(()),
                    "{shortcut:?} {text}"
                );
            }
        }
        assert!(serde_json::from_str::<VocabularyShortcutCombination>("\"Ctrl+C\"").is_err());
        assert!(serde_json::from_str::<RecordShortcutCombination>("\"Shift+Insert\"").is_err());
        assert!(serde_json::from_str::<CancelShortcutCombination>("\"Ctrl+Z\"").is_err());
    }

    #[test]
    fn the_vocabulary_shortcut_setting_defaults_to_ctrl_alt_d() {
        assert_eq!(
            VocabularyShortcutCombination::default().combination(),
            parse("Ctrl+Alt+D")
        );
        let setting: VocabularyShortcutCombination =
            serde_json::from_str("\"Alt+Ctrl+W\"").expect("allowed");
        assert_eq!(serde_json::to_string(&setting).unwrap(), "\"Ctrl+Alt+W\"");
        for invalid in ["\"D\"", "\"\"", "\"Escape\"", "\"Ctrl\"", "3"] {
            assert!(
                serde_json::from_str::<VocabularyShortcutCombination>(invalid).is_err(),
                "{invalid}"
            );
        }
    }
}
