//! Unit tests for the fixture acceptance tolerance (`dictation-pipeline.md` rules 48–50,
//! acceptance test 18): the normaliser, the normalised word error rate and the Vocabulary-term
//! condition of `pl-slownik.wav`.

mod common;

use common::acceptance::{FIXTURES, fixture, missing_terms, normalise, wer};

fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

// Rule 49.1 — Unicode NFC and lowercase.

#[test]
fn lowercases() {
    assert_eq!(normalise("Dzień DOBRY Echo"), words("dzień dobry echo"));
}

#[test]
fn composes_to_nfc() {
    // "z" + combining dot above, "o" + combining acute: decomposed forms of "żó".
    let decomposed = "Z\u{0307}o\u{0301}łw";
    assert_eq!(normalise(decomposed), vec!["żółw".to_owned()]);
}

// Rule 49.2 — currency.

#[test]
fn zloty_words_become_zl() {
    assert_eq!(normalise("zł złoty złote złotych"), words("zl zl zl zl"),);
}

#[test]
fn grosz_words_become_gr() {
    assert_eq!(normalise("gr grosz grosze groszy"), words("gr gr gr gr"));
}

#[test]
fn currency_words_match_whole_words_only() {
    assert_eq!(
        normalise("złotówka grosik grupa"),
        words("złotówka grosik grupa")
    );
}

#[test]
fn decimal_amount_with_comma_splits_into_zloty_and_grosze() {
    assert_eq!(normalise("129,99 zł"), words("129 zl 99 gr"));
}

#[test]
fn decimal_amount_with_dot_splits_into_zloty_and_grosze() {
    assert_eq!(normalise("129.99 zł."), words("129 zl 99 gr"));
}

#[test]
fn decimal_amount_before_a_long_currency_word() {
    assert_eq!(normalise("129,99 złotych"), words("129 zl 99 gr"));
}

#[test]
fn spelled_amount_matches_decimal_amount() {
    assert_eq!(normalise("129 złotych i 99 groszy"), words("129 zl 99 gr"));
    assert_eq!(normalise("129 złotych i 99 groszy"), normalise("129,99 zł"));
}

// Rule 49.3 — "i" between zl and a number.

#[test]
fn and_is_removed_only_between_zl_and_a_number() {
    assert_eq!(normalise("5 zł i 20 gr"), words("5 zl 20 gr"));
    assert_eq!(normalise("5 zł i dwadzieścia"), words("5 zl i dwadzieścia"));
    assert_eq!(normalise("kot i 20 psów"), words("kot i 20 psów"));
}

// Rule 49.4 — "nr".

#[test]
fn nr_becomes_numer() {
    assert_eq!(normalise("Zamówienie nr 7"), words("zamówienie numer 7"));
    assert_eq!(normalise("nr. 7"), words("numer 7"));
    assert_eq!(normalise("nrx"), words("nrx"));
}

// Rule 49.5 — thousands separators.

#[test]
fn thousands_separators_are_removed() {
    for text in ["4 521", "4,521", "4.521"] {
        assert_eq!(normalise(text), words("4521"), "input {text:?}");
    }
}

#[test]
fn several_thousands_groups_are_joined() {
    assert_eq!(normalise("1 234 567"), words("1234567"));
}

#[test]
fn groups_that_are_not_three_digits_stay_separate() {
    assert_eq!(normalise("4 52"), words("4 52"));
    assert_eq!(normalise("4,5210"), words("4 5210"));
}

// Rule 49.6 — times.

#[test]
fn full_hours_lose_their_minutes() {
    assert_eq!(normalise("o 15:00"), words("o 15"));
    assert_eq!(normalise("o 15.00"), words("o 15"));
}

// Rule 49.7 — punctuation.

#[test]
fn punctuation_becomes_space() {
    assert_eq!(
        normalise("Czy jutro? Tak, oczywiście! (raz-dwa)"),
        words("czy jutro tak oczywiście raz dwa"),
    );
}

#[test]
fn polish_quotation_marks_are_stripped() {
    assert_eq!(normalise("„Echo” i «Tauri»"), words("echo i tauri"));
}

// Rule 49.8 — whitespace.

#[test]
fn whitespace_collapses_and_is_trimmed() {
    assert_eq!(normalise("  raz \t dwa\n\n trzy  "), words("raz dwa trzy"));
    assert!(normalise("  .,!  ").is_empty());
}

// Rule 49.9 — spelled-out numbers stay as words.

#[test]
fn spelled_out_numbers_are_not_converted() {
    assert_eq!(normalise("o piętnastej"), words("o piętnastej"));
}

// Rule 48 — WER.

#[test]
fn identical_texts_have_zero_wer() {
    assert_eq!(wer("Dzień dobry.", "dzień   dobry"), 0.0);
}

#[test]
fn wer_counts_substitutions_deletions_and_insertions() {
    // Expected 4 words; one substitution, one deletion, one insertion.
    let w = wer("raz dwa trzy cztery", "raz dwie cztery pięć");
    assert!((w - 3.0 / 4.0).abs() < 1e-9, "got {w}");
}

#[test]
fn empty_actual_is_all_deletions() {
    assert_eq!(wer("raz dwa", ""), 1.0);
}

#[test]
fn empty_expected_and_empty_actual_is_zero() {
    assert_eq!(wer("", " . "), 0.0);
}

#[test]
fn empty_expected_with_any_output_is_infinite() {
    assert_eq!(wer("", "dziękuję"), f64::INFINITY);
}

// Rule 50 — the thresholds table as data.

#[test]
fn expected_word_counts_match_the_table() {
    for f in FIXTURES {
        assert_eq!(normalise(f.expected).len(), f.expected_words, "{}", f.file);
    }
}

#[test]
fn max_wer_allows_exactly_the_listed_number_of_errors() {
    for f in FIXTURES.iter().filter(|f| f.max_wer.is_some()) {
        let max = f.max_wer.unwrap();
        let allowed = f.allowed_errors.unwrap();
        let n = f.expected_words as f64;
        assert!(allowed as f64 / n <= max, "{}", f.file);
        assert!((allowed + 1) as f64 / n > max, "{}", f.file);
    }
}

#[test]
fn silence_fixture_expects_empty_output() {
    let cisza = fixture("cisza.wav").unwrap();
    assert_eq!(cisza.expected_words, 0);
    assert_eq!(cisza.max_wer, None);
    assert!(cisza.accepts("").is_ok());
    assert!(cisza.accepts("Dziękuję.").is_err());
}

// Rule 50 — Vocabulary terms for pl-slownik.wav.

#[test]
fn inflected_terms_count() {
    let actual = normalise("Na GitHuba, w Vulkanie.");
    assert!(missing_terms(&["GitHub", "Vulkan"], &actual).is_empty());
}

#[test]
fn missing_term_is_reported() {
    let actual = normalise("Echo na GitLabie");
    assert_eq!(missing_terms(&["Echo", "GitHub"], &actual), vec!["GitHub"]);
}

#[test]
fn multi_word_term_needs_consecutive_words() {
    let terms = ["Claude Code"];
    assert!(missing_terms(&terms, &normalise("a Claude Code przetestował")).is_empty());
    assert_eq!(
        missing_terms(&terms, &normalise("Claude, to Code")),
        vec!["Claude Code"]
    );
    assert_eq!(
        missing_terms(&terms, &normalise("Code Claude")),
        vec!["Claude Code"]
    );
}

// Real large-v3-turbo outputs from the Engine run must pass.

#[test]
fn real_turbo_output_for_pl_slownik_passes() {
    let actual = "Wrzuciłem poprawkę Echo na GitHub, a Claude Code przetestował ją w Tauri z \
                  modelem Parakeet na Vulkanie.";
    let f = fixture("pl-slownik.wav").unwrap();
    assert!((wer(f.expected, actual) - 2.0 / 18.0).abs() < 1e-9);
    assert_eq!(f.accepts(actual), Ok(()));
}

#[test]
fn real_turbo_output_for_pl_liczby_passes() {
    let actual = "Zamówienie numer 4521 kosztuje 129,99 zł.";
    let f = fixture("pl-liczby.wav").unwrap();
    assert_eq!(wer(f.expected, actual), 0.0);
    assert_eq!(f.accepts(actual), Ok(()));
}

#[test]
fn too_many_errors_are_rejected() {
    let f = fixture("pl-liczby.wav").unwrap();
    assert!(f.accepts("Zamówienie 4521 kosztuje 129 zł").is_err());
}

#[test]
fn missing_vocabulary_term_is_rejected_even_under_the_wer_threshold() {
    let f = fixture("pl-slownik.wav").unwrap();
    let actual = "Wrzuciłem poprawkę do Echo na GitHuba, a Claude Code przetestował ją w Tauri z \
                  modelem Parakit na Vulkanie.";
    assert!(wer(f.expected, actual) <= f.max_wer.unwrap());
    assert!(f.accepts(actual).is_err());
}
