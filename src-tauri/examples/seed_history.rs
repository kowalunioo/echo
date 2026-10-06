//! Fills a History database with sample entries, for screenshots and manual testing of the
//! History page. Development only: examples are not part of the app.
//!
//! ```text
//! cargo run --example seed_history -- <path to history.db>
//! ```
//!
//! Point it at a dev data directory (e.g. one created by `bun tauri dev` with another
//! identifier), never at a real user's History. It adds to whatever is there; Echo applies the
//! History limit the next time it starts.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use echo_lib::history::{EntryLanguage, HistoryStore, NewEntry};

const SAMPLES: &[(&str, &str, Option<&str>, u64)] = &[
    (
        "Wrzuciłem poprawkę do Echo na GitHuba, a Claude Code przetestował ją w Tauri.",
        "whisper-large-v3-turbo",
        Some("pl"),
        3 * 24 * 3600,
    ),
    (
        "This is a quick dictation test in English.",
        "parakeet-tdt-0.6b-v3",
        None,
        26 * 3600,
    ),
    (
        "Czy jutro o piętnastej możemy się spotkać? Jeśli nie, napisz mi wiadomość.",
        "whisper-large-v3-turbo",
        Some("pl"),
        5 * 3600,
    ),
    (
        "Notatki ze spotkania. Po pierwsze, wydanie 0.1.0 obejmuje skrót nagrywania w dwóch \
         trybach, skrót anulowania, słownik, wybór języka dyktowania i mikrofonu, historię, \
         modele z pobieraniem, ikonę w zasobniku, wskaźnik nagrywania, autostart i aktualizacje. \
         Po drugie, każda funkcja ma swoją specyfikację i testy akceptacyjne, a zmiany interfejsu \
         trafiają do przeglądu ze zrzutami ekranu w obu językach i obu motywach. Po trzecie, \
         przed wydaniem robimy test z prawdziwym mikrofonem, bo testy z plikami WAV nie wyłapią \
         wszystkiego, na przykład cichego wejścia albo urządzenia, które znika w trakcie nagrania.",
        "whisper-large-v3-turbo",
        Some("pl"),
        40 * 60,
    ),
    (
        "Zamówienie numer 4521 kosztuje 129 złotych i 99 groszy.",
        "whisper-small",
        Some("pl"),
        2 * 60,
    ),
];

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: seed_history <path to history.db>");
    let mut store = HistoryStore::open(path.as_ref()).expect("open the History database");
    let now = SystemTime::now();
    for (text, model, language, age_secs) in SAMPLES {
        let created = now - Duration::from_secs(*age_secs);
        let created_at = created.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        let language = match language {
            Some(code) => EntryLanguage::Specific {
                code: (*code).into(),
            },
            None => EntryLanguage::Automatic {
                detected: Some("en".into()),
            },
        };
        let entry = NewEntry {
            text: (*text).into(),
            model: (*model).into(),
            language,
        };
        store.add(&entry, created_at, 100).expect("add an entry");
    }
    println!("added {} sample entries to {path}", SAMPLES.len());
}
