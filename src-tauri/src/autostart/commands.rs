//! Commands for the "Start Echo when I sign in to Windows" toggle on the App page.

use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use super::{Autostart, AutostartStatus};
use crate::settings::SettingsStore;

/// The Windows Settings page that lists Startup apps (`autostart.md` "UI").
pub const STARTUP_APPS_URI: &str = "ms-settings:startupapps";

/// The effective autostart state: whether Windows "Startup apps" has turned Echo off while the
/// setting is on (rule 9). The setting itself comes with the other settings.
#[tauri::command]
#[specta::specta]
pub fn autostart_status(
    autostart: State<'_, Autostart>,
    settings: State<'_, SettingsStore>,
) -> AutostartStatus {
    autostart.status(settings.get().start_with_windows)
}

/// Opens the Windows Startup apps settings page.
#[tauri::command]
#[specta::specta]
pub fn open_startup_apps_settings(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(STARTUP_APPS_URI, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Registers Echo's sign-in entry for this executable, re-applies the stored setting (self-heal,
/// rule 4) and follows every later change of it (rule 3).
pub fn install(app: &AppHandle) {
    let autostart = Autostart::new(entry(app), launch_command());
    let store = app.state::<SettingsStore>();
    autostart.apply_logged(store.get().start_with_windows);
    let handle = app.clone();
    store.subscribe(move |old, new| {
        if old.start_with_windows != new.start_with_windows {
            handle
                .state::<Autostart>()
                .apply_logged(new.start_with_windows);
        }
    });
    app.manage(autostart);
}

fn launch_command() -> String {
    match std::env::current_exe() {
        Ok(exe) => super::launch_command(&exe),
        Err(error) => {
            log::error!("cannot find Echo's own executable: {error}");
            String::new()
        }
    }
}

/// The value is named after the app identifier, so a development build (run with its own
/// identifier) never touches an installed Echo's entry, and the uninstaller knows the name.
#[cfg(windows)]
fn entry(app: &AppHandle) -> Box<dyn super::StartupEntry> {
    Box::new(super::windows::RunKeyEntry::new(&app.config().identifier))
}

/// Other platforms have no sign-in registration yet.
#[cfg(not(windows))]
fn entry(_app: &AppHandle) -> Box<dyn super::StartupEntry> {
    struct Unsupported;
    impl super::StartupEntry for Unsupported {
        fn registered(&self) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn register(&self, _: &str) -> Result<(), String> {
            Err("starting at sign-in is only supported on Windows".into())
        }
        fn unregister(&self) -> Result<(), String> {
            Ok(())
        }
        fn disabled_in_windows(&self) -> Result<bool, String> {
            Ok(false)
        }
    }
    Box::new(Unsupported)
}
