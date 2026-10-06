//! Acceptance tolerance for the fixture recordings (`docs/specs/dictation-pipeline.md`,
//! rules 47–53): the normaliser, the normalised word error rate (WER), the thresholds table and
//! the Vocabulary-term condition of `pl-slownik.wav`.
//!
//! Lives with the integration-test helpers so the app binary never ships it; integration tests
//! and the end-to-end fixture suite reach it through `mod common;`.

use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// One row of the rule 50 table, with the expected text from `docs/plan.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fixture {
    /// File name in `tests/fixtures/audio/`.
    pub file: &'static str,
    /// Dictation Language as an ISO 639-1 code.
    pub language: &'static str,
    /// What the recording says; empty for silence.
    pub expected: &'static str,
    /// Number of words in the normalised expected text.
    pub expected_words: usize,
    /// Highest accepted WER; `None` when the Transcript must be empty.
    pub max_wer: Option<f64>,
    /// The error count the threshold stands for, as listed in the table.
    pub allowed_errors: Option<usize>,
    /// Vocabulary to set for the run; every term must appear in the output.
    pub vocabulary: &'static [&'static str],
}

/// The rule 50 table.
pub const FIXTURES: &[Fixture] = &[
    Fixture {
        file: "pl-proste.wav",
        language: "pl",
        expected: "Dzień dobry, to jest test dyktowania w aplikacji Echo.",
        expected_words: 9,
        max_wer: Some(0.12),
        allowed_errors: Some(1),
        vocabulary: &[],
    },
    Fixture {
        file: "pl-interpunkcja.wav",
        language: "pl",
        expected: "Czy jutro o piętnastej możemy się spotkać? Jeśli nie, napisz mi wiadomość.",
        expected_words: 12,
        max_wer: Some(0.17),
        allowed_errors: Some(2),
        vocabulary: &[],
    },
    Fixture {
        file: "pl-slownik.wav",
        language: "pl",
        expected: "Wrzuciłem poprawkę do Echo na GitHuba, a Claude Code przetestował ją w Tauri z \
                   modelem Parakeet na Vulkanie.",
        expected_words: 18,
        max_wer: Some(0.17),
        allowed_errors: Some(3),
        vocabulary: &[
            "Echo",
            "GitHub",
            "Claude Code",
            "Tauri",
            "Parakeet",
            "Vulkan",
        ],
    },
    Fixture {
        file: "pl-liczby.wav",
        language: "pl",
        expected: "Zamówienie numer 4521 kosztuje 129 złotych i 99 groszy.",
        expected_words: 8,
        max_wer: Some(0.13),
        allowed_errors: Some(1),
        vocabulary: &[],
    },
    Fixture {
        file: "en-proste.wav",
        language: "en",
        expected: "This is a quick dictation test in English.",
        expected_words: 8,
        max_wer: Some(0.13),
        allowed_errors: Some(1),
        vocabulary: &[],
    },
    Fixture {
        file: "cisza.wav",
        language: "pl",
        expected: "",
        expected_words: 0,
        max_wer: None,
        allowed_errors: None,
        vocabulary: &[],
    },
];

/// The table row for `file`.
pub fn fixture(file: &str) -> Option<&'static Fixture> {
    FIXTURES.iter().find(|f| f.file == file)
}

impl Fixture {
    /// Whether `actual` meets this row's threshold and extra condition; the error says why not.
    pub fn accepts(&self, actual: &str) -> Result<(), String> {
        let Some(max) = self.max_wer else {
            return if actual.trim().is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "{}: expected an empty Transcript, got {actual:?}",
                    self.file
                ))
            };
        };
        let w = wer(self.expected, actual);
        if w > max {
            return Err(format!("{}: WER {w:.3} > {max} for {actual:?}", self.file));
        }
        let missing = missing_terms(self.vocabulary, &normalise(actual));
        if !missing.is_empty() {
            return Err(format!(
                "{}: Vocabulary terms missing {missing:?} in {actual:?}",
                self.file
            ));
        }
        Ok(())
    }
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("valid pattern")
}

static DECIMAL_AMOUNT: LazyLock<Regex> =
    LazyLock::new(|| re(r"\b(\d+)[,.](\d{1,2})\s*(?:zł|złoty|złote|złotych)\b"));
static ZLOTY: LazyLock<Regex> = LazyLock::new(|| re(r"\b(?:zł|złoty|złote|złotych)\b"));
static GROSZ: LazyLock<Regex> = LazyLock::new(|| re(r"\b(?:gr|grosz|grosze|groszy)\b"));
static ZL_AND_NUMBER: LazyLock<Regex> = LazyLock::new(|| re(r"\bzl\s+i\s+(\d)"));
static NR: LazyLock<Regex> = LazyLock::new(|| re(r"\bnr\b"));
static THOUSANDS: LazyLock<Regex> = LazyLock::new(|| re(r"(\d)[ ,.](\d{3})\b"));
static FULL_HOUR: LazyLock<Regex> = LazyLock::new(|| re(r"\b(\d{1,2})[:.]00\b"));

/// Normalises a text into words (rule 49, steps applied in order).
pub fn normalise(text: &str) -> Vec<String> {
    // 1. NFC, lowercase.
    let text: String = text.nfc().collect::<String>().to_lowercase();
    // 2. Currency; decimal amounts first, so their currency word is consumed with them.
    let text = DECIMAL_AMOUNT.replace_all(&text, "$1 zl $2 gr");
    let text = ZLOTY.replace_all(&text, "zl");
    let text = GROSZ.replace_all(&text, "gr");
    // 3. "i" between zl and a number.
    let text = ZL_AND_NUMBER.replace_all(&text, "zl $1");
    // 4. "nr".
    let text = NR.replace_all(&text, "numer");
    // 5. Thousands separators; repeated because matches cannot overlap ("1 234 567").
    let mut text = text.into_owned();
    loop {
        let next = THOUSANDS.replace_all(&text, "$1$2").into_owned();
        if next == text {
            break;
        }
        text = next;
    }
    // 6. Full hours.
    let text = FULL_HOUR.replace_all(&text, "$1");
    // 7. Anything but letters, digits and whitespace becomes a space.
    let text: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    // 8. Collapse and trim whitespace.
    text.split_whitespace().map(str::to_owned).collect()
}

/// Normalised word error rate (rule 48): word-level edit distance between the normalised texts
/// divided by the normalised expected word count. With no expected words it is 0 for empty
/// output and infinite otherwise.
pub fn wer(expected: &str, actual: &str) -> f64 {
    let expected = normalise(expected);
    let actual = normalise(actual);
    if expected.is_empty() {
        return if actual.is_empty() {
            0.0
        } else {
            f64::INFINITY
        };
    }
    edit_distance(&expected, &actual) as f64 / expected.len() as f64
}

fn edit_distance(a: &[String], b: &[String]) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, wa) in a.iter().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, wb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(wa != wb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        previous = current;
    }
    previous[b.len()]
}

/// The Vocabulary terms that do not appear in the normalised output (rule 50, `pl-slownik.wav`):
/// a term is present when each of its normalised words is the start of a word in `actual`, and a
/// multi-word term's words are consecutive.
pub fn missing_terms<'a>(terms: &[&'a str], actual: &[String]) -> Vec<&'a str> {
    terms
        .iter()
        .copied()
        .filter(|term| {
            let parts = normalise(term);
            !actual.windows(parts.len().max(1)).any(|window| {
                window
                    .iter()
                    .zip(&parts)
                    .all(|(w, p)| w.starts_with(p.as_str()))
            })
        })
        .collect()
}
