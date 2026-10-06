//! Starting Echo when the user signs in to Windows (`autostart.md`).
//!
//! The registration itself sits behind [`StartupEntry`] so the rules can be tested with a fake;
//! the real entry is a value in the per-user `Run` key ([`windows::RunKeyEntry`]). Echo applies
//! the stored "Start Echo when I sign in to Windows" setting at every launch (self-heal, rule 4)
//! and whenever it changes (rule 3), and never re-registers itself while the user has turned it
//! off in Windows "Startup apps" (rule 9).

pub mod commands;
#[cfg(windows)]
pub mod windows;

use std::path::Path;

use serde::Serialize;
use specta::Type;

use crate::window::AUTOSTART_ARG;

/// The per-user sign-in registration of one executable.
pub trait StartupEntry: Send + Sync {
    /// The command line currently registered, or `None` when there is no registration.
    fn registered(&self) -> Result<Option<String>, String>;
    /// Registers `command` to run at sign-in, replacing any earlier command.
    fn register(&self, command: &str) -> Result<(), String>;
    /// Removes the registration and anything Windows keeps about it. Succeeds when there is none.
    fn unregister(&self) -> Result<(), String>;
    /// Whether the user has turned the entry off in Windows "Startup apps".
    fn disabled_in_windows(&self) -> Result<bool, String>;
}

/// What applying the setting did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The registration was written (it was missing or pointed elsewhere).
    Registered,
    /// The registration was already correct.
    Unchanged,
    /// The user turned Echo off in Windows; the registration was left alone.
    DisabledInWindows,
    /// The registration is gone (or never existed).
    Unregistered,
}

/// The effective autostart state the App page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AutostartStatus {
    /// The setting is on but Windows "Startup apps" has Echo's entry turned off, so Echo will not
    /// start at sign-in (rule 9).
    pub disabled_in_windows: bool,
}

/// The command line registered for `exe`: the quoted path plus the autostart marker, so a launch
/// at sign-in starts hidden (rule 6).
pub fn launch_command(exe: &Path) -> String {
    format!("\"{}\" {AUTOSTART_ARG}", exe.display())
}

/// Keeps the sign-in registration in step with the setting.
pub struct Autostart {
    entry: Box<dyn StartupEntry>,
    command: String,
}

impl Autostart {
    /// `command` is what the registration runs, normally [`launch_command`] of this executable.
    pub fn new(entry: Box<dyn StartupEntry>, command: String) -> Self {
        Self { entry, command }
    }

    /// Makes the registration match `enabled` (rules 2, 4, 5 and 9).
    pub fn apply(&self, enabled: bool) -> Result<Applied, String> {
        if !enabled {
            self.entry.unregister()?;
            return Ok(Applied::Unregistered);
        }
        if self.command.is_empty() {
            return Err("Echo's own executable path is unknown".into());
        }
        if self.entry.disabled_in_windows()? {
            return Ok(Applied::DisabledInWindows);
        }
        if self.entry.registered()?.as_deref() == Some(self.command.as_str()) {
            return Ok(Applied::Unchanged);
        }
        self.entry.register(&self.command)?;
        Ok(Applied::Registered)
    }

    /// Applies `enabled` and logs the outcome; a failure never stops Echo (rule 4).
    pub fn apply_logged(&self, enabled: bool) {
        match self.apply(enabled) {
            Ok(Applied::Registered) => log::info!("registered Echo to start at sign-in"),
            Ok(Applied::Unchanged) => log::info!("start at sign-in already registered"),
            Ok(Applied::DisabledInWindows) => {
                log::info!("start at sign-in is turned off in Windows Startup apps; left alone")
            }
            Ok(Applied::Unregistered) => log::info!("Echo does not start at sign-in"),
            Err(error) => log::error!("could not apply start at sign-in: {error}"),
        }
    }

    /// The effective state for a stored setting of `enabled`.
    pub fn status(&self, enabled: bool) -> AutostartStatus {
        let disabled_in_windows = enabled
            && self.entry.disabled_in_windows().unwrap_or_else(|error| {
                log::warn!("could not read the Windows Startup apps state: {error}");
                false
            });
        AutostartStatus {
            disabled_in_windows,
        }
    }
}

#[cfg(test)]
pub mod fake {
    use std::sync::{Arc, Mutex};

    use super::StartupEntry;

    /// What the fake registry holds.
    #[derive(Debug, Default)]
    pub struct FakeState {
        pub command: Option<String>,
        pub disabled_in_windows: bool,
        pub writes: usize,
        pub failing: bool,
    }

    /// An in-memory [`StartupEntry`] whose state the test can look at and change.
    #[derive(Clone, Default)]
    pub struct FakeStartupEntry(pub Arc<Mutex<FakeState>>);

    impl FakeStartupEntry {
        pub fn state(&self) -> std::sync::MutexGuard<'_, FakeState> {
            self.0.lock().unwrap()
        }

        fn check(&self) -> Result<(), String> {
            if self.state().failing {
                Err("access denied".into())
            } else {
                Ok(())
            }
        }
    }

    impl StartupEntry for FakeStartupEntry {
        fn registered(&self) -> Result<Option<String>, String> {
            self.check()?;
            Ok(self.state().command.clone())
        }

        fn register(&self, command: &str) -> Result<(), String> {
            self.check()?;
            let mut state = self.state();
            state.command = Some(command.to_owned());
            state.writes += 1;
            Ok(())
        }

        fn unregister(&self) -> Result<(), String> {
            self.check()?;
            let mut state = self.state();
            state.command = None;
            state.disabled_in_windows = false;
            Ok(())
        }

        fn disabled_in_windows(&self) -> Result<bool, String> {
            self.check()?;
            Ok(self.state().disabled_in_windows)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::fake::FakeStartupEntry;
    use super::*;

    const EXE: &str = r"C:\Users\Ada\AppData\Local\Echo\echo.exe";

    fn autostart() -> (Autostart, FakeStartupEntry) {
        let entry = FakeStartupEntry::default();
        let command = launch_command(&PathBuf::from(EXE));
        (Autostart::new(Box::new(entry.clone()), command), entry)
    }

    // autostart.md rule 1.
    #[test]
    fn autostart_is_off_by_default() {
        assert!(!crate::settings::Settings::defaults(None).start_with_windows);
    }

    #[test]
    fn the_registered_command_runs_this_executable_with_the_autostart_marker() {
        assert_eq!(
            launch_command(&PathBuf::from(EXE)),
            format!("\"{EXE}\" --autostart")
        );
    }

    // autostart.md acceptance test 1.
    #[test]
    fn turning_on_registers_this_executable_and_turning_off_removes_it() {
        let (autostart, entry) = autostart();

        assert_eq!(autostart.apply(true), Ok(Applied::Registered));
        assert_eq!(
            entry.state().command.as_deref(),
            Some(format!("\"{EXE}\" --autostart").as_str())
        );

        assert_eq!(autostart.apply(false), Ok(Applied::Unregistered));
        assert_eq!(entry.state().command, None);
    }

    // autostart.md acceptance test 2.
    #[test]
    fn a_missing_registration_is_restored() {
        let (autostart, entry) = autostart();

        assert_eq!(autostart.apply(true), Ok(Applied::Registered));
        assert!(entry.state().command.is_some());
    }

    // autostart.md acceptance test 2.
    #[test]
    fn a_registration_for_an_old_install_location_is_replaced() {
        let (autostart, entry) = autostart();
        entry.state().command = Some(r#""D:\Old\echo.exe" --autostart"#.into());

        assert_eq!(autostart.apply(true), Ok(Applied::Registered));
        assert_eq!(
            entry.state().command.as_deref(),
            Some(format!("\"{EXE}\" --autostart").as_str())
        );
    }

    #[test]
    fn a_correct_registration_is_not_rewritten() {
        let (autostart, entry) = autostart();
        autostart.apply(true).unwrap();

        assert_eq!(autostart.apply(true), Ok(Applied::Unchanged));
        assert_eq!(entry.state().writes, 1);
    }

    // autostart.md acceptance test 3.
    #[test]
    fn turning_off_without_a_registration_is_not_an_error() {
        let (autostart, _) = autostart();
        assert_eq!(autostart.apply(false), Ok(Applied::Unregistered));
    }

    // autostart.md acceptance test 6 (backend half).
    #[test]
    fn echo_does_not_re_register_while_turned_off_in_windows() {
        let (autostart, entry) = autostart();
        entry.state().command = Some(r#""D:\Old\echo.exe" --autostart"#.into());
        entry.state().disabled_in_windows = true;

        assert_eq!(autostart.apply(true), Ok(Applied::DisabledInWindows));
        assert_eq!(entry.state().writes, 0);
        assert!(entry.state().disabled_in_windows);
        assert_eq!(
            autostart.status(true),
            AutostartStatus {
                disabled_in_windows: true
            }
        );
    }

    #[test]
    fn the_windows_state_only_matters_while_the_setting_is_on() {
        let (autostart, entry) = autostart();
        entry.state().disabled_in_windows = true;

        assert!(!autostart.status(false).disabled_in_windows);
    }

    // autostart.md rule 4: a failure is reported, and `apply_logged` never panics.
    #[test]
    fn a_registry_failure_is_reported_not_fatal() {
        let (autostart, entry) = autostart();
        entry.state().failing = true;

        assert!(autostart.apply(true).is_err());
        autostart.apply_logged(true);
        assert!(!autostart.status(true).disabled_in_windows);
    }
}
