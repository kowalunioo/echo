//! Following the Windows taskbar theme (rule 3). Windows has no change notification that
//! reaches a process without a visible top-level window reliably, so the theme is read every
//! second; reading one registry value is cheap.

use super::TaskbarTheme;

/// Where the taskbar theme comes from: the registry, or a fake in tests.
pub trait ThemeSource: Send {
    fn taskbar_theme(&self) -> TaskbarTheme;
}

/// Reports the taskbar theme when it differs from the last one seen.
pub struct ThemeWatcher<S> {
    source: S,
    last: TaskbarTheme,
}

impl<S: ThemeSource> ThemeWatcher<S> {
    pub fn new(source: S) -> Self {
        let last = source.taskbar_theme();
        Self { source, last }
    }

    /// The theme at start.
    pub fn current(&self) -> TaskbarTheme {
        self.last
    }

    /// The new theme if the taskbar theme changed since the last call.
    pub fn poll(&mut self) -> Option<TaskbarTheme> {
        let now = self.source.taskbar_theme();
        (now != self.last).then(|| {
            self.last = now;
            now
        })
    }
}

/// The taskbar theme Windows stores for the current user ("Choose your default Windows mode").
pub struct SystemTheme;

impl ThemeSource for SystemTheme {
    #[cfg(windows)]
    fn taskbar_theme(&self) -> TaskbarTheme {
        let light = windows_registry::CURRENT_USER
            .open(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .and_then(|key| key.get_u32("SystemUsesLightTheme"))
            .is_ok_and(|value| value != 0);
        // Missing value: Windows 11's default dark taskbar.
        if light {
            TaskbarTheme::Light
        } else {
            TaskbarTheme::Dark
        }
    }

    #[cfg(not(windows))]
    fn taskbar_theme(&self) -> TaskbarTheme {
        TaskbarTheme::Dark
    }
}
