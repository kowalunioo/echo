//! Vocabulary spelling correction for Models without a text prompt (`vocabulary.md` rules 10–11).
//!
//! The Transcript is split into whitespace-separated words. At each word, groups of one to three
//! consecutive words are compared with every eligible entry, ignoring case, spaces and
//! punctuation; the closest qualifying entry replaces the group, keeping the punctuation around
//! it, the capitalisation pattern of the transcript and any Polish ending. Groups are taken left
//! to right; a replaced group is not looked at again.

use std::panic::{AssertUnwindSafe, catch_unwind};

/// Edit distance allowed, as a share of the longer length (rule 10.3).
const TOLERANCE: f64 = 0.18;
/// Edit distance allowed when both sides sound alike (rule 10.3, "proportionally more").
const PHONETIC_TOLERANCE: f64 = 0.36;
/// The extra phonetic tolerance applies only from this length on: shorter words have too few
/// letters for a sound-alike to mean anything (with 18% they must already match exactly).
const PHONETIC_MIN_LEN: usize = 6;
/// Lengths may differ by this share of the longer length...
const LENGTH_SHARE: f64 = 0.25;
/// ...or by this many characters, whichever is larger (rule 10.3).
const LENGTH_SLACK: f64 = 2.0;
/// The most transcript words one entry can match (rule 10.2).
const MAX_GROUP: usize = 3;
/// Polish endings kept after a corrected stem (rule 10.8).
const POLISH_ENDINGS: [&str; 9] = ["a", "ie", "em", "u", "owi", "y", "ach", "ami", "om"];

/// Corrects words in `transcript` that closely resemble an entry of `vocabulary` (rule 10).
/// Never fails: if correction panics, the uncorrected Transcript is returned (rule 11).
pub fn correct(transcript: &str, vocabulary: &[String]) -> String {
    guarded(transcript, || correct_unguarded(transcript, vocabulary))
}

/// Runs `correction`, falling back to `transcript` if it panics (rule 11).
fn guarded(transcript: &str, correction: impl FnOnce() -> String) -> String {
    catch_unwind(AssertUnwindSafe(correction)).unwrap_or_else(|_| {
        log::warn!("Vocabulary correction failed; the uncorrected Transcript is used");
        transcript.to_owned()
    })
}

fn correct_unguarded(transcript: &str, vocabulary: &[String]) -> String {
    let entries: Vec<Entry<'_>> = vocabulary.iter().filter_map(|e| Entry::new(e)).collect();
    if entries.is_empty() {
        return transcript.to_owned();
    }
    let words = words(transcript);
    let mut out = String::with_capacity(transcript.len());
    let mut cursor = 0;
    let mut i = 0;
    while i < words.len() {
        match best_match(&words[i..], &entries) {
            // A group whose first word adds nothing ("w tauri", "z github") leaves that word
            // alone: the entry matches at least as closely from the next word on.
            Some(found)
                if found.words > 1
                    && best_match(&words[i + 1..], &entries)
                        .is_some_and(|next| next.distance <= found.distance) =>
            {
                i += 1;
            }
            Some(found) => {
                let first = &words[i];
                let last = &words[i + found.words - 1];
                out.push_str(&transcript[cursor..first.start]);
                out.push_str(&replacement(&words[i..i + found.words], &found, &entries));
                cursor = last.end;
                i += found.words;
            }
            None => i += 1,
        }
    }
    out.push_str(&transcript[cursor..]);
    out
}

/// One whitespace-separated transcript word: punctuation before, the core (first to last letter
/// or digit), punctuation after.
#[derive(Debug)]
struct Word<'a> {
    start: usize,
    end: usize,
    lead: &'a str,
    core: &'a str,
    trail: &'a str,
}

fn words(text: &str) -> Vec<Word<'_>> {
    text.split_whitespace()
        .map(|token| {
            let start = token.as_ptr() as usize - text.as_ptr() as usize;
            let core_start = token
                .char_indices()
                .find(|(_, c)| c.is_alphanumeric())
                .map_or(token.len(), |(i, _)| i);
            let core_end = token
                .char_indices()
                .rev()
                .find(|(_, c)| c.is_alphanumeric())
                .map_or(core_start, |(i, c)| i + c.len_utf8());
            Word {
                start,
                end: start + token.len(),
                lead: &token[..core_start],
                core: &token[core_start..core_end.max(core_start)],
                trail: &token[core_end.max(core_start)..],
            }
        })
        .collect()
}

/// A comparison key: letters and digits only, lowercased (rule 10.2).
fn key(text: &str) -> Vec<char> {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// An entry taking part in the correction, with the keys it can be matched by.
struct Entry<'a> {
    spelling: &'a str,
    /// The entry's key and, for an entry with "&", the key of its spoken form with "and"
    /// (rule 10.7).
    keys: Vec<Key>,
}

struct Key {
    chars: Vec<char>,
    sound: String,
}

impl Key {
    fn new(chars: Vec<char>) -> Self {
        let sound = sound(&chars);
        Self { chars, sound }
    }
}

impl<'a> Entry<'a> {
    /// The entry, or `None` if it does not take part: only entries made of ASCII letters and
    /// digits, ignoring spaces and punctuation, are used (rule 10.1).
    fn new(spelling: &'a str) -> Option<Self> {
        let ascii = spelling
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || !c.is_alphanumeric());
        let chars = key(spelling);
        if !ascii || chars.is_empty() {
            return None;
        }
        let mut keys = vec![Key::new(chars)];
        if spelling.contains('&') {
            keys.push(Key::new(key(&spelling.replace('&', " and "))));
        }
        Some(Self { spelling, keys })
    }
}

/// The best entry for a group starting at the first of `words`.
#[derive(Debug, Clone, Copy)]
struct Match {
    entry: usize,
    /// How many transcript words the entry replaces.
    words: usize,
    /// How many characters at the end of the last word are a kept Polish ending.
    ending: usize,
    /// Edit distance divided by the longer length; lower is closer.
    distance: f64,
}

impl Match {
    /// Closest first; on a tie prefer more words, then no ending, then the earlier entry.
    fn better_than(&self, other: &Match) -> bool {
        (
            self.distance,
            std::cmp::Reverse(self.words),
            self.ending,
            self.entry,
        ) < (
            other.distance,
            std::cmp::Reverse(other.words),
            other.ending,
            other.entry,
        )
    }
}

fn best_match(words: &[Word<'_>], entries: &[Entry<'_>]) -> Option<Match> {
    let mut best: Option<Match> = None;
    for size in 1..=MAX_GROUP.min(words.len()) {
        let group = &words[..size];
        if !is_group(group) {
            break;
        }
        let group_key: Vec<char> = group.iter().flat_map(|w| key(w.core)).collect();
        let last_core = key(group[size - 1].core);
        for (index, entry) in entries.iter().enumerate() {
            for entry_key in &entry.keys {
                let mut consider = |ending: usize| {
                    let stem = &group_key[..group_key.len() - ending];
                    if let Some(distance) = similarity(stem, entry_key) {
                        let found = Match {
                            entry: index,
                            words: size,
                            ending,
                            distance,
                        };
                        if best.is_none_or(|b| found.better_than(&b)) {
                            best = Some(found);
                        }
                    }
                };
                consider(0);
                for ending in POLISH_ENDINGS {
                    let len = ending.len();
                    if last_core.len() > len
                        && last_core[last_core.len() - len..]
                            .iter()
                            .copied()
                            .eq(ending.chars())
                        && original_ending(group[size - 1].core, len).is_some()
                    {
                        consider(len);
                    }
                }
            }
        }
    }
    best
}

/// Whether `group` can match one entry: every word has letters or digits, and no punctuation
/// sits between the words (rule 10.4).
fn is_group(group: &[Word<'_>]) -> bool {
    group.iter().all(|w| !w.core.is_empty())
        && group[..group.len() - 1].iter().all(|w| w.trail.is_empty())
        && group[1..].iter().all(|w| w.lead.is_empty())
}

/// The distance of `text` from `entry` divided by the longer length, if they are similar enough
/// (rule 10.3), `None` otherwise.
fn similarity(text: &[char], entry: &Key) -> Option<f64> {
    let longer = text.len().max(entry.chars.len());
    if text.is_empty() || longer == 0 {
        return None;
    }
    let longer_f = longer as f64;
    let length_gap = text.len().abs_diff(entry.chars.len()) as f64;
    if length_gap > (LENGTH_SHARE * longer_f).max(LENGTH_SLACK) {
        return None;
    }
    let distance = edit_distance(text, &entry.chars) as f64;
    let tolerance = if longer >= PHONETIC_MIN_LEN && sound(text) == entry.sound {
        PHONETIC_TOLERANCE
    } else {
        TOLERANCE
    };
    (distance <= tolerance * longer_f).then_some(distance / longer_f)
}

/// Levenshtein distance: insertions, deletions and substitutions of one character.
fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// A rough English sound key, so that words spelled differently but pronounced alike ("cloud
/// code" / "claude code", "charge b" / "chargebee") compare equal: letters that sound the same
/// are unified, repeated letters collapse, and vowels and "h" are dropped after the first letter.
fn sound(key: &[char]) -> String {
    let mut mapped: Vec<char> = Vec::with_capacity(key.len() + 2);
    let mut i = 0;
    while i < key.len() {
        let next = key.get(i + 1).copied();
        match (key[i], next) {
            ('p', Some('h')) => {
                mapped.push('f');
                i += 2;
                continue;
            }
            ('c', Some('k')) => {
                mapped.push('k');
                i += 2;
                continue;
            }
            ('c' | 'q', _) => mapped.push('k'),
            ('x', _) => mapped.extend(['k', 's']),
            ('z', _) => mapped.push('s'),
            ('w', _) => mapped.push('v'),
            ('y', _) => mapped.push('i'),
            (other, _) => mapped.push(other),
        }
        i += 1;
    }
    mapped.dedup();
    mapped
        .iter()
        .enumerate()
        .filter(|(index, c)| *index == 0 || !matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'h'))
        .map(|(_, c)| *c)
        .collect()
}

/// The last `chars` characters of `core` as the transcript spells them, if they are letters.
fn original_ending(core: &str, chars: usize) -> Option<&str> {
    let (start, _) = core.char_indices().rev().nth(chars - 1)?;
    let ending = &core[start..];
    ending.chars().all(char::is_alphabetic).then_some(ending)
}

/// The text replacing `group`: the punctuation before and after it, the entry in the group's
/// capitalisation pattern (rule 10.6), and the kept Polish ending (rule 10.8).
fn replacement(group: &[Word<'_>], found: &Match, entries: &[Entry<'_>]) -> String {
    let first = &group[0];
    let last = &group[group.len() - 1];
    let mut text = String::from(first.lead);
    text.push_str(&cased(entries[found.entry].spelling, group));
    if found.ending > 0 {
        text.push_str(original_ending(last.core, found.ending).unwrap_or_default());
    }
    text.push_str(last.trail);
    text
}

/// `spelling` capitalised like the transcript words it replaces (rule 10.6): all capitals stay
/// all capitals, a capitalised first word capitalises the entry, otherwise the entry's own
/// spelling is used.
fn cased(spelling: &str, group: &[Word<'_>]) -> String {
    let cased_letters: Vec<char> = group
        .iter()
        .flat_map(|w| w.core.chars())
        .filter(|c| c.is_uppercase() || c.is_lowercase())
        .collect();
    let all_capitals = cased_letters.len() >= 2 && cased_letters.iter().all(|c| c.is_uppercase());
    if all_capitals {
        return spelling.to_uppercase();
    }
    let capitalised = group[0].core.chars().next().is_some_and(char::is_uppercase);
    if capitalised {
        let mut chars = spelling.chars();
        if let Some(first) = chars.next() {
            return first.to_uppercase().chain(chars).collect();
        }
    }
    spelling.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(transcript: &str, entries: &[&str]) -> String {
        let vocabulary: Vec<String> = entries.iter().map(|e| (*e).to_owned()).collect();
        correct(transcript, &vocabulary)
    }

    // --- acceptance tests of vocabulary.md ---------------------------------------------------

    #[test]
    fn acceptance_7_words_resembling_entries_are_corrected() {
        assert_eq!(
            fix("I pushed it to git hub, then tauri.", &["GitHub", "Tauri"]),
            "I pushed it to GitHub, then Tauri."
        );
    }

    #[test]
    fn acceptance_8_case_pattern_follows_the_transcript_word() {
        assert_eq!(fix("GITHUB", &["GitHub"]), "GITHUB");
        assert_eq!(fix("Github", &["GitHub"]), "GitHub");
        assert_eq!(fix("github", &["GitHub"]), "GitHub");
    }

    #[test]
    fn acceptance_9_a_match_never_spans_punctuation_inside_the_group() {
        assert_eq!(fix("Charge B, che", &["ChargeBee"]), "ChargeBee, che");
        assert_eq!(fix("Charge B,", &["ChargeBee"]), "ChargeBee,");
    }

    #[test]
    fn acceptance_10_entries_with_non_ascii_letters_are_skipped() {
        assert_eq!(fix("byłem w łódź", &["Łódź"]), "byłem w łódź");
        assert_eq!(fix("byłem w lodz", &["Łódź"]), "byłem w lodz");
    }

    #[test]
    fn acceptance_10a_polish_endings_are_kept() {
        let entries = ["GitHub", "Vulkan"];
        assert_eq!(
            fix("wrzuciłem na githuba", &entries),
            "wrzuciłem na GitHuba"
        );
        assert_eq!(fix("na vulkanie", &entries), "na Vulkanie");
    }

    // --- rule 10.1: eligible entries ------------------------------------------------------------

    #[test]
    fn non_ascii_entries_are_skipped_but_the_others_still_apply() {
        assert_eq!(
            fix("z kraków na github", &["Kraków", "GitHub"]),
            "z kraków na GitHub"
        );
    }

    #[test]
    fn punctuation_and_spaces_inside_an_entry_do_not_exclude_it() {
        assert_eq!(
            fix("we use node js here", &["Node.js"]),
            "we use Node.js here"
        );
    }

    #[test]
    fn entries_without_letters_or_digits_are_skipped() {
        assert_eq!(fix("a - b", &["-"]), "a - b");
    }

    #[test]
    fn an_empty_vocabulary_or_transcript_changes_nothing() {
        assert_eq!(fix("  git  hub ", &[]), "  git  hub ");
        assert_eq!(fix("", &["GitHub"]), "");
    }

    // --- rule 10.2: one to three words ------------------------------------------------------------

    #[test]
    fn up_to_three_words_match_one_entry() {
        assert_eq!(
            fix("Charge B is billing", &["ChargeBee"]),
            "ChargeBee is billing"
        );
        assert_eq!(
            fix("then cloud code tested it", &["Claude Code"]),
            "then Claude Code tested it"
        );
        assert_eq!(fix("ask open a i now", &["OpenAI"]), "ask OpenAI now");
    }

    #[test]
    fn four_words_never_match_one_entry() {
        assert_eq!(fix("o p e n", &["Open"]), "o p e n");
    }

    #[test]
    fn a_short_word_before_an_entry_is_not_swallowed() {
        assert_eq!(fix("w tauri,", &["Tauri"]), "w Tauri,");
        assert_eq!(fix("z github", &["GitHub"]), "z GitHub");
        assert_eq!(fix("i echo", &["Echo"]), "i Echo");
    }

    #[test]
    fn whitespace_outside_a_match_is_kept() {
        assert_eq!(fix("on  git hub\tnow", &["GitHub"]), "on  GitHub\tnow");
    }

    // --- rule 10.3: edit distance, length and sound ---------------------------------------------

    #[test]
    fn a_small_edit_distance_is_accepted() {
        // 1 edit in 8 letters is within 18%.
        assert_eq!(
            fix("tested in parakeat", &["Parakeet"]),
            "tested in Parakeet"
        );
    }

    #[test]
    fn a_larger_edit_distance_is_rejected() {
        // GitLab is 2 edits from GitHub (33%) and does not sound alike.
        assert_eq!(fix("on gitlab", &["GitHub"]), "on gitlab");
    }

    #[test]
    fn short_words_must_match_exactly() {
        // 18% of 4 letters allows no edit, and sound tolerance needs 6 letters.
        assert_eq!(fix("echa", &["Echo"]), "echa");
        assert_eq!(fix("to echo", &["Echo"]), "to Echo");
    }

    #[test]
    fn words_that_sound_alike_get_more_tolerance() {
        // "parakit" is 2 edits from "parakeet" (25%), but sounds the same.
        assert_eq!(fix("model parakit", &["Parakeet"]), "model Parakeet");
        // Without the same sound, 25% is too far.
        assert_eq!(fix("model paraken", &["Parakeet"]), "model paraken");
    }

    #[test]
    fn lengths_may_differ_by_at_most_a_quarter_or_two_characters() {
        // "git" against "github": 3 characters apart, more than 2 and more than 25% of 6.
        assert_eq!(fix("git", &["GitHub"]), "git");
        // A long word with the entry inside it is not the entry.
        assert_eq!(fix("vulkanizacja", &["Vulkan"]), "vulkanizacja");
    }

    #[test]
    fn the_closest_entry_wins() {
        assert_eq!(fix("paraket", &["Parakeet", "Paraket"]), "Paraket");
        assert_eq!(fix("parakeet", &["Paraket", "Parakeet"]), "Parakeet");
    }

    #[test]
    fn edit_distance_counts_insertions_deletions_and_substitutions() {
        let d = |a: &str, b: &str| {
            edit_distance(
                &a.chars().collect::<Vec<_>>(),
                &b.chars().collect::<Vec<_>>(),
            )
        };
        assert_eq!(d("kitten", "sitting"), 3);
        assert_eq!(d("", "abc"), 3);
        assert_eq!(d("github", "github"), 0);
        assert_eq!(d("łódź", "lodz"), 3);
    }

    #[test]
    fn sound_keys_unify_letters_that_sound_alike() {
        let s = |text: &str| sound(&key(text));
        assert_eq!(s("cloud code"), s("Claude Code"));
        assert_eq!(s("charge b"), s("ChargeBee"));
        assert_eq!(s("fone"), s("phone"));
        assert_ne!(s("gitlab"), s("github"));
    }

    // --- rules 10.4 and 10.5: punctuation ----------------------------------------------------------

    #[test]
    fn punctuation_before_and_after_the_match_is_kept() {
        assert_eq!(fix("(github).", &["GitHub"]), "(GitHub).");
        assert_eq!(fix("\"git hub?\"", &["GitHub"]), "\"GitHub?\"");
    }

    #[test]
    fn punctuation_before_a_later_word_ends_the_group() {
        assert_eq!(fix("git (hub)", &["GitHub"]), "git (hub)");
        assert_eq!(fix("git, hub", &["GitHub"]), "git, hub");
    }

    #[test]
    fn punctuation_inside_a_word_is_ignored_for_comparison() {
        assert_eq!(fix("on git-hub now", &["GitHub"]), "on GitHub now");
    }

    // --- rule 10.6: capitalisation ------------------------------------------------------------

    #[test]
    fn capitalisation_of_a_multi_word_match_follows_its_words() {
        assert_eq!(fix("CLOUD CODE", &["Claude Code"]), "CLAUDE CODE");
        assert_eq!(fix("Cloud code", &["Claude Code"]), "Claude Code");
        assert_eq!(fix("Iphone", &["iPhone"]), "IPhone");
        assert_eq!(fix("iphone", &["iPhone"]), "iPhone");
    }

    // --- rule 10.7: "&" and "and" ---------------------------------------------------------------

    #[test]
    fn an_entry_with_ampersand_matches_the_spoken_and() {
        assert_eq!(fix("we did R and D today", &["R&D"]), "we did R&D today");
        assert_eq!(
            fix("procter and gamble", &["Procter & Gamble"]),
            "Procter & Gamble"
        );
        assert_eq!(fix("we did r&d", &["R&D"]), "we did R&D");
    }

    // --- rule 10.8: Polish endings ----------------------------------------------------------------

    #[test]
    fn every_listed_polish_ending_is_kept() {
        for ending in POLISH_ENDINGS {
            assert_eq!(
                fix(&format!("github{ending}"), &["GitHub"]),
                format!("GitHub{ending}"),
                "ending -{ending}"
            );
        }
    }

    #[test]
    fn a_polish_ending_follows_the_case_pattern_and_keeps_punctuation() {
        assert_eq!(fix("Githuba.", &["GitHub"]), "GitHuba.");
        assert_eq!(fix("GITHUBA", &["GitHub"]), "GITHUBA");
        assert_eq!(fix("w tauri,", &["Tauri"]), "w Tauri,");
    }

    #[test]
    fn a_polish_ending_on_a_sound_alike_stem_is_kept() {
        assert_eq!(fix("na wulkanie", &["Vulkan"]), "na Vulkanie");
        assert_eq!(fix("z wulkanem", &["Vulkan"]), "z Vulkanem");
        assert_eq!(fix("z parakitem", &["Parakeet"]), "z Parakeetem");
    }

    #[test]
    fn unlisted_endings_are_not_kept_as_endings() {
        // "-ów" is not a listed ending, and the whole word is too far from the entry.
        assert_eq!(fix("githubów", &["GitHub"]), "githubów");
    }

    // --- rule 11: never fails -------------------------------------------------------------------

    #[test]
    fn a_failing_correction_falls_back_to_the_uncorrected_transcript() {
        let result = guarded("git hub", || panic!("correction bug"));
        assert_eq!(result, "git hub");
    }

    #[test]
    fn unusual_text_never_panics() {
        let entries = ["GitHub", "R&D", "Claude Code"];
        for text in [
            "…",
            " , , ",
            "a\u{301}b",
            "ǅungla",
            "日本語 github",
            "&&&",
            "1 2 3 4",
        ] {
            let _ = fix(text, &entries);
        }
    }
}
