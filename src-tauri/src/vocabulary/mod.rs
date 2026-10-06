//! The **Vocabulary**: the user's ordered list of words and phrases the Engine should favour and
//! spell exactly as listed (`docs/specs/vocabulary.md`).
//!
//! [`Vocabulary`] is the setting (rules 1–7); [`correct`] is the spelling correction for Models
//! without a text prompt (rules 10–11). The hint for prompt-accepting Models is built by the
//! Engine (`engine::model_engine`).

mod correction;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use specta::Type;

pub use correction::correct;

/// The longest entry, in characters, after normalisation (rule 3).
pub const MAX_ENTRY_CHARS: usize = 50;

/// Characters removed from an entry when it is added (rule 2).
const REMOVED_CHARS: [char; 3] = ['<', '>', '"'];

/// What a user typed, as it is stored: trimmed, internal whitespace collapsed to single spaces,
/// and `<`, `>` and `"` removed (rule 2).
pub fn normalise_entry(input: &str) -> String {
    let kept: String = input
        .chars()
        .filter(|c| !REMOVED_CHARS.contains(c))
        .collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The key two entries are compared by for duplicates: case-insensitive (rule 4).
fn duplicate_key(entry: &str) -> String {
    entry.to_lowercase()
}

/// The Vocabulary setting: an ordered list of normalised, non-empty entries of at most
/// [`MAX_ENTRY_CHARS`] characters, without case-insensitive duplicates. Empty by default
/// (rule 1).
///
/// Deserialising accepts exactly such lists, so a stored or patched list that breaks a rule is
/// rejected (`docs/settings.md`). The frontend normalises and checks an entry before adding it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
#[specta(transparent)]
pub struct Vocabulary(Vec<String>);

impl Vocabulary {
    /// The entries in list order.
    pub fn entries(&self) -> &[String] {
        &self.0
    }
}

impl TryFrom<Vec<String>> for Vocabulary {
    type Error = String;

    fn try_from(entries: Vec<String>) -> Result<Self, Self::Error> {
        let mut seen = HashSet::new();
        for entry in &entries {
            if entry.is_empty() {
                return Err("a Vocabulary entry is empty".into());
            }
            if normalise_entry(entry) != *entry {
                return Err(format!("the Vocabulary entry {entry:?} is not normalised"));
            }
            if entry.chars().count() > MAX_ENTRY_CHARS {
                return Err(format!(
                    "the Vocabulary entry {entry:?} is longer than {MAX_ENTRY_CHARS} characters"
                ));
            }
            if !seen.insert(duplicate_key(entry)) {
                return Err(format!("the Vocabulary entry {entry:?} is a duplicate"));
            }
        }
        Ok(Self(entries))
    }
}

impl From<Vocabulary> for Vec<String> {
    fn from(vocabulary: Vocabulary) -> Self {
        vocabulary.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(entries: &[&str]) -> Result<Vocabulary, String> {
        Vocabulary::try_from(entries.iter().map(|e| (*e).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn acceptance_1_normalisation_trims_collapses_and_removes_quotes() {
        assert_eq!(normalise_entry("  \"Claude   Code\" "), "Claude Code");
        assert_eq!(normalise_entry("<Tauri>\t2"), "Tauri 2");
    }

    #[test]
    fn the_default_is_empty() {
        assert!(Vocabulary::default().entries().is_empty());
    }

    #[test]
    fn a_valid_list_keeps_its_order_and_spelling() {
        let vocabulary = list(&["Tauri", "GitHub", "Claude Code", "Łódź"]).unwrap();
        assert_eq!(
            vocabulary.entries(),
            ["Tauri", "GitHub", "Claude Code", "Łódź"]
        );
    }

    #[test]
    fn duplicates_are_rejected_ignoring_case() {
        assert!(list(&["GitHub", "github"]).is_err());
        assert!(list(&["Łódź", "ŁÓDŹ"]).is_err());
    }

    #[test]
    fn entries_of_fifty_characters_are_accepted_and_longer_ones_rejected() {
        let fifty = "a".repeat(50);
        let fifty_one = "a".repeat(51);
        assert!(list(&[&fifty]).is_ok());
        assert!(list(&[&fifty_one]).is_err());
        // Characters, not bytes.
        assert!(list(&[&"ł".repeat(50)]).is_ok());
    }

    #[test]
    fn empty_or_unnormalised_entries_are_rejected() {
        assert!(list(&[""]).is_err());
        assert!(list(&[" Tauri"]).is_err());
        assert!(list(&["Claude  Code"]).is_err());
        assert!(list(&["\"Echo\""]).is_err());
    }

    #[test]
    fn it_round_trips_through_json_as_a_plain_list() {
        let vocabulary = list(&["Echo", "GitHub"]).unwrap();
        let json = serde_json::to_value(&vocabulary).unwrap();
        assert_eq!(json, serde_json::json!(["Echo", "GitHub"]));
        assert_eq!(
            serde_json::from_value::<Vocabulary>(json).unwrap(),
            vocabulary
        );
    }
}
