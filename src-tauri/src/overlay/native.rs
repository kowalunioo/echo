//! Win32 details of the Overlay window (`overlay.md` rules 8–12, 15).
//!
//! Focus: the window is created non-focusable (`"focusable": false` → `WS_EX_NOACTIVATE`), so
//! clicking it never makes it the foreground window, and it is only ever shown with
//! `SWP_NOACTIVATE` (never Tauri's `show`, which activates). `WS_EX_TOOLWINDOW` keeps it out of
//! the taskbar and Alt+Tab; `WS_EX_TOPMOST` plus a periodic re-assert keeps it above other
//! windows. A window region the size of the pill lets clicks beside it through.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, HWND_TOPMOST, SW_HIDE, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

use super::placement::Rect;

/// Gives the window the Overlay's extended styles.
pub fn prepare(hwnd: HWND) {
    // SAFETY: plain Win32 calls on a window handle owned by this process.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let style = (style & !(WS_EX_APPWINDOW.0 as isize))
            | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST).0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// Places the window and shows it without activating it.
pub fn show(hwnd: HWND, rect: Rect) {
    prepare(hwnd);
    // Twice: moving onto a monitor with other scaling makes Windows resize the window for the
    // new DPI; the second call restores the exact size computed for that monitor (rule 15).
    for _ in 0..2 {
        // SAFETY: as above.
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                rect.x,
                rect.y,
                rect.width as i32,
                rect.height as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }
}

pub fn hide(hwnd: HWND) {
    // SAFETY: as above. SW_HIDE never activates anything.
    let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
}

/// Puts the window back on top of windows that became topmost after it (rule 12).
pub fn keep_on_top(hwnd: HWND) {
    // SAFETY: as above.
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
}

/// Clips the window to a pill of `width` × `height` physical pixels, centred horizontally and
/// vertically, with a pixel to spare so the pill's anti-aliased edge stays visible.
pub fn set_shape(hwnd: HWND, width: f64, height: f64) {
    let mut window = RECT::default();
    // SAFETY: as above; `window` outlives the call.
    if unsafe { GetWindowRect(hwnd, &mut window) }.is_err() {
        return;
    }
    let (window_width, window_height) = (window.right - window.left, window.bottom - window.top);
    let width = (width.ceil() as i32 + 2).min(window_width);
    let height = (height.ceil() as i32 + 2).min(window_height);
    let left = (window_width - width) / 2;
    let top = (window_height - height) / 2;
    // SAFETY: as above. The system owns the region after SetWindowRgn succeeds.
    unsafe {
        let region = CreateRoundRectRgn(
            left,
            top,
            left + width + 1,
            top + height + 1,
            height,
            height,
        );
        SetWindowRgn(hwnd, Some(region), true);
    }
}

/// The Windows accessibility "Text size" factor: 1.0 at 100 %, up to 2.25.
pub fn text_scale() -> f64 {
    windows_registry::CURRENT_USER
        .open(r"SOFTWARE\Microsoft\Accessibility")
        .and_then(|key| key.get_u32("TextScaleFactor"))
        .map_or(1.0, |percent| f64::from(percent.clamp(100, 225)) / 100.0)
}
