//! Main-window and process behaviour from `settings-and-first-run.md` rules 16–20: one process
//! per user, hidden start on autostart, and a remembered size and position that never lands on a
//! monitor that is gone. Light/dark follows Windows on its own (no theme is forced, rule 20).
//! Closing the window hides it to the tray (rule 17); see `tray::app`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent};

use crate::data_dir::{without_utf8_bom, write_atomic};

/// The label of the main window in `tauri.conf.json`.
pub const MAIN_WINDOW: &str = "main";

/// The command-line marker the Windows sign-in registration passes (`autostart.md`), so a launch
/// at sign-in starts hidden in the tray.
pub const AUTOSTART_ARG: &str = "--autostart";

/// Whether this process was started by the sign-in registration.
pub fn launched_by_autostart(mut args: impl Iterator<Item = String>) -> bool {
    args.any(|arg| arg == AUTOSTART_ARG)
}

/// Whether the main window appears at launch: always for a manual launch (rule 18), and for an
/// autostart launch only while onboarding is unfinished (rule 1, `autostart.md` decisions).
pub fn show_at_launch(autostart: bool, onboarding_completed: bool) -> bool {
    !autostart || !onboarding_completed
}

/// What the main window shows until its page has painted: the page's own background
/// (`--echo-bg`) for the Windows app theme. WebView2's default is white, which flashes at launch
/// in dark mode.
pub fn background(theme: Option<tauri::Theme>) -> tauri::window::Color {
    match theme {
        Some(tauri::Theme::Dark) => tauri::window::Color(0x1b, 0x1b, 0x1d, 0xff),
        _ => tauri::window::Color(0xf7, 0xf7, 0xf8, 0xff),
    }
}

/// A rectangle in physical pixels (virtual-screen coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    fn overlap(&self, other: &Rect) -> (i64, i64) {
        let span = |a0: i32, a_len: u32, b0: i32, b_len: u32| {
            let start = i64::from(a0).max(i64::from(b0));
            let end = (i64::from(a0) + i64::from(a_len)).min(i64::from(b0) + i64::from(b_len));
            (end - start).max(0)
        };
        (
            span(self.x, self.width, other.x, other.width),
            span(self.y, self.height, other.y, other.height),
        )
    }
}

/// Height of the strip at the top of the window the user grabs to move it.
const TITLE_STRIP: u32 = 40;
/// How much of that strip must be on a monitor for the window to count as reachable.
const MIN_VISIBLE_WIDTH: i64 = 120;
const MIN_VISIBLE_HEIGHT: i64 = 20;

/// Where the main window opens, given its remembered rectangle and the work areas of the
/// connected monitors: the remembered place if the user can still grab its title bar, otherwise
/// centred on the primary monitor and shrunk to fit it (rule 19). `None` when no monitor is known.
pub fn place(saved: Rect, monitors: &[Rect], primary: Option<Rect>) -> Option<Rect> {
    let strip = Rect {
        height: TITLE_STRIP.min(saved.height),
        ..saved
    };
    let reachable = monitors.iter().any(|monitor| {
        let (w, h) = strip.overlap(monitor);
        w >= MIN_VISIBLE_WIDTH.min(i64::from(saved.width))
            && h >= MIN_VISIBLE_HEIGHT.min(i64::from(strip.height))
    });
    if reachable {
        return Some(saved);
    }
    let target = primary.or_else(|| monitors.first().copied())?;
    let width = saved.width.min(target.width);
    let height = saved.height.min(target.height);
    Some(Rect {
        x: target.x + ((target.width - width) / 2) as i32,
        y: target.y + ((target.height - height) / 2) as i32,
        width,
        height,
    })
}

/// The main window's remembered geometry, stored in `window-state.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    /// Outer position and inner size while neither maximised nor minimised.
    pub bounds: Rect,
    pub maximized: bool,
}

impl WindowState {
    pub fn load(path: &Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice(without_utf8_bom(&bytes))
            .inspect_err(|error| log::warn!("ignoring unreadable window state: {error}"))
            .ok()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        write_atomic(path, &json)
    }
}

/// Tracks the main window's normal (not maximised) bounds while it moves, and writes them out
/// when the window closes or Echo exits.
pub struct WindowTracker {
    path: PathBuf,
    last: Mutex<Option<WindowState>>,
}

impl WindowTracker {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            last: Mutex::new(None),
        }
    }

    /// Restores the remembered geometry onto `window`, before it is first shown.
    pub fn restore(&self, window: &WebviewWindow) {
        let Some(state) = WindowState::load(&self.path) else {
            return;
        };
        let monitors: Vec<Rect> = window
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|m| rect_of(m.work_area().position, m.work_area().size))
            .collect();
        let primary = window
            .primary_monitor()
            .ok()
            .flatten()
            .map(|m| rect_of(m.work_area().position, m.work_area().size));
        let Some(bounds) = place(state.bounds, &monitors, primary) else {
            return;
        };
        if bounds != state.bounds {
            log::info!("remembered window position is off-screen; centring on the primary monitor");
        }
        let _ = window.set_size(PhysicalSize::new(bounds.width, bounds.height));
        let _ = window.set_position(PhysicalPosition::new(bounds.x, bounds.y));
        if state.maximized {
            let _ = window.maximize();
        }
        *self.lock() = Some(WindowState { bounds, ..state });
    }

    /// Follows the window's events: remembers its bounds and saves them on close.
    pub fn on_event(&self, window: &tauri::Window, event: &WindowEvent) {
        match event {
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => self.record(window),
            WindowEvent::CloseRequested { .. } => {
                self.record(window);
                self.save();
            }
            _ => {}
        }
    }

    fn record(&self, window: &tauri::Window) {
        let maximized = window.is_maximized().unwrap_or(false);
        if window.is_minimized().unwrap_or(false) {
            return;
        }
        let mut last = self.lock();
        if maximized {
            if let Some(state) = last.as_mut() {
                state.maximized = true;
            }
            return;
        }
        if let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) {
            *last = Some(WindowState {
                bounds: rect_of(position, size),
                maximized: false,
            });
        }
    }

    /// Writes the last known geometry to disk.
    pub fn save(&self) {
        let last = *self.lock();
        if let Some(Err(error)) = last.map(|state| state.save(&self.path)) {
            log::warn!("could not save the window state: {error}");
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<WindowState>> {
        self.last
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn rect_of(position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> Rect {
    Rect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    }
}

/// Brings the main window to the front, restoring it if minimised or hidden (rule 16).
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };
    const LEFT: Rect = Rect {
        x: -2560,
        y: 0,
        width: 2560,
        height: 1400,
    };

    fn window_at(x: i32, y: i32) -> Rect {
        Rect {
            x,
            y,
            width: 980,
            height: 680,
        }
    }

    #[test]
    fn a_window_on_a_connected_monitor_keeps_its_place() {
        let saved = window_at(-2000, 300);
        assert_eq!(place(saved, &[PRIMARY, LEFT], Some(PRIMARY)), Some(saved));
    }

    // settings-and-first-run.md acceptance test 12.
    #[test]
    fn a_window_on_a_removed_monitor_opens_centred_on_the_primary_monitor() {
        let saved = window_at(-2000, 300);

        let placed = place(saved, &[PRIMARY], Some(PRIMARY)).unwrap();

        assert_eq!(
            placed,
            Rect {
                x: 470,
                y: 180,
                width: 980,
                height: 680
            }
        );
    }

    #[test]
    fn a_window_whose_title_bar_is_out_of_reach_is_centred() {
        // Only the bottom edge peeks onto the monitor; the title bar is above it.
        let saved = window_at(100, -660);
        assert_eq!(place(saved, &[PRIMARY], Some(PRIMARY)).unwrap().y, 180);
        // Hanging off the right edge with just a sliver of title bar showing.
        let saved = window_at(1900, 100);
        assert_eq!(place(saved, &[PRIMARY], Some(PRIMARY)).unwrap().x, 470);
    }

    #[test]
    fn a_window_partly_off_screen_but_grabbable_keeps_its_place() {
        let saved = window_at(1500, 200);
        assert_eq!(place(saved, &[PRIMARY], Some(PRIMARY)), Some(saved));
    }

    #[test]
    fn a_window_larger_than_the_primary_monitor_is_shrunk_to_fit() {
        let saved = Rect {
            x: -2500,
            y: 0,
            width: 2500,
            height: 1300,
        };
        assert_eq!(place(saved, &[PRIMARY], Some(PRIMARY)), Some(PRIMARY));
    }

    #[test]
    fn without_any_monitor_information_nothing_is_changed() {
        assert_eq!(place(window_at(5000, 5000), &[], None), None);
    }

    #[test]
    fn window_state_round_trips_through_its_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window-state.json");
        let state = WindowState {
            bounds: window_at(10, 20),
            maximized: true,
        };

        state.save(&path).unwrap();

        assert_eq!(WindowState::load(&path), Some(state));
        std::fs::write(&path, b"garbage").unwrap();
        assert_eq!(WindowState::load(&path), None);
    }

    // Issue #34: a hand-edited file saved with a UTF-8 BOM is still read.
    #[test]
    fn window_state_with_a_utf8_bom_is_still_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window-state.json");
        let state = WindowState {
            bounds: window_at(10, 20),
            maximized: false,
        };
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend(serde_json::to_vec(&state).unwrap());
        std::fs::write(&path, bytes).unwrap();

        assert_eq!(WindowState::load(&path), Some(state));
    }

    #[test]
    fn the_background_before_the_page_paints_matches_the_app_theme() {
        use tauri::{Theme, window::Color};
        assert_eq!(background(Some(Theme::Dark)), Color(0x1b, 0x1b, 0x1d, 0xff));
        assert_eq!(
            background(Some(Theme::Light)),
            Color(0xf7, 0xf7, 0xf8, 0xff)
        );
        assert_eq!(background(None), Color(0xf7, 0xf7, 0xf8, 0xff));
    }

    #[test]
    fn manual_launches_show_the_window_and_autostart_launches_start_hidden() {
        assert!(show_at_launch(false, true));
        assert!(show_at_launch(false, false));
        assert!(!show_at_launch(true, true));
        // An unfinished first run always shows onboarding (rule 1).
        assert!(show_at_launch(true, false));
    }

    #[test]
    fn the_autostart_marker_is_recognised() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(launched_by_autostart(
            args(&["echo.exe", "--autostart"]).into_iter()
        ));
        assert!(!launched_by_autostart(args(&["echo.exe"]).into_iter()));
    }
}
