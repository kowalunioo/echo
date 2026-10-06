//! Loading, salvaging, saving and broadcasting [`Settings`].
//!
//! The file is a JSON object: `{"version": 1, "uiLanguage": "pl", ...}`, one key per setting.
//! Keys this version does not know (written by a newer Echo) are kept and written back
//! untouched, so moving between versions never loses settings.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError, RwLock};

use serde_json::{Map, Value};

use super::{Settings, SettingsPatch};
use crate::data_dir::write_atomic;

/// The version written into the settings file. Bump it only together with a migration step in
/// [`migrate`]; adding a setting does not need a new version (missing keys get defaults).
pub const SETTINGS_FORMAT_VERSION: u64 = 1;

const VERSION_KEY: &str = "version";

/// What loading found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome {
    /// There was no settings file; defaults are used (first run).
    Fresh,
    /// Every stored setting was valid and present.
    Clean,
    /// The file was readable, but some settings were invalid (reset to defaults) or missing
    /// (filled with defaults); the file was rewritten (rules 13–14).
    Repaired {
        reset: Vec<String>,
        added: Vec<String>,
    },
    /// The file was completely unreadable; it was renamed with a `.broken` suffix and defaults
    /// are used (rule 13).
    Broken { renamed_to: Option<PathBuf> },
}

/// Why a change to the settings was refused or not saved.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("unknown setting `{0}`")]
    UnknownSetting(String),
    #[error("settings could not be saved: {0}")]
    Save(#[from] io::Error),
}

/// Called with the old and the new settings after every change.
type Listener = Box<dyn Fn(&Settings, &Settings) + Send + Sync>;

struct State {
    settings: Settings,
    /// Keys from the file that this version does not know.
    unknown: Map<String, Value>,
}

/// The single owner of the settings at run time. Every change is saved immediately with an
/// atomic write (rule 11) and then reported to the subscribers.
pub struct SettingsStore {
    path: PathBuf,
    defaults: Settings,
    state: Mutex<State>,
    listeners: RwLock<Vec<Listener>>,
}

impl SettingsStore {
    /// Loads the settings from `path`, salvaging what it can, and rewrites the file when it was
    /// missing, repaired or broken.
    pub fn open(path: impl Into<PathBuf>, defaults: Settings) -> (Self, LoadOutcome) {
        let path = path.into();
        let (settings, unknown, outcome) = load(&path, &defaults);
        let store = Self {
            path,
            defaults,
            state: Mutex::new(State { settings, unknown }),
            listeners: RwLock::new(Vec::new()),
        };
        if outcome != LoadOutcome::Clean {
            let state = store.lock();
            if let Err(error) = store.save(&state) {
                log::error!("could not write settings file: {error}");
            }
        }
        (store, outcome)
    }

    /// The settings file this store reads and writes.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A snapshot of the current settings.
    pub fn get(&self) -> Settings {
        self.lock().settings.clone()
    }

    /// Changes settings from Rust: `store.update(|s| s.ui_language = UiLanguage::Pl)`.
    pub fn update(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings, SettingsError> {
        self.modify(|current| {
            let mut next = current.clone();
            change(&mut next);
            Ok(next)
        })
    }

    /// Applies a partial change such as `{"uiLanguage": "en"}` (what the frontend sends). Its
    /// values were already validated when the patch was deserialised.
    pub fn apply_patch(&self, patch: SettingsPatch) -> Result<Settings, SettingsError> {
        self.update(|settings| patch.apply_to(settings))
    }

    /// Puts the setting named `key` (as in the file, e.g. `"uiLanguage"`) back to its default
    /// (rule 15).
    pub fn reset(&self, key: &str) -> Result<Settings, SettingsError> {
        let default = to_object(&self.defaults)
            .remove(key)
            .ok_or_else(|| SettingsError::UnknownSetting(key.to_owned()))?;
        let patch =
            serde_json::from_value(Value::Object(Map::from_iter([(key.to_owned(), default)])))
                .expect("a default is a valid patch");
        self.apply_patch(patch)
    }

    /// Registers `listener` to run after every change, with the old and the new settings. Use it
    /// for side effects such as relabelling the tray menu when the UI Language changes.
    pub fn subscribe(&self, listener: impl Fn(&Settings, &Settings) + Send + Sync + 'static) {
        self.listeners
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Box::new(listener));
    }

    /// Computes the next settings from the current ones and saves them, all under one lock so
    /// concurrent changes never overwrite each other.
    fn modify(
        &self,
        change: impl FnOnce(&Settings) -> Result<Settings, SettingsError>,
    ) -> Result<Settings, SettingsError> {
        let mut state = self.lock();
        let next = change(&state.settings)?;
        if state.settings == next {
            return Ok(next);
        }
        let previous = std::mem::replace(&mut state.settings, next.clone());
        // The new value applies even if the disk write fails, so the app keeps working; the
        // caller learns about the failure.
        let saved = self.save(&state);
        if let Err(error) = &saved {
            log::error!("could not save settings: {error}");
        }
        // Subscribers run after the lock is released, so they may read the store.
        drop(state);
        self.notify(&previous, &next);
        saved?;
        Ok(next)
    }

    fn notify(&self, previous: &Settings, next: &Settings) {
        for listener in self
            .listeners
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            listener(previous, next);
        }
    }

    fn save(&self, state: &State) -> io::Result<()> {
        let mut file = state.unknown.clone();
        file.extend(to_object(&state.settings));
        file.insert(VERSION_KEY.to_owned(), SETTINGS_FORMAT_VERSION.into());
        let mut json = serde_json::to_vec_pretty(&Value::Object(file)).map_err(io::Error::other)?;
        json.push(b'\n');
        write_atomic(&self.path, &json)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn load(path: &Path, defaults: &Settings) -> (Settings, Map<String, Value>, LoadOutcome) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (defaults.clone(), Map::new(), LoadOutcome::Fresh);
        }
        Err(error) => {
            log::warn!("settings file is unreadable ({error}); using defaults");
            return broken(path, defaults);
        }
    };
    let Ok(Value::Object(file)) = serde_json::from_slice::<Value>(&bytes) else {
        log::warn!("settings file is not a JSON object; using defaults");
        return broken(path, defaults);
    };
    let version = file.get(VERSION_KEY).and_then(Value::as_u64);
    let file = migrate(file, version.unwrap_or(0));
    let (settings, unknown, reset, added) = salvage(file, defaults);
    let outcome = if reset.is_empty() && added.is_empty() && version.is_some() {
        LoadOutcome::Clean
    } else {
        if !reset.is_empty() {
            log::warn!(
                "invalid settings reset to their defaults: {}",
                reset.join(", ")
            );
        }
        LoadOutcome::Repaired { reset, added }
    };
    (settings, unknown, outcome)
}

fn broken(path: &Path, defaults: &Settings) -> (Settings, Map<String, Value>, LoadOutcome) {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".broken");
    let target = path.with_file_name(name);
    let renamed_to = match fs::rename(path, &target) {
        Ok(()) => Some(target),
        Err(error) => {
            log::error!("could not rename the broken settings file: {error}");
            None
        }
    };
    (
        defaults.clone(),
        Map::new(),
        LoadOutcome::Broken { renamed_to },
    )
}

/// Upgrades a settings file written in an older format `version`. There are no older formats
/// yet; when [`SETTINGS_FORMAT_VERSION`] is bumped, the conversion goes here.
fn migrate(file: Map<String, Value>, _version: u64) -> Map<String, Value> {
    file
}

/// Keeps every individually valid setting from `file` and the default for every other one.
/// Returns the settings, the unknown keys, the keys reset for being invalid, and the keys that
/// were missing.
fn salvage(
    file: Map<String, Value>,
    defaults: &Settings,
) -> (Settings, Map<String, Value>, Vec<String>, Vec<String>) {
    let mut accepted = to_object(defaults);
    let known: BTreeSet<String> = accepted.keys().cloned().collect();
    let added: Vec<String> = known
        .iter()
        .filter(|key| !file.contains_key(*key))
        .cloned()
        .collect();
    let mut unknown = Map::new();
    let mut reset = Vec::new();
    for (key, value) in file {
        if key == VERSION_KEY {
            continue;
        }
        if !known.contains(&key) {
            unknown.insert(key, value);
            continue;
        }
        // Each value is checked against otherwise-default settings, so one bad setting can
        // never make another one look invalid.
        if check_one(defaults, &key, &value).is_ok() {
            accepted.insert(key, value);
        } else {
            reset.push(key);
        }
    }
    let settings = serde_json::from_value(Value::Object(accepted))
        .expect("individually valid settings combine into valid settings");
    (settings, unknown, reset, added)
}

/// Checks that `value` is valid for the setting `key` on its own, against the defaults.
fn check_one(defaults: &Settings, key: &str, value: &Value) -> Result<(), String> {
    let mut candidate = to_object(defaults);
    candidate.insert(key.to_owned(), value.clone());
    serde_json::from_value::<Settings>(Value::Object(candidate))
        .map(drop)
        .map_err(|error| error.to_string())
}

fn to_object(settings: &Settings) -> Map<String, Value> {
    match serde_json::to_value(settings).expect("settings always serialise") {
        Value::Object(map) => map,
        _ => unreachable!("Settings is a struct"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;

    use super::*;
    use crate::settings::UiLanguage;

    fn defaults() -> Settings {
        Settings::defaults(Some("en-US"))
    }

    fn settings_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        (dir, path)
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn a_missing_file_gives_defaults_and_is_written() {
        let (_dir, path) = settings_path();

        let (store, outcome) = SettingsStore::open(&path, defaults());

        assert_eq!(outcome, LoadOutcome::Fresh);
        assert_eq!(store.get(), defaults());
        assert_eq!(read_json(&path)["uiLanguage"], "en");
        assert_eq!(read_json(&path)["version"], SETTINGS_FORMAT_VERSION);
    }

    #[test]
    fn changes_are_saved_immediately_and_survive_a_restart() {
        let (_dir, path) = settings_path();
        let (store, _) = SettingsStore::open(&path, defaults());

        store.update(|s| s.ui_language = UiLanguage::Pl).unwrap();
        drop(store);

        let (reopened, outcome) = SettingsStore::open(&path, defaults());
        assert_eq!(outcome, LoadOutcome::Clean);
        assert_eq!(reopened.get().ui_language, UiLanguage::Pl);
    }

    // settings-and-first-run.md acceptance test 9.
    #[test]
    fn one_invalid_value_is_reset_and_the_others_are_kept() {
        let (_dir, path) = settings_path();
        fs::write(
            &path,
            json!({
                "version": 1,
                "uiLanguage": "klingon",
                "onboardingWelcomeDone": true,
                "onboardingCompleted": true,
                "historyLimit": 5,
            })
            .to_string(),
        )
        .unwrap();

        let (store, outcome) = SettingsStore::open(&path, defaults());

        assert_eq!(
            outcome,
            LoadOutcome::Repaired {
                reset: vec!["uiLanguage".into()],
                added: vec![]
            }
        );
        let settings = store.get();
        assert_eq!(settings.ui_language, UiLanguage::En);
        assert!(settings.onboarding_completed);
        assert_eq!(read_json(&path)["uiLanguage"], "en", "file is rewritten");
    }

    #[test]
    fn an_out_of_range_history_limit_is_reset_to_its_default() {
        let (_dir, path) = settings_path();
        fs::write(
            &path,
            json!({
                "version": 1,
                "uiLanguage": "pl",
                "onboardingWelcomeDone": true,
                "onboardingCompleted": true,
                "historyLimit": 101,
            })
            .to_string(),
        )
        .unwrap();

        let (store, outcome) = SettingsStore::open(&path, defaults());

        assert_eq!(
            outcome,
            LoadOutcome::Repaired {
                reset: vec!["historyLimit".into()],
                added: vec![]
            }
        );
        assert_eq!(store.get().history_limit.get(), 5);
        assert_eq!(store.get().ui_language, UiLanguage::Pl);
    }

    // settings-and-first-run.md acceptance test 10.
    #[test]
    fn a_garbage_file_is_renamed_broken_and_defaults_are_used() {
        let (dir, path) = settings_path();
        fs::write(&path, b"\x00\xffnot json at all{{").unwrap();

        let (store, outcome) = SettingsStore::open(&path, defaults());

        let broken = dir.path().join("settings.json.broken");
        assert_eq!(
            outcome,
            LoadOutcome::Broken {
                renamed_to: Some(broken.clone())
            }
        );
        assert_eq!(store.get(), defaults());
        assert_eq!(fs::read(&broken).unwrap(), b"\x00\xffnot json at all{{");
        assert_eq!(read_json(&path)["uiLanguage"], "en");
    }

    #[test]
    fn a_json_value_that_is_not_an_object_counts_as_unreadable() {
        let (dir, path) = settings_path();
        fs::write(&path, b"[1, 2, 3]").unwrap();

        let (_store, outcome) = SettingsStore::open(&path, defaults());

        assert!(matches!(outcome, LoadOutcome::Broken { .. }));
        assert!(dir.path().join("settings.json.broken").exists());
    }

    // Rule 14: settings added in a newer version get their defaults.
    #[test]
    fn settings_missing_from_an_older_file_get_their_defaults() {
        let (_dir, path) = settings_path();
        fs::write(
            &path,
            json!({ "version": 1, "uiLanguage": "pl" }).to_string(),
        )
        .unwrap();

        let (store, outcome) = SettingsStore::open(&path, defaults());

        assert_eq!(
            outcome,
            LoadOutcome::Repaired {
                reset: vec![],
                added: vec![
                    "historyLimit".into(),
                    "onboardingCompleted".into(),
                    "onboardingWelcomeDone".into()
                ]
            }
        );
        assert_eq!(store.get().ui_language, UiLanguage::Pl);
        assert!(!store.get().onboarding_completed);
        assert_eq!(read_json(&path)["onboardingCompleted"], false);
    }

    #[test]
    fn settings_from_a_newer_version_are_kept_in_the_file() {
        let (_dir, path) = settings_path();
        fs::write(
            &path,
            json!({ "version": 1, "uiLanguage": "pl", "futureSetting": [1, 2] }).to_string(),
        )
        .unwrap();
        let (store, _) = SettingsStore::open(&path, defaults());

        store.update(|s| s.ui_language = UiLanguage::En).unwrap();

        assert_eq!(read_json(&path)["futureSetting"], json!([1, 2]));
        assert_eq!(read_json(&path)["uiLanguage"], "en");
    }

    #[test]
    fn a_patch_changes_only_the_named_settings() {
        let (_dir, path) = settings_path();
        let (store, _) = SettingsStore::open(&path, defaults());
        store.update(|s| s.onboarding_welcome_done = true).unwrap();

        let patch: SettingsPatch = serde_json::from_value(json!({ "uiLanguage": "pl" })).unwrap();
        let settings = store.apply_patch(patch).unwrap();

        assert_eq!(settings.ui_language, UiLanguage::Pl);
        assert!(settings.onboarding_welcome_done);
        assert_eq!(read_json(&path)["uiLanguage"], "pl");
    }

    #[test]
    fn a_patch_with_an_invalid_value_or_unknown_key_is_rejected() {
        let invalid = serde_json::from_value::<SettingsPatch>(json!({ "uiLanguage": "de" }));
        let unknown =
            serde_json::from_value::<SettingsPatch>(json!({ "uiLanguage": "pl", "nope": 1 }));

        assert!(invalid.is_err());
        assert!(unknown.is_err());
    }

    #[test]
    fn reset_puts_one_setting_back_to_its_default() {
        let (_dir, path) = settings_path();
        let (store, _) = SettingsStore::open(&path, Settings::defaults(Some("pl-PL")));
        store
            .update(|s| {
                s.ui_language = UiLanguage::En;
                s.onboarding_completed = true;
            })
            .unwrap();

        let settings = store.reset("uiLanguage").unwrap();

        assert_eq!(settings.ui_language, UiLanguage::Pl);
        assert!(settings.onboarding_completed);
        assert!(matches!(
            store.reset("nope"),
            Err(SettingsError::UnknownSetting(_))
        ));
    }

    #[test]
    fn subscribers_hear_about_real_changes_only() {
        let (_dir, path) = settings_path();
        let (store, _) = SettingsStore::open(&path, defaults());
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&calls);
        store.subscribe(move |old, new| {
            assert_eq!(old.ui_language, UiLanguage::En);
            assert_eq!(new.ui_language, UiLanguage::Pl);
            seen.fetch_add(1, Ordering::SeqCst);
        });

        store.update(|s| s.ui_language = UiLanguage::Pl).unwrap();
        store.update(|s| s.ui_language = UiLanguage::Pl).unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_file_without_a_version_is_salvaged_and_rewritten_with_one() {
        let (_dir, path) = settings_path();
        fs::write(
            &path,
            json!({ "uiLanguage": "pl", "onboardingWelcomeDone": false, "onboardingCompleted": false })
                .to_string(),
        )
        .unwrap();

        let (store, outcome) = SettingsStore::open(&path, defaults());

        assert!(matches!(outcome, LoadOutcome::Repaired { .. }));
        assert_eq!(store.get().ui_language, UiLanguage::Pl);
        assert_eq!(read_json(&path)["version"], SETTINGS_FORMAT_VERSION);
    }
}
