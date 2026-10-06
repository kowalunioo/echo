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
use super::validation::RecordShortcutCombination;
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
    }
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
    let settings = store.get();
    let core = RecordShortcut::new(
        listener,
        config_of(&settings),
        settings.onboarding_completed,
    );
    let intents_app = app.clone();
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
    );
    if let Err(error) = started {
        log::error!("the Record Shortcut is unavailable: {error}");
    }

    let follower = handle.clone();
    let settings_app = app.clone();
    store.subscribe(move |old, new| {
        follow_settings(&follower, old, new, |in_force| {
            let store = settings_app.state::<SettingsStore>();
            if let Err(error) = store.update(|s| s.record_shortcut = in_force) {
                log::warn!("cannot put the Record Shortcut setting back: {error}");
            }
        });
    });
    app.manage(handle);
}

/// Applies a settings change to the running Record Shortcut. A combination that cannot be
/// activated is put back in the settings, so the settings always show what is in force.
fn follow_settings(
    shortcut: &RecordShortcutHandle,
    old: &Settings,
    new: &Settings,
    put_back: impl FnOnce(RecordShortcutCombination),
) {
    if old.shortcut_mode != new.shortcut_mode {
        shortcut.set_mode(new.shortcut_mode);
    }
    if old.record_shortcut != new.record_shortcut
        && let Err(error) = shortcut.apply_combination(&new.record_shortcut.combination())
    {
        log::warn!("cannot activate {:?}: {error}", new.record_shortcut);
        if let Ok(in_force) = shortcut.config().combination.try_into() {
            put_back(in_force);
        }
    }
    if old.onboarding_completed != new.onboarding_completed
        && let Err(error) = shortcut.set_active(new.onboarding_completed)
    {
        log::warn!("cannot (de)activate the Record Shortcut: {error}");
    }
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
    use std::cell::RefCell;

    use super::super::modes::ShortcutMode;
    use super::super::{FakeShortcutListener, Shortcut};
    use super::*;

    fn running(active: bool) -> (FakeShortcutListener, RecordShortcutHandle) {
        let fake = FakeShortcutListener::new();
        let core = RecordShortcut::new(
            Box::new(fake.clone()),
            RecordShortcutConfig::default(),
            active,
        );
        let (handle, started) = spawn(core, Box::new(|_| {}), Box::new(|_| {}));
        started.unwrap();
        (fake, handle)
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

    // Rule 24 via the settings: "Restore default" (or any other change) is rebound at once.
    #[test]
    fn a_combination_changed_in_the_settings_is_rebound() {
        let (fake, handle) = running(true);
        let old = finished_setup();

        follow_settings(&handle, &old, &with_shortcut(&old, "F9"), |_| {
            panic!("nothing to put back")
        });

        assert_eq!(bound(&fake).as_deref(), Some("F9"));
    }

    #[test]
    fn a_combination_that_cannot_be_activated_is_put_back_in_the_settings() {
        let (fake, handle) = running(true);
        fake.reject_binds(Some("in use"));
        let old = finished_setup();
        let put_back = RefCell::new(None);

        follow_settings(&handle, &old, &with_shortcut(&old, "F9"), |value| {
            *put_back.borrow_mut() = Some(String::from(value));
        });

        assert_eq!(put_back.into_inner().as_deref(), Some("Ctrl+Space"));
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

        follow_settings(&handle, &old, &new, |_| {});

        assert_eq!(handle.config().mode, ShortcutMode::Toggle);
    }

    // record-shortcut.md rule 17.
    #[test]
    fn finishing_first_run_setup_activates_the_shortcut() {
        let (fake, handle) = running(false);
        assert_eq!(bound(&fake), None);

        follow_settings(
            &handle,
            &Settings::defaults(None),
            &finished_setup(),
            |_| {},
        );

        assert_eq!(bound(&fake).as_deref(), Some("Ctrl+Space"));
    }
}
