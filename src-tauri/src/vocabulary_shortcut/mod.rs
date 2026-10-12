//! The **Vocabulary Shortcut** (`docs/specs/vocabulary-shortcut.md`): a global key combination
//! that adds the text selected in any application to the Vocabulary.
//!
//! - The shortcut itself (binding, validation, when a press becomes an attempt) lives with the
//!   other shortcuts in `crate::shortcut` (`record.rs`, `vocabulary.rs`), because they share
//!   the keyboard hook.
//! - [`selection`]: the Selection Reader seam, which copies the selection (rules 6–9).
//! - This module: what an attempt does with the selection — normalise, check, append, and the
//!   Overlay message (rules 10–15).
//! - [`app`]: the Tauri wiring.

pub mod app;
pub mod selection;
#[cfg(windows)]
mod win32;

use std::time::Instant;

use crate::overlay::OverlayMessage;
use crate::settings::SettingsStore;
use crate::shortcut::KeyCombination;
use crate::vocabulary::{EntryCheck, Vocabulary};
use selection::{Selection, SelectionReader};

/// The most words a selection may have (rule 11): entries are names and terms.
pub const MAX_WORDS: usize = 3;

/// Adds `selection` to `vocabulary` if it may be added (rules 10–12) and returns the message to
/// show (rule 13). `None` means nothing was selected.
pub fn add_selection(
    vocabulary: &mut Vocabulary,
    selection: Option<&str>,
    shortcut: &KeyCombination,
) -> OverlayMessage {
    let nothing = || OverlayMessage::VocabularyNothingSelected {
        shortcut: shortcut.to_string(),
    };
    let Some(text) = selection else {
        return nothing();
    };
    match vocabulary.check(text) {
        EntryCheck::Empty => nothing(),
        EntryCheck::TooLong => OverlayMessage::VocabularyTooLong,
        EntryCheck::Duplicate { existing } => {
            OverlayMessage::VocabularyAlreadyPresent { entry: existing }
        }
        EntryCheck::Ok(entry) if entry.split(' ').count() > MAX_WORDS => {
            OverlayMessage::VocabularyTooLong
        }
        EntryCheck::Ok(entry) => {
            vocabulary.push(&EntryCheck::Ok(entry.clone()));
            OverlayMessage::VocabularyAdded { entry }
        }
    }
}

/// Runs one attempt: reads the selection (waiting for the keys until `release_by`), adds it to
/// the Vocabulary in `store` — saved at once, and every window hears of it through the
/// `SettingsChanged` event (rule 12) — and returns the message to show. `None` when the attempt
/// cannot be read at all or the Vocabulary could not be saved.
pub fn run_attempt(
    reader: &mut dyn SelectionReader,
    store: &SettingsStore,
    release_by: Instant,
    shortcut: &KeyCombination,
) -> Option<OverlayMessage> {
    log::info!("Vocabulary Shortcut attempt started");
    let text = match reader.read(release_by) {
        Selection::KeysHeld => {
            log::info!("Vocabulary Shortcut attempt ended: keys still held, nothing copied");
            return Some(OverlayMessage::VocabularyReleaseKeys);
        }
        Selection::Unavailable => {
            log::info!("Vocabulary Shortcut attempt dropped: selections cannot be read");
            return None;
        }
        Selection::Nothing => None,
        Selection::Text(text) => Some(text),
    };
    // Checked inside the update, against the list as it is at that moment. The store writes the
    // file only when the settings changed, so an outcome that adds nothing saves nothing.
    let mut message = None;
    match store.update(|settings| {
        message = Some(add_selection(
            &mut settings.vocabulary,
            text.as_deref(),
            shortcut,
        ));
    }) {
        Ok(_) => {
            if let Some(message) = &message {
                log::info!("Vocabulary Shortcut attempt ended: {}", outcome(message));
            }
            message
        }
        Err(error) => {
            log::error!("could not save the Vocabulary: {error}");
            None
        }
    }
}

/// The kind of an attempt's outcome, without the selection's text (the log's privacy rule).
fn outcome(message: &OverlayMessage) -> &'static str {
    match message {
        OverlayMessage::VocabularyAdded { .. } => "added",
        OverlayMessage::VocabularyAlreadyPresent { .. } => "duplicate",
        OverlayMessage::VocabularyTooLong => "too long",
        OverlayMessage::VocabularyNothingSelected { .. } => "nothing selected",
        OverlayMessage::VocabularyReleaseKeys => "keys held",
        OverlayMessage::Problem { .. } | OverlayMessage::MicrophoneFallback => "other",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::selection::FakeSelectionReader;
    use super::*;
    use crate::settings::Settings;

    fn shortcut() -> KeyCombination {
        "Ctrl+Alt+D".parse().unwrap()
    }

    struct Rig {
        _dir: tempfile::TempDir,
        path: std::path::PathBuf,
        store: SettingsStore,
        reader: FakeSelectionReader,
    }

    impl Rig {
        fn with(entries: &[&str]) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            let mut defaults = Settings::defaults(Some("en-US"));
            defaults.vocabulary = entries
                .iter()
                .map(|e| (*e).to_owned())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let (store, _) = SettingsStore::open(&path, defaults);
            Self {
                _dir: dir,
                path,
                store,
                reader: FakeSelectionReader::new(),
            }
        }

        fn attempt(&mut self, selection: Selection) -> Option<OverlayMessage> {
            self.reader.select(selection);
            let release_by = Instant::now() + Duration::from_secs(2);
            run_attempt(&mut self.reader, &self.store, release_by, &shortcut())
        }

        fn entries(&self) -> Vec<String> {
            self.store.get().vocabulary.entries().to_vec()
        }

        fn saved_entries(&self) -> serde_json::Value {
            let json: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap();
            json["vocabulary"].clone()
        }
    }

    fn text(text: &str) -> Selection {
        Selection::Text(text.into())
    }

    /// Acceptance test 1.
    #[test]
    fn adds_the_normalised_selection_and_saves_it() {
        let mut rig = Rig::with(&["Echo"]);

        let message = rig.attempt(text("  GitHub\t"));

        assert_eq!(
            message,
            Some(OverlayMessage::VocabularyAdded {
                entry: "GitHub".into()
            })
        );
        assert_eq!(
            message.unwrap().action(),
            Some(crate::overlay::MessageAction::OpenVocabulary)
        );
        assert_eq!(rig.entries(), ["Echo", "GitHub"]);
        assert_eq!(rig.saved_entries(), serde_json::json!(["Echo", "GitHub"]));
        assert_eq!(rig.reader.calls(), 1);
    }

    /// Acceptance test 4.
    #[test]
    fn a_duplicate_names_the_stored_spelling_and_changes_nothing() {
        let mut rig = Rig::with(&["GitHub"]);

        assert_eq!(
            rig.attempt(text("github")),
            Some(OverlayMessage::VocabularyAlreadyPresent {
                entry: "GitHub".into()
            })
        );
        assert_eq!(rig.entries(), ["GitHub"]);
    }

    /// Acceptance test 5.
    #[test]
    fn too_long_too_many_words_or_empty_change_nothing() {
        let mut rig = Rig::with(&[]);
        let too_long = rig.attempt(text(&"a".repeat(51)));
        let four_words = rig.attempt(text("one two three four"));
        let empty = rig.attempt(text(" \n\t "));
        let nothing = rig.attempt(Selection::Nothing);

        assert_eq!(too_long, Some(OverlayMessage::VocabularyTooLong));
        assert_eq!(four_words, Some(OverlayMessage::VocabularyTooLong));
        let nothing_selected = Some(OverlayMessage::VocabularyNothingSelected {
            shortcut: "Ctrl+Alt+D".into(),
        });
        assert_eq!(empty, nothing_selected);
        assert_eq!(nothing, nothing_selected);
        assert!(rig.entries().is_empty());

        // 50 characters in 3 words is added.
        let fifty = format!("{} {} {}", "a".repeat(16), "b".repeat(16), "c".repeat(16));
        assert_eq!(fifty.chars().count(), 50);
        assert_eq!(
            rig.attempt(text(&fifty)),
            Some(OverlayMessage::VocabularyAdded {
                entry: fifty.clone()
            })
        );
        assert_eq!(rig.entries(), [fifty]);
    }

    /// Rule 10: added as selected, without changing capitalisation or endings.
    #[test]
    fn the_entry_keeps_its_spelling() {
        let mut rig = Rig::with(&[]);
        rig.attempt(text("Kowalskiego"));
        rig.attempt(text("iPhone\tPro"));
        assert_eq!(rig.entries(), ["Kowalskiego", "iPhone Pro"]);
    }

    /// Without a working clipboard the attempt shows nothing rather than "select a word first",
    /// which would send the user looking for the wrong fix.
    #[test]
    fn an_unavailable_reader_shows_nothing_and_changes_nothing() {
        let rig = Rig::with(&["Echo"]);
        let release_by = Instant::now() + Duration::from_secs(2);

        let message = run_attempt(
            &mut super::selection::UnavailableSelectionReader,
            &rig.store,
            release_by,
            &shortcut(),
        );

        assert_eq!(message, None);
        assert_eq!(rig.entries(), ["Echo"]);
    }

    /// Rule 4 and acceptance test 3: keys still held 10 s after the press end the attempt with
    /// the release-the-keys message, which is not an error and shows with the Overlay off.
    #[test]
    fn keys_held_too_long_ask_for_the_release() {
        let mut rig = Rig::with(&[]);
        let message = rig.attempt(Selection::KeysHeld).expect("a message");
        assert_eq!(message, OverlayMessage::VocabularyReleaseKeys);
        assert!(!message.is_error());
        assert!(message.shows_when_overlay_off());
        assert!(rig.entries().is_empty());
    }
}
