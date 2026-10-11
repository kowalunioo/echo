//! The Record Shortcut inside the Tauri app: start-up wiring to the settings, the commands the
//! Dictation page uses, and the events it listens to.
//!
//! The combination and mode are settings (`recordShortcut`, `shortcutMode`). The mode, "Restore
//! default" and changes made anywhere else go through the generic settings commands and reach the
//! Record Shortcut through [`SettingsStore::subscribe`]. A new combination from the capture UI
//! goes through [`set_record_shortcut`] instead, because it must be activated before it is saved
//! and the user must see why when it cannot be (rule 23).

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use super::modes::RecordIntent;
use super::record::{
    RecordShortcut, RecordShortcutConfig, RecordShortcutHandle, ShortcutChangeError, spawn,
};
use super::validation::{
    CancelShortcutCombination, RecordShortcutCombination, VocabularyShortcutCombination,
};
use super::{KeyAction, KeyCombination, ShortcutListener};
use crate::dictation::Dictation;
use crate::settings::{Settings, SettingsStore};

/// A key pressed or released during shortcut capture, by its capture name (`"LeftCtrl"`,
/// `"RightAlt"`, `"Space"`, `"F9"`).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct CapturedKeyEvent {
    pub key: String,
    pub pressed: bool,
}

fn config_of(settings: &Settings) -> RecordShortcutConfig {
    RecordShortcutConfig {
        combination: settings.record_shortcut.combination(),
        mode: settings.shortcut_mode,
        cancel: settings.cancel_shortcut.combination(),
        vocabulary: settings.vocabulary_shortcut.combination(),
        vocabulary_enabled: settings.vocabulary_shortcut_enabled,
    }
}

/// A shortcut setting put back to the value in force because the new one could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PutBack {
    Record(RecordShortcutCombination),
    Cancel(CancelShortcutCombination),
    Vocabulary(VocabularyShortcutCombination),
    VocabularyEnabled(bool),
}

impl PutBack {
    fn apply(self, settings: &mut Settings) {
        match self {
            Self::Record(value) => settings.record_shortcut = value,
            Self::Cancel(value) => settings.cancel_shortcut = value,
            Self::Vocabulary(value) => settings.vocabulary_shortcut = value,
            Self::VocabularyEnabled(value) => settings.vocabulary_shortcut_enabled = value,
        }
    }
}

/// Combinations the Vocabulary Shortcut falls back to when stored settings give it one another
/// shortcut holds; with two other shortcuts, one of the three is always free.
const VOCABULARY_FALLBACKS: [&str; 3] = ["Ctrl+Alt+D", "Ctrl+Alt+Shift+D", "Ctrl+Alt+Shift+Win+D"];

/// Repairs stored settings in which two shortcuts share a combination. That happens by editing
/// the file, or when a Record or Cancel Shortcut chosen before the Vocabulary Shortcut existed is
/// its default, Ctrl+Alt+D. The Cancel Shortcut goes back to Escape, which the Record Shortcut can
/// never be (`record-shortcut.md` rule 20); the Vocabulary Shortcut moves to the first free
/// combination of [`VOCABULARY_FALLBACKS`].
fn without_conflict(settings: &Settings) -> Vec<PutBack> {
    let mut repairs = Vec::new();
    let record = settings.record_shortcut.combination();
    let mut cancel = settings.cancel_shortcut.combination();
    if cancel == record {
        repairs.push(PutBack::Cancel(CancelShortcutCombination::default()));
        cancel = CancelShortcutCombination::default().combination();
    }
    let vocabulary = settings.vocabulary_shortcut.combination();
    if vocabulary == record || vocabulary == cancel {
        let free = VOCABULARY_FALLBACKS
            .iter()
            .map(|text| text.parse::<KeyCombination>().expect("a valid fallback"))
            .find(|c| *c != record && *c != cancel)
            .and_then(|c| VocabularyShortcutCombination::try_from(c).ok());
        repairs.extend(free.map(PutBack::Vocabulary));
    }
    repairs
}

/// Starts the Record Shortcut from the settings (the [`SettingsStore`] must already be managed):
/// installs the keyboard hook, binds the combination once first-run setup is finished (rule 17),
/// follows settings changes, and makes the [`RecordShortcutHandle`] available to commands and the
/// pipeline.
pub fn install(app: &AppHandle) {
    #[cfg(windows)]
    let listener: Box<dyn ShortcutListener> = Box::new(super::WindowsShortcutListener::new());
    #[cfg(not(windows))]
    let listener: Box<dyn ShortcutListener> = Box::new(super::FakeShortcutListener::new());

    let store = app.state::<SettingsStore>();
    for repaired in without_conflict(&store.get()) {
        log::warn!("two shortcuts shared a combination; repairing: {repaired:?}");
        if let Err(error) = store.update(|s| repaired.apply(s)) {
            log::warn!("cannot save the repaired shortcut: {error}");
        }
    }
    let settings = store.get();
    let core = RecordShortcut::new(listener, config_of(&settings), shortcut_wanted(&settings));
    let intents_app = app.clone();
    let cancel_app = app.clone();
    let keys_app = app.clone();
    let vocabulary_app = app.clone();
    let (handle, started) = spawn(
        core,
        // The dictation pipeline is installed before the Record Shortcut.
        Box::new(move |intent| {
            // An update is installing and Echo is about to restart (updater.md rule 5).
            if matches!(intent, RecordIntent::Start)
                && crate::updater::app::installing(&intents_app)
            {
                log::info!("Not starting a Dictation: an update is installing");
                return;
            }
            if let Some(dictation) = intents_app.try_state::<Dictation>() {
                dictation.intent(intent);
            }
        }),
        Box::new(move |key| {
            let _ = CapturedKeyEvent {
                key: key.key,
                pressed: key.action == KeyAction::Pressed,
            }
            .emit(&keys_app);
        }),
        // The same Cancellation as the Overlay's cancel button (cancel-shortcut.md rule 13).
        Box::new(move || {
            if let Some(dictation) = cancel_app.try_state::<Dictation>() {
                dictation.cancel();
            }
        }),
        Box::new(move |attempt| crate::vocabulary_shortcut::app::run(&vocabulary_app, attempt)),
    );
    if let Err(error) = started {
        log::error!("the Record Shortcut is unavailable: {error}");
    }

    let follower = handle.clone();
    let settings_app = app.clone();
    store.subscribe(move |old, new| {
        for value in follow_settings(&follower, old, new) {
            let store = settings_app.state::<SettingsStore>();
            if let Err(error) = store.update(|s| value.apply(s)) {
                log::warn!("cannot put a shortcut setting back: {error}");
            }
        }
    });
    app.manage(handle);
}

/// Whether the Record Shortcut should be active: once first-run setup is finished, or as soon as
/// a Model is active during it, so the user can dictate on the onboarding's Try it step
/// (`record-shortcut.md` rule 17, `settings-and-first-run.md` rule 3).
fn shortcut_wanted(settings: &Settings) -> bool {
    settings.onboarding_completed || settings.active_model.is_some()
}

/// Applies a settings change to the running Record Shortcut and Cancel Shortcut. A combination
/// that cannot be used (it cannot be activated, or it is the other shortcut) is returned to be
/// put back in the settings, so the settings always show what is in force.
fn follow_settings(
    shortcut: &RecordShortcutHandle,
    old: &Settings,
    new: &Settings,
) -> Vec<PutBack> {
    let mut put_back = Vec::new();
    if old.shortcut_mode != new.shortcut_mode {
        shortcut.set_mode(new.shortcut_mode);
    }
    if old.record_shortcut != new.record_shortcut
        && let Err(error) = shortcut.apply_combination(&new.record_shortcut.combination())
    {
        log::warn!("cannot activate {:?}: {error}", new.record_shortcut);
        if let Ok(in_force) = shortcut.config().combination.try_into() {
            put_back.push(PutBack::Record(in_force));
        }
    }
    if old.cancel_shortcut != new.cancel_shortcut
        && let Err(problem) = shortcut.apply_cancel_combination(&new.cancel_shortcut.combination())
    {
        log::warn!("cannot use {:?}: {problem}", new.cancel_shortcut);
        if let Ok(in_force) = shortcut.config().cancel.try_into() {
            put_back.push(PutBack::Cancel(in_force));
        }
    }
    if old.vocabulary_shortcut != new.vocabulary_shortcut
        && let Err(error) =
            shortcut.apply_vocabulary_combination(&new.vocabulary_shortcut.combination())
    {
        log::warn!("cannot activate {:?}: {error}", new.vocabulary_shortcut);
        if let Ok(in_force) = shortcut.config().vocabulary.try_into() {
            put_back.push(PutBack::Vocabulary(in_force));
        }
    }
    if old.vocabulary_shortcut_enabled != new.vocabulary_shortcut_enabled
        && let Err(error) = shortcut.set_vocabulary_enabled(new.vocabulary_shortcut_enabled)
    {
        log::warn!("cannot turn the Vocabulary Shortcut on or off: {error}");
        put_back.push(PutBack::VocabularyEnabled(
            shortcut.config().vocabulary_enabled,
        ));
    }
    if shortcut_wanted(old) != shortcut_wanted(new)
        && let Err(error) = shortcut.set_active(shortcut_wanted(new))
    {
        log::warn!("cannot (de)activate the Record Shortcut: {error}");
    }
    put_back
}

/// Validates and activates a new Record Shortcut given in canonical text form (`"Ctrl+Space"`),
/// then saves it. On failure the previous one stays active and saved. Ends a capture in progress.
/// Returns the new settings (every window also receives them as a `SettingsChanged` event).
#[tauri::command]
#[specta::specta]
pub fn set_record_shortcut(
    shortcut: State<'_, RecordShortcutHandle>,
    store: State<'_, SettingsStore>,
    combination: String,
) -> Result<Settings, ShortcutChangeError> {
    let accepted = shortcut.set_combination(&combination)?;
    store.update(|s| s.record_shortcut = accepted).map_err(|e| {
        ShortcutChangeError::ActivationFailed {
            reason: e.to_string(),
        }
    })
}

/// Validates a new Cancel Shortcut given in canonical text form (`"Escape"`, `"Ctrl+Q"`) and
/// saves it; it applies from the next Dictation (`cancel-shortcut.md` rules 11–12). On failure
/// the previous one stays. Ends a capture in progress. Returns the new settings.
#[tauri::command]
#[specta::specta]
pub fn set_cancel_shortcut(
    shortcut: State<'_, RecordShortcutHandle>,
    store: State<'_, SettingsStore>,
    combination: String,
) -> Result<Settings, ShortcutChangeError> {
    let accepted = shortcut.set_cancel_combination(&combination)?;
    store.update(|s| s.cancel_shortcut = accepted).map_err(|e| {
        ShortcutChangeError::ActivationFailed {
            reason: e.to_string(),
        }
    })
}

/// Validates and activates a new Vocabulary Shortcut given in canonical text form
/// (`"Ctrl+Alt+D"`), then saves it (`vocabulary-shortcut.md` rules 18–21). A turned-off
/// Vocabulary Shortcut takes the new combination without binding it. On failure the previous one
/// stays active and saved, and the error names the shortcut that holds the combination
/// (`sameAsRecord`, `sameAsCancel`). Ends a capture in progress. Returns the new settings.
#[tauri::command]
#[specta::specta]
pub fn set_vocabulary_shortcut(
    shortcut: State<'_, RecordShortcutHandle>,
    store: State<'_, SettingsStore>,
    combination: String,
) -> Result<Settings, ShortcutChangeError> {
    let accepted = shortcut.set_vocabulary_combination(&combination)?;
    store
        .update(|s| s.vocabulary_shortcut = accepted)
        .map_err(|e| ShortcutChangeError::ActivationFailed {
            reason: e.to_string(),
        })
}

/// Starts shortcut capture: the Record Shortcut is suspended and keys arrive as
/// [`CapturedKeyEvent`]s instead of reaching applications.
#[tauri::command]
#[specta::specta]
pub fn begin_shortcut_capture(shortcut: State<'_, RecordShortcutHandle>) -> Result<(), String> {
    shortcut.begin_capture().map_err(|e| e.to_string())
}

/// A key pressed or released in Echo's own window, by capture name. Windows does not run Echo's
/// keyboard hook while Echo's window has focus, so the window forwards its keys here.
#[tauri::command]
#[specta::specta]
pub fn own_window_key(shortcut: State<'_, RecordShortcutHandle>, key: String, pressed: bool) {
    let action = if pressed {
        KeyAction::Pressed
    } else {
        KeyAction::Released
    };
    shortcut.own_window_key(&key, action);
}

/// Ends shortcut capture without changes.
#[tauri::command]
#[specta::specta]
pub fn end_shortcut_capture(shortcut: State<'_, RecordShortcutHandle>) {
    shortcut.end_capture();
}

#[cfg(test)]
mod tests {
    use super::super::modes::ShortcutMode;
    use super::super::{FakeShortcutListener, Shortcut};
    use super::*;
    use crate::models::ModelId;

    fn running_with(
        config: RecordShortcutConfig,
        active: bool,
    ) -> (FakeShortcutListener, RecordShortcutHandle) {
        let fake = FakeShortcutListener::new();
        let core = RecordShortcut::new(Box::new(fake.clone()), config, active);
        let (handle, started) = spawn(
            core,
            Box::new(|_| {}),
            Box::new(|_| {}),
            Box::new(|| {}),
            Box::new(drop),
        );
        started.unwrap();
        (fake, handle)
    }

    fn running(active: bool) -> (FakeShortcutListener, RecordShortcutHandle) {
        running_with(RecordShortcutConfig::default(), active)
    }

    fn bound(fake: &FakeShortcutListener) -> Option<String> {
        fake.binding(Shortcut::Record).map(|c| c.to_string())
    }

    fn finished_setup() -> Settings {
        Settings {
            onboarding_completed: true,
            ..Settings::defaults(None)
        }
    }

    fn with_shortcut(settings: &Settings, text: &str) -> Settings {
        Settings {
            record_shortcut: text.to_owned().try_into().unwrap(),
            ..settings.clone()
        }
    }

    fn with_cancel(settings: &Settings, text: &str) -> Settings {
        Settings {
            cancel_shortcut: text.to_owned().try_into().unwrap(),
            ..settings.clone()
        }
    }

    // Rule 24 via the settings: "Restore default" (or any other change) is rebound at once.
    #[test]
    fn a_combination_changed_in_the_settings_is_rebound() {
        let (fake, handle) = running(true);
        let old = finished_setup();

        let put_back = follow_settings(&handle, &old, &with_shortcut(&old, "F9"));

        assert_eq!(put_back, vec![]);
        assert_eq!(bound(&fake).as_deref(), Some("F9"));
    }

    #[test]
    fn a_combination_that_cannot_be_activated_is_put_back_in_the_settings() {
        let (fake, handle) = running(true);
        fake.reject_binds(Some("in use"));
        let old = finished_setup();

        let put_back = follow_settings(&handle, &old, &with_shortcut(&old, "F9"));

        assert_eq!(
            put_back,
            vec![PutBack::Record(RecordShortcutCombination::default())]
        );
        assert_eq!(bound(&fake).as_deref(), Some("Ctrl+Space"));
    }

    #[test]
    fn the_mode_follows_the_settings() {
        let (_fake, handle) = running(true);
        let old = finished_setup();
        let new = Settings {
            shortcut_mode: ShortcutMode::Toggle,
            ..old.clone()
        };

        follow_settings(&handle, &old, &new);

        assert_eq!(handle.config().mode, ShortcutMode::Toggle);
    }

    // record-shortcut.md rule 17.
    #[test]
    fn finishing_first_run_setup_activates_the_shortcut() {
        let (fake, handle) = running(false);
        assert_eq!(bound(&fake), None);

        follow_settings(&handle, &Settings::defaults(None), &finished_setup());

        assert_eq!(bound(&fake).as_deref(), Some("Ctrl+Space"));
    }

    // settings-and-first-run.md rule 3: a Model made active during onboarding activates the
    // shortcut before "Finish", so the user can dictate on the Try it step.
    #[test]
    fn a_model_made_active_during_onboarding_activates_the_shortcut() {
        let (fake, handle) = running(false);
        let onboarding = Settings::defaults(None);
        let with_model = Settings {
            active_model: Some(ModelId::WhisperSmall),
            ..onboarding.clone()
        };

        follow_settings(&handle, &onboarding, &with_model);

        assert_eq!(bound(&fake).as_deref(), Some("Ctrl+Space"));
    }

    #[test]
    fn the_shortcut_is_active_at_start_when_a_model_is_active_but_onboarding_is_not_finished() {
        let settings = Settings {
            active_model: Some(ModelId::WhisperSmall),
            ..Settings::defaults(None)
        };

        assert!(shortcut_wanted(&settings));
        assert!(!shortcut_wanted(&Settings::defaults(None)));
    }

    // cancel-shortcut.md rule 12 via the settings ("Reset to default" and the like).
    #[test]
    fn a_cancel_shortcut_changed_in_the_settings_is_used_for_the_next_dictation() {
        let (_fake, handle) = running(true);
        let old = finished_setup();

        let put_back = follow_settings(&handle, &old, &with_cancel(&old, "Ctrl+Q"));

        assert_eq!(put_back, vec![]);
        assert_eq!(handle.config().cancel.to_string(), "Ctrl+Q");
    }

    // cancel-shortcut.md rule 11: a Cancel Shortcut equal to the Record Shortcut is put back.
    #[test]
    fn a_cancel_shortcut_equal_to_the_record_shortcut_is_put_back() {
        let (_fake, handle) = running(true);
        let old = finished_setup();

        let put_back = follow_settings(&handle, &old, &with_cancel(&old, "Ctrl+Space"));

        assert_eq!(
            put_back,
            vec![PutBack::Cancel(CancelShortcutCombination::default())]
        );
        assert_eq!(handle.config().cancel.to_string(), "Escape");
    }

    // record-shortcut.md rule 24: "Reset to default" is subject to the same validation, so a
    // reset to Ctrl+Space while that is the Cancel Shortcut keeps the current one.
    #[test]
    fn a_record_shortcut_reset_onto_the_cancel_shortcut_is_put_back() {
        let config = RecordShortcutConfig {
            combination: "F9".parse().unwrap(),
            cancel: "Ctrl+Space".parse().unwrap(),
            ..RecordShortcutConfig::default()
        };
        let (fake, handle) = running_with(config, true);
        let old = with_cancel(&with_shortcut(&finished_setup(), "F9"), "Ctrl+Space");

        let put_back = follow_settings(&handle, &old, &with_shortcut(&old, "Ctrl+Space"));

        assert_eq!(
            put_back,
            vec![PutBack::Record("F9".to_owned().try_into().unwrap())]
        );
        assert_eq!(bound(&fake).as_deref(), Some("F9"));
    }

    #[test]
    fn stored_settings_with_both_shortcuts_equal_get_the_default_cancel_shortcut() {
        let settings = with_cancel(&with_shortcut(&finished_setup(), "F9"), "F9");
        assert_eq!(
            without_conflict(&settings),
            vec![PutBack::Cancel(CancelShortcutCombination::default())]
        );
        assert_eq!(without_conflict(&finished_setup()), vec![]);
    }

    fn with_vocabulary(settings: &Settings, text: &str) -> Settings {
        Settings {
            vocabulary_shortcut: text.to_owned().try_into().unwrap(),
            ..settings.clone()
        }
    }

    fn vocabulary(text: &str) -> PutBack {
        PutBack::Vocabulary(text.to_owned().try_into().unwrap())
    }

    // A Record or Cancel Shortcut chosen before the Vocabulary Shortcut existed may be its
    // default; the Vocabulary Shortcut moves out of the way (vocabulary-shortcut.md rule 19).
    #[test]
    fn a_stored_vocabulary_shortcut_held_by_another_shortcut_moves_to_a_free_one() {
        let record_has_it = with_shortcut(&finished_setup(), "Ctrl+Alt+D");
        assert_eq!(
            without_conflict(&record_has_it),
            vec![vocabulary("Ctrl+Alt+Shift+D")]
        );
        let both_taken = with_cancel(&record_has_it, "Ctrl+Alt+Shift+D");
        assert_eq!(
            without_conflict(&both_taken),
            vec![vocabulary("Ctrl+Alt+Shift+Win+D")]
        );
        let cancel_has_it = with_vocabulary(&with_cancel(&finished_setup(), "F8"), "F8");
        assert_eq!(
            without_conflict(&cancel_has_it),
            vec![vocabulary("Ctrl+Alt+D")]
        );
    }

    fn vocabulary_bound(fake: &FakeShortcutListener) -> Option<String> {
        fake.binding(Shortcut::Vocabulary).map(|c| c.to_string())
    }

    // vocabulary-shortcut.md rules 20-21 via the settings (the switch, "Reset to default").
    #[test]
    fn the_vocabulary_shortcut_follows_the_settings() {
        let (fake, handle) = running(true);
        let old = finished_setup();
        assert_eq!(vocabulary_bound(&fake).as_deref(), Some("Ctrl+Alt+D"));

        let changed = with_vocabulary(&old, "Ctrl+Alt+V");
        assert_eq!(follow_settings(&handle, &old, &changed), vec![]);
        assert_eq!(vocabulary_bound(&fake).as_deref(), Some("Ctrl+Alt+V"));

        let off = Settings {
            vocabulary_shortcut_enabled: false,
            ..changed.clone()
        };
        assert_eq!(follow_settings(&handle, &changed, &off), vec![]);
        assert_eq!(vocabulary_bound(&fake), None);
        assert!(!handle.config().vocabulary_enabled);
    }

    #[test]
    fn a_vocabulary_shortcut_held_by_another_shortcut_is_put_back() {
        let (fake, handle) = running(true);
        let old = finished_setup();

        let put_back = follow_settings(&handle, &old, &with_vocabulary(&old, "Ctrl+Space"));

        assert_eq!(put_back, vec![vocabulary("Ctrl+Alt+D")]);
        assert_eq!(vocabulary_bound(&fake).as_deref(), Some("Ctrl+Alt+D"));
    }

    #[test]
    fn a_switch_that_cannot_be_activated_is_put_back() {
        let (fake, handle) = running(true);
        let old = finished_setup();
        fake.reject_binds(Some("in use"));

        let off = Settings {
            vocabulary_shortcut_enabled: false,
            ..old.clone()
        };
        assert_eq!(
            follow_settings(&handle, &old, &off),
            vec![PutBack::VocabularyEnabled(true)]
        );
    }
}
