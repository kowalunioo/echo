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

use super::record::{
    RecordShortcut, RecordShortcutConfig, RecordShortcutHandle, ShortcutChangeError, spawn,
};
use super::validation::{CancelShortcutCombination, RecordShortcutCombination};
use super::{KeyAction, ShortcutListener};
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
    }
}

/// A shortcut setting put back to the value in force because the new one could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PutBack {
    Record(RecordShortcutCombination),
    Cancel(CancelShortcutCombination),
}

impl PutBack {
    fn apply(self, settings: &mut Settings) {
        match self {
            Self::Record(value) => settings.record_shortcut = value,
            Self::Cancel(value) => settings.cancel_shortcut = value,
        }
    }
}

/// Settings saved with the Cancel Shortcut equal to the Record Shortcut (only possible by editing
/// the file) are repaired by putting the Cancel Shortcut back to its default, Escape, which the
/// Record Shortcut can never be (`record-shortcut.md` rule 20).
fn without_conflict(settings: &Settings) -> Option<CancelShortcutCombination> {
    (settings.cancel_shortcut.combination() == settings.record_shortcut.combination())
        .then(CancelShortcutCombination::default)
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
    if let Some(repaired) = without_conflict(&store.get()) {
        log::warn!("the Cancel Shortcut was the Record Shortcut; it is reset to Escape");
        if let Err(error) = store.update(|s| s.cancel_shortcut = repaired) {
            log::warn!("cannot save the repaired Cancel Shortcut: {error}");
        }
    }
    let settings = store.get();
    let core = RecordShortcut::new(
        listener,
        config_of(&settings),
        settings.onboarding_completed,
    );
    let intents_app = app.clone();
    let cancel_app = app.clone();
    let keys_app = app.clone();
    let (handle, started) = spawn(
        core,
        // The dictation pipeline is installed before the Record Shortcut.
        Box::new(move |intent| {
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
    if old.onboarding_completed != new.onboarding_completed
        && let Err(error) = shortcut.set_active(new.onboarding_completed)
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

    fn running_with(
        config: RecordShortcutConfig,
        active: bool,
    ) -> (FakeShortcutListener, RecordShortcutHandle) {
        let fake = FakeShortcutListener::new();
        let core = RecordShortcut::new(Box::new(fake.clone()), config, active);
        let (handle, started) = spawn(core, Box::new(|_| {}), Box::new(|_| {}), Box::new(|| {}));
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
            Some(CancelShortcutCombination::default())
        );
        assert_eq!(without_conflict(&finished_setup()), None);
    }
}
