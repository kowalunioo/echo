//! Transcript clean-up after the Engine returns (`dictation-pipeline.md` rules 25–26).
//!
//! Vocabulary correction (rule 25.1) applies only to Models without a text prompt (Parakeet); the
//! Whisper Models take the Vocabulary as a prompt instead (`vocabulary.md` rules 8 and 10).

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::vocabulary;

/// A word repeated this many times in a row or more collapses to one occurrence (rule 25.2).
const REPEAT_COLLAPSE: usize = 3;

/// Cleans an Engine text. Never loses a Transcript: if a step fails, the uncleaned text is
/// returned (rule 26).
pub fn clean_up(raw: &str) -> String {
    catch_unwind(AssertUnwindSafe(|| collapse(raw))).unwrap_or_else(|_| raw.to_owned())
}

/// Cleans the text of a Model without a text prompt: Vocabulary correction first (rule 25.1),
/// then [`clean_up`]. A failed correction leaves the text uncorrected (`vocabulary.md` rule 11).
pub fn correct_and_clean_up(raw: &str, vocabulary: &[String]) -> String {
    clean_up(&vocabulary::correct(raw, vocabulary))
}

/// Collapses repeated words and whitespace runs and trims the ends (rules 25.2–25.4). Splitting
/// on whitespace and joining with one space does 25.3 and 25.4 at the same time.
fn collapse(raw: &str) -> String {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let mut kept: Vec<&str> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let key = letters(words[i]);
        let mut run = 1;
        if !key.is_empty() {
            while i + run < words.len() && letters(words[i + run]) == key {
                run += 1;
            }
        }
        if run >= REPEAT_COLLAPSE {
            kept.push(words[i]);
        } else {
            kept.extend_from_slice(&words[i..i + run]);
        }
        i += run;
    }
    kept.join(" ")
}

/// A word's identity for repeat detection: its letters, lowercased (rule 25.2 "case-insensitive,
/// letters only").
fn letters(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphabetic())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceptance_13_repeated_words_and_whitespace_are_cleaned() {
        assert_eq!(clean_up("  so so so so we   go  "), "so we go");
    }

    #[test]
    fn three_repeats_collapse_case_insensitively_and_ignoring_punctuation() {
        assert_eq!(clean_up("I I I I think"), "I think");
        assert_eq!(clean_up("No, no NO! stop"), "No, stop");
    }

    #[test]
    fn two_repeats_are_kept() {
        assert_eq!(clean_up("bardzo bardzo dobrze"), "bardzo bardzo dobrze");
    }

    #[test]
    fn words_without_letters_are_never_collapsed() {
        assert_eq!(clean_up("1 1 1 - - -"), "1 1 1 - - -");
    }

    #[test]
    fn line_breaks_and_tabs_become_single_spaces() {
        assert_eq!(clean_up("\tAla\n\nma  kota\r\n"), "Ala ma kota");
    }

    #[test]
    fn vocabulary_correction_comes_before_the_other_steps() {
        let vocabulary = vec!["GitHub".to_owned()];
        assert_eq!(
            correct_and_clean_up("  git  hub git hub git hub ", &vocabulary),
            "GitHub"
        );
    }

    #[test]
    fn whitespace_only_becomes_empty() {
        assert_eq!(clean_up(" \n\t "), "");
    }
}
