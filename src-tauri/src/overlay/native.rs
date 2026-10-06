//! Win32 details of the Overlay window (`overlay.md` rules 8–12, 15).
//!
//! Focus: the window is created non-focusable (`"focusable": false` → `WS_EX_NOACTIVATE`), so
//! clicking it never makes it the foreground window, and it is only ever shown with
//! `SWP_NOACTIVATE` (never Tauri's `show`, which activates). `WS_EX_TOOLWINDOW` keeps it out of
//! the taskbar and Alt+Tab; `WS_EX_TOPMOST` plus a periodic re-assert keeps it above other
//! windows. A window region the size of the pill lets clicks beside it through.
//!
//! Clicks land in WebView2's child windows, not the top-level one, so the window and every child
//! of it this thread owns are also subclassed to answer `WM_MOUSEACTIVATE` with `MA_NOACTIVATE`.

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, MA_NOACTIVATE, SW_HIDE,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, WM_MOUSEACTIVATE, WM_NCDESTROY, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};
use windows::core::BOOL;

use super::placement::{PillRegion, Rect};

/// The id of the no-activate subclass.
const NO_ACTIVATE: usize = 0x4543_484f; // "ECHO"

unsafe extern "system" fn no_activate(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_NCDESTROY => {
            // SAFETY: removing our own subclass from the window being destroyed.
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(no_activate), NO_ACTIVATE);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
        }
        // SAFETY: passes the message on down the subclass chain.
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn subclass_child(child: HWND, _: LPARAM) -> BOOL {
    // SAFETY: fails harmlessly for windows of other threads or processes (WebView2's renderer).
    let _ = unsafe { SetWindowSubclass(child, Some(no_activate), NO_ACTIVATE, 0) };
    BOOL(1)
}

/// Makes clicks on the window or its children never activate it. Idempotent; call it on the
/// window's thread whenever children may have appeared.
fn guard_activation(hwnd: HWND) {
    // SAFETY: as in `prepare`; the callback only subclasses.
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(no_activate), NO_ACTIVATE, 0);
        let _ = EnumChildWindows(Some(hwnd), Some(subclass_child), LPARAM(0));
    }
}

/// Gives the window the Overlay's extended styles.
pub fn prepare(hwnd: HWND) {
    guard_activation(hwnd);
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

/// Places the window and shows it without activating it, clipped to `pill` (computed for this
/// size and scaling) when the pill's size is known.
pub fn show(hwnd: HWND, rect: Rect, pill: Option<PillRegion>) {
    prepare(hwnd);
    if let Some(pill) = pill {
        set_shape(hwnd, pill);
    }
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

/// Clips the window to the pill (rule 11).
pub fn set_shape(hwnd: HWND, pill: PillRegion) {
    // SAFETY: as above. The system owns the region once SetWindowRgn succeeds; until then it is
    // ours to delete.
    unsafe {
        let region = CreateRoundRectRgn(
            pill.left,
            pill.top,
            pill.right,
            pill.bottom,
            pill.corner,
            pill.corner,
        );
        if region.is_invalid() {
            log::warn!("could not create the Overlay's window region");
            return;
        }
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            log::warn!("could not set the Overlay's window region");
            let _ = DeleteObject(region.into());
        }
    }
}

/// The Windows accessibility "Text size" factor: 1.0 at 100 %, up to 2.25.
pub fn text_scale() -> f64 {
    windows_registry::CURRENT_USER
        .open(r"SOFTWARE\Microsoft\Accessibility")
        .and_then(|key| key.get_u32("TextScaleFactor"))
        .map_or(1.0, |percent| f64::from(percent.clamp(100, 225)) / 100.0)
}
