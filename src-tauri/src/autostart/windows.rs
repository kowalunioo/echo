//! The real [`StartupEntry`]: a string value in the current user's `Run` key, which Windows runs
//! at sign-in without administrator rights (`autostart.md` rule 2). Windows "Startup apps" (and
//! Task Manager's Startup tab) record the user's on/off choice for that value in a sibling
//! `StartupApproved\Run` key as a binary value whose first byte is odd when the entry is off.

use windows_registry::{CURRENT_USER, Key};

use super::StartupEntry;

/// Where per-user sign-in commands live.
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Where Windows keeps the user's Startup apps choice for each `Run` value.
pub const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// The registry error for a key or value that does not exist.
const NOT_FOUND: windows::core::HRESULT =
    windows::Win32::Foundation::ERROR_FILE_NOT_FOUND.to_hresult();

/// One value in the `Run` key (and its `StartupApproved` twin), named after the app identifier so
/// a development build never touches an installed Echo's entry.
pub struct RunKeyEntry {
    run_key: String,
    approved_key: String,
    value_name: String,
}

impl RunKeyEntry {
    /// The entry named `value_name` in the real Windows keys.
    pub fn new(value_name: &str) -> Self {
        Self::at(RUN_KEY, APPROVED_KEY, value_name)
    }

    /// The entry in other keys under `HKEY_CURRENT_USER` (used by tests).
    pub fn at(run_key: &str, approved_key: &str, value_name: &str) -> Self {
        Self {
            run_key: run_key.into(),
            approved_key: approved_key.into(),
            value_name: value_name.into(),
        }
    }

    /// Opens `path` (for writing too when `write`), or `None` when it does not exist.
    fn open(path: &str, write: bool) -> Result<Option<Key>, String> {
        let mut options = CURRENT_USER.options();
        options.read();
        if write {
            options.write();
        }
        match options.open(path) {
            Ok(key) => Ok(Some(key)),
            Err(error) if error.code() == NOT_FOUND => Ok(None),
            Err(error) => Err(format!("cannot open {path}: {error}")),
        }
    }

    fn remove_value(&self, path: &str) -> Result<(), String> {
        let Some(key) = Self::open(path, true)? else {
            return Ok(());
        };
        match key.remove_value(&self.value_name) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == NOT_FOUND => Ok(()),
            Err(error) => Err(format!("cannot remove {}: {error}", self.value_name)),
        }
    }
}

impl StartupEntry for RunKeyEntry {
    fn registered(&self) -> Result<Option<String>, String> {
        let Some(key) = Self::open(&self.run_key, false)? else {
            return Ok(None);
        };
        match key.get_string(&self.value_name) {
            Ok(command) => Ok(Some(command)),
            Err(error) if error.code() == NOT_FOUND => Ok(None),
            // Not a string (someone else's value under our name): treat as wrong, to be replaced.
            Err(_) => Ok(Some(String::new())),
        }
    }

    fn register(&self, command: &str) -> Result<(), String> {
        CURRENT_USER
            .create(&self.run_key)
            .and_then(|key| key.set_string(&self.value_name, command))
            .map_err(|error| format!("cannot write {}: {error}", self.value_name))
    }

    fn unregister(&self) -> Result<(), String> {
        self.remove_value(&self.run_key)?;
        // Forget the Startup apps choice too, so a later opt-in in Echo starts from a clean entry.
        self.remove_value(&self.approved_key)
    }

    fn disabled_in_windows(&self) -> Result<bool, String> {
        let Some(key) = Self::open(&self.approved_key, false)? else {
            return Ok(false);
        };
        match key.get_value(&self.value_name) {
            Ok(value) => Ok(value.first().is_some_and(|flag| flag & 1 == 1)),
            Err(error) if error.code() == NOT_FOUND => Ok(false),
            Err(error) => Err(format!("cannot read the Startup apps state: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use windows_registry::Type;

    use super::*;

    /// A scratch tree under `HKCU\Software`, removed when dropped, so the test never touches the
    /// real `Run` key.
    struct Scratch(String);

    impl Scratch {
        fn new() -> Self {
            Self(format!(
                r"Software\EchoAutostartTest-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ))
        }

        fn entry(&self) -> RunKeyEntry {
            RunKeyEntry::at(
                &format!(r"{}\Run", self.0),
                &format!(r"{}\StartupApproved\Run", self.0),
                "com.enloque.echo.test",
            )
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = CURRENT_USER.remove_tree(&self.0);
        }
    }

    #[test]
    fn registers_reads_and_removes_the_run_value() {
        let scratch = Scratch::new();
        let entry = scratch.entry();

        assert_eq!(entry.registered(), Ok(None));
        entry.unregister().expect("removing nothing is fine");

        entry.register(r#""C:\echo.exe" --autostart"#).unwrap();
        assert_eq!(
            entry.registered(),
            Ok(Some(r#""C:\echo.exe" --autostart"#.into()))
        );

        entry.unregister().unwrap();
        assert_eq!(entry.registered(), Ok(None));
    }

    #[test]
    fn reads_the_startup_apps_choice() {
        let scratch = Scratch::new();
        let entry = scratch.entry();
        assert_eq!(entry.disabled_in_windows(), Ok(false));

        let approved = CURRENT_USER
            .create(format!(r"{}\StartupApproved\Run", scratch.0))
            .unwrap();
        let mut flag = [0u8; 12];
        flag[0] = 3;
        approved
            .set_bytes("com.enloque.echo.test", Type::Bytes, &flag)
            .unwrap();
        assert_eq!(entry.disabled_in_windows(), Ok(true));

        flag[0] = 2;
        approved
            .set_bytes("com.enloque.echo.test", Type::Bytes, &flag)
            .unwrap();
        assert_eq!(entry.disabled_in_windows(), Ok(false));

        entry.unregister().unwrap();
        assert!(approved.get_value("com.enloque.echo.test").is_err());
    }
}
