//! Opt-in tests against the real Windows clipboard and keyboard (`cargo test -- --ignored`).
//!
//! They change the clipboard, so each first saves the user's clipboard and puts it back exactly
//! at the end. Keystrokes go only into a window these tests create, and only while it is in the
//! foreground — checked right before every injection.

use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    CloseClipboard, CountClipboardFormats, EmptyClipboard, GetClipboardData,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, ES_MULTILINE, GetForegroundWindow,
    GetMessageW, GetWindowTextLengthW, GetWindowTextW, MSG, PostMessageW, PostThreadMessageW,
    RegisterClassW, SW_SHOW, SetForegroundWindow, ShowWindow, TranslateMessage, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_PASTE, WM_QUIT, WNDCLASSW, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use super::super::Inserter;
use super::super::paste::{
    Clipboard, Desktop, Key, KeyEvent, PasteInserter, Restore, SendError, SystemClock,
};
use super::{WindowsClipboard, WindowsDesktop};

/// Serialises these tests: they share the one system clipboard.
static CLIPBOARD_LOCK: Mutex<()> = Mutex::new(());

/// Saves the user's clipboard and restores it exactly when dropped.
struct UserClipboard {
    clipboard: WindowsClipboard,
    saved: Option<super::clipboard::Saved>,
    fingerprint: Vec<(u32, usize, u64)>,
}

impl UserClipboard {
    fn save() -> Self {
        let clipboard = WindowsClipboard::start().unwrap();
        let saved = clipboard.save().expect("save the user's clipboard");
        let fingerprint = saved.fingerprint();
        Self {
            clipboard,
            saved: Some(saved),
            fingerprint,
        }
    }
}

impl Drop for UserClipboard {
    fn drop(&mut self) {
        if let Some(saved) = self.saved.take() {
            self.clipboard
                .restore_unconditionally(saved)
                .expect("restore the user's clipboard");
            let marks = [
                mark("CanIncludeInClipboardHistory"),
                mark("CanUploadToCloudClipboard"),
            ];
            let now: Vec<_> = self
                .clipboard
                .save()
                .unwrap()
                .fingerprint()
                .into_iter()
                .filter(|(format, ..)| !marks.contains(format))
                .collect();
            let before: Vec<_> = self
                .fingerprint
                .iter()
                .copied()
                .filter(|(f, ..)| !marks.contains(f))
                .collect();
            assert_eq!(now, before, "the user's clipboard was not restored exactly");
        }
    }
}

fn mark(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
    // SAFETY: NUL-terminated string.
    unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
}

fn open_as_other() {
    for _ in 0..50 {
        // SAFETY: opened without an owner window, closed by the caller.
        if unsafe { OpenClipboard(None) }.is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("could not open the clipboard");
}

/// Writes `text` to the clipboard the way another application would.
fn copy_as_other(text: Option<&str>) {
    open_as_other();
    // SAFETY: the clipboard is open on this thread; the block is handed to the clipboard.
    unsafe {
        EmptyClipboard().unwrap();
        if let Some(text) = text {
            let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
            let memory = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2).unwrap();
            let pointer = GlobalLock(memory).cast::<u16>();
            std::ptr::copy_nonoverlapping(units.as_ptr(), pointer, units.len());
            let _ = GlobalUnlock(memory);
            SetClipboardData(CF_UNICODETEXT.0.into(), Some(HANDLE(memory.0))).unwrap();
        }
        CloseClipboard().unwrap();
    }
}

/// Reads the clipboard text the way another application would.
fn read_as_other() -> Option<String> {
    open_as_other();
    // SAFETY: the clipboard is open on this thread; the text is read up to its NUL.
    unsafe {
        let text = GetClipboardData(CF_UNICODETEXT.0.into())
            .ok()
            .map(|handle| {
                let pointer =
                    GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0)).cast::<u16>();
                let len = (0..).take_while(|&i| *pointer.add(i) != 0).count();
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(pointer, len));
                let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0));
                text
            });
        CloseClipboard().unwrap();
        text
    }
}

fn format_count() -> i32 {
    open_as_other();
    // SAFETY: the clipboard is open on this thread.
    let count = unsafe { CountClipboardFormats() };
    // SAFETY: opened above.
    unsafe { CloseClipboard() }.unwrap();
    count
}

fn has_format(format: u32) -> bool {
    open_as_other();
    // SAFETY: the clipboard is open on this thread.
    let present = unsafe { IsClipboardFormatAvailable(format) }.is_ok();
    // SAFETY: opened above.
    unsafe { CloseClipboard() }.unwrap();
    present
}

#[test]
#[ignore = "uses the real clipboard"]
fn real_clipboard_marks_observes_reads_and_restores() {
    let _lock = CLIPBOARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _user = UserClipboard::save();
    let clipboard = WindowsClipboard::start().unwrap();

    // Put, observed read, restore.
    copy_as_other(Some("previous"));
    let saved = clipboard.save().unwrap();
    clipboard.put("hello zażółć").unwrap();
    assert!(!clipboard.changed());
    assert_eq!(clipboard.last_read(), None);
    for name in [
        "CanIncludeInClipboardHistory",
        "CanUploadToCloudClipboard",
        "ExcludeClipboardContentFromMonitorProcessing",
    ] {
        assert!(has_format(mark(name)), "{name} missing");
    }
    assert_eq!(read_as_other().as_deref(), Some("hello zażółć"));
    assert!(clipboard.last_read().is_some(), "the read was not observed");
    assert_eq!(clipboard.restore(saved).unwrap(), Restore::Restored);
    assert_eq!(read_as_other().as_deref(), Some("previous"));
    assert!(
        has_format(mark("CanIncludeInClipboardHistory")),
        "restore not kept out of history"
    );

    // An empty clipboard stays empty.
    copy_as_other(None);
    let saved = clipboard.save().unwrap();
    clipboard.put("hello").unwrap();
    assert_eq!(clipboard.restore(saved).unwrap(), Restore::Restored);
    assert_eq!(format_count(), 0);

    // The user's newer copy wins.
    copy_as_other(Some("previous"));
    let saved = clipboard.save().unwrap();
    clipboard.put("hello").unwrap();
    copy_as_other(Some("newer"));
    assert!(clipboard.changed());
    assert_eq!(clipboard.restore(saved).unwrap(), Restore::KeptNewer);
    assert_eq!(read_as_other().as_deref(), Some("newer"));
}

/// A test window with an edit control on its own UI thread.
struct TestWindow {
    top: HWND,
    edit: HWND,
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}

// SAFETY: window handles are plain identifiers usable from any thread.
unsafe impl Send for TestWindow {}

impl TestWindow {
    fn open() -> Self {
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            // SAFETY: plain Win32 window creation and message loop on this thread.
            unsafe {
                let instance = GetModuleHandleW(None).unwrap();
                let class = w!("EchoInsertionTestWindow");
                RegisterClassW(&WNDCLASSW {
                    lpfnWndProc: Some(test_window_proc),
                    hInstance: instance.into(),
                    lpszClassName: class,
                    ..Default::default()
                });
                let top = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    class,
                    w!("Echo insertion test"),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    100,
                    100,
                    400,
                    200,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )
                .unwrap();
                let edit = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("EDIT"),
                    w!(""),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_MULTILINE as u32),
                    0,
                    0,
                    380,
                    150,
                    Some(top),
                    None,
                    Some(instance.into()),
                    None,
                )
                .unwrap();
                let _ = ShowWindow(top, SW_SHOW);
                tx.send((top.0 as isize, edit.0 as isize, GetCurrentThreadId()))
                    .unwrap();
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });
        let (top, edit, thread_id) = rx.recv().unwrap();
        Self {
            top: HWND(top as *mut _),
            edit: HWND(edit as *mut _),
            thread_id,
            thread: Some(thread),
        }
    }

    fn text(&self) -> String {
        // SAFETY: the edit control lives until drop; the buffer is sized for its text.
        unsafe {
            let len = GetWindowTextLengthW(self.edit) as usize;
            let mut buffer = vec![0u16; len + 1];
            let copied = GetWindowTextW(self.edit, &mut buffer) as usize;
            String::from_utf16_lossy(&buffer[..copied])
        }
    }

    /// Brings the window to the front; `false` if Windows refused (foreground lock).
    fn bring_to_front(&self) -> bool {
        // SAFETY: plain calls with our own window handles.
        unsafe {
            let _ = SetForegroundWindow(self.top);
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(self.edit));
            std::thread::sleep(Duration::from_millis(200));
            GetForegroundWindow() == self.top
        }
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        // SAFETY: posting WM_QUIT to our own UI thread.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

unsafe extern "system" fn test_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: forwarding the arguments Windows gave us.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// Instead of keystrokes, asks the test's edit control to paste — a real reader of the
/// clipboard without injecting any input.
struct WmPasteDesktop {
    edit: isize,
}

impl Desktop for WmPasteDesktop {
    fn modifiers_held(&self) -> bool {
        false
    }

    fn foreground_elevated(&self) -> bool {
        false
    }

    fn send(&mut self, events: &[KeyEvent]) -> Result<(), SendError> {
        if events.contains(&KeyEvent::Down(Key::V)) {
            // SAFETY: posting a message to our own edit control.
            unsafe {
                PostMessageW(
                    Some(HWND(self.edit as *mut _)),
                    WM_PASTE,
                    WPARAM(0),
                    LPARAM(0),
                )
            }
            .map_err(|e| SendError::Failed(e.to_string()))?;
        }
        Ok(())
    }
}

#[test]
#[ignore = "uses the real clipboard"]
fn real_edit_control_reads_the_transcript_and_the_clipboard_comes_back() {
    let _lock = CLIPBOARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _user = UserClipboard::save();
    let window = TestWindow::open();
    copy_as_other(Some("previous"));

    let clipboard = Arc::new(WindowsClipboard::start().unwrap());
    let desktop = WmPasteDesktop {
        edit: window.edit.0 as isize,
    };
    let mut inserter = PasteInserter::new(Arc::clone(&clipboard), desktop, Arc::new(SystemClock));
    let start = Instant::now();
    inserter.insert("hello zażółć").unwrap();
    inserter.wait_for_restore();

    assert_eq!(window.text(), "hello zażółć");
    assert!(
        clipboard.last_read().is_some(),
        "the edit control's read was not observed"
    );
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "restored only after {:?}",
        start.elapsed()
    );
    assert_eq!(read_as_other().as_deref(), Some("previous"));
}

/// The real keyboard, refusing to inject unless the test window is in the foreground.
struct GuardedDesktop {
    top: isize,
}

impl Desktop for GuardedDesktop {
    fn modifiers_held(&self) -> bool {
        WindowsDesktop.modifiers_held()
    }

    fn foreground_elevated(&self) -> bool {
        WindowsDesktop.foreground_elevated()
    }

    fn send(&mut self, events: &[KeyEvent]) -> Result<(), SendError> {
        // SAFETY: GetForegroundWindow has no preconditions.
        if unsafe { GetForegroundWindow() }.0 as isize != self.top {
            return Err(SendError::Failed(
                "the test window lost the foreground".into(),
            ));
        }
        WindowsDesktop.send(events)
    }
}

#[test]
#[ignore = "injects keystrokes into its own test window"]
fn real_keystrokes_paste_and_type_into_the_test_window() {
    let _lock = CLIPBOARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _user = UserClipboard::save();
    let window = TestWindow::open();
    if !window.bring_to_front() {
        eprintln!("SKIPPED: Windows did not let the test window into the foreground");
        return;
    }
    copy_as_other(Some("previous"));
    let clipboard = Arc::new(WindowsClipboard::start().unwrap());
    let desktop = GuardedDesktop {
        top: window.top.0 as isize,
    };
    let mut inserter = PasteInserter::new(Arc::clone(&clipboard), desktop, Arc::new(SystemClock));

    // Ctrl+V.
    inserter.insert("hello ").unwrap();
    inserter.wait_for_restore();
    assert!(
        clipboard.last_read().is_some(),
        "the paste's read was not observed"
    );
    assert_eq!(read_as_other().as_deref(), Some("previous"));

    // Typing, while another process holds the clipboard open.
    open_as_other();
    let typed = inserter.insert("zażółć gęślą jaźń");
    // SAFETY: opened above.
    unsafe { CloseClipboard() }.unwrap();
    typed.unwrap();
    inserter.wait_for_restore();
    std::thread::sleep(Duration::from_millis(300));

    assert_eq!(window.text(), "hello zażółć gęślą jaźń");
    assert_eq!(read_as_other().as_deref(), Some("previous"));
    // The test's own window is not elevated.
    assert!(window.bring_to_front() && !WindowsDesktop.foreground_elevated());
}
