//! The real [`ShortcutListener`] on Windows: our own `WH_KEYBOARD_LL` hook (ADR 0002).
//!
//! The hook runs on a dedicated thread with its own message loop. Its callback stays minimal,
//! because Windows skips (and eventually removes) hooks that take too long: it asks the
//! [`Matcher`] what to do, hands events to the sinks (which only forward them into channels),
//! and returns. All decisions live in the matcher, which is unit-tested with synthetic keys.
//!
//! Windows allows the callback no context pointer, so the matcher and sinks are process-wide:
//! there is one keyboard hook per process.
//!
//! Since Windows 7 a hook that misses the system's timeout even once (for example while the
//! machine is starved of CPU) is removed silently, with no way to find out. The hook thread
//! therefore runs at high priority and reinstalls the hook every few seconds: the new hook is
//! installed before the old one is removed, so no keystroke is lost, and a keystroke that reaches
//! both while they overlap is recognised and handled only once.
//!
//! Windows does not call the hook while Echo's own window has focus (observed on Windows 11), so
//! that window forwards its key events through [`ShortcutListener::own_window_key`] into the
//! same matcher. Both routes are idempotent for a key reported twice (a second key-down counts
//! as auto-repeat, a second key-up as a release of a key that is not down).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, HC_ACTION, HHOOK,
    KBDLLHOOKSTRUCT, LLKHF_UP, MSG, PostThreadMessageW, SetTimer, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_QUIT, WM_TIMER,
};

use super::keys;
use super::matcher::{KeyboardState, Matcher, Output, RawKey};
use super::{
    CaptureSink, KeyAction, KeyCombination, Shortcut, ShortcutError, ShortcutListener, ShortcutSink,
};

/// `dwExtraInfo` of every keystroke Echo injects (the masking key; later the Inserter's
/// keystrokes). The hook ignores keystrokes carrying it.
pub const ECHO_INPUT_TAG: usize = 0x4543_484F; // "ECHO"

/// An unassigned virtual-key code: pressing it does nothing, but Windows counts it as "another
/// key", so a following bare Alt or Win release opens no menu.
const MASK_VK: u16 = 0xE8;

/// How often the hook is reinstalled, in case Windows removed it.
const REINSTALL_EVERY_MS: u32 = 5_000;

/// The last keystroke handled and whether it was swallowed, so a keystroke that reaches both
/// hooks during a reinstall is handled once.
static LAST_KEY: Mutex<Option<(KeyStamp, bool)>> = Mutex::new(None);

#[derive(Clone, Copy, PartialEq, Eq)]
struct KeyStamp {
    vk: u32,
    scan: u32,
    flags: u32,
    time: u32,
    extra: usize,
}

static MATCHER: Mutex<Matcher> = Mutex::new(Matcher::new());
static SINKS: Mutex<Sinks> = Mutex::new(Sinks {
    shortcut: None,
    capture: None,
});

struct Sinks {
    shortcut: Option<ShortcutSink>,
    capture: Option<CaptureSink>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The keyboard hook. Bindings may be set before [`start`](ShortcutListener::start); dropping
/// the listener removes the hook.
#[derive(Default)]
pub struct WindowsShortcutListener {
    thread: Option<(u32, JoinHandle<()>)>,
}

impl WindowsShortcutListener {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ShortcutListener for WindowsShortcutListener {
    fn start(&mut self, sink: ShortcutSink) -> Result<(), ShortcutError> {
        if self.thread.is_some() {
            return Err(ShortcutError::Unavailable("already started".into()));
        }
        lock(&SINKS).shortcut = Some(sink);
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("echo-keyboard-hook".into())
            .spawn(move || run_hook_thread(&tx))
            .map_err(|e| ShortcutError::Unavailable(e.to_string()))?;
        match rx.recv() {
            Ok(Ok(thread_id)) => {
                self.thread = Some((thread_id, thread));
                Ok(())
            }
            Ok(Err(reason)) => Err(ShortcutError::Unavailable(reason)),
            Err(_) => Err(ShortcutError::Unavailable("the hook thread died".into())),
        }
    }

    fn bind(
        &mut self,
        shortcut: Shortcut,
        combination: Option<KeyCombination>,
    ) -> Result<(), ShortcutError> {
        lock(&MATCHER)
            .bind(shortcut, combination.as_ref())
            .map_err(|reason| ShortcutError::Rejected {
                combination: combination.map(|c| c.to_string()).unwrap_or_default(),
                reason,
            })
    }

    fn capture(&mut self, sink: Option<CaptureSink>) -> Result<(), ShortcutError> {
        let capturing = sink.is_some();
        if capturing {
            lock(&SINKS).capture = sink;
            lock(&MATCHER).set_capturing(true);
        } else {
            lock(&MATCHER).set_capturing(false);
            lock(&SINKS).capture = None;
        }
        Ok(())
    }

    fn own_window_key(&mut self, key: &str, action: KeyAction) {
        let Some(vk) = keys::vk_of_capture_name(key) else {
            return;
        };
        let capturing = lock(&MATCHER).is_capturing();
        let pressed = action == KeyAction::Pressed;
        // Echo's window cannot hold back what it already received, so "swallowed" here only
        // means the keystroke belongs to a shortcut.
        let shortcut_key = process(RawKey {
            vk,
            scan: 0,
            up: !pressed,
            from_echo: false,
        });
        // Windows saw every key go down, so a bare Win or Alt release would open the Start menu
        // or a menu bar: mask it when it is part of a shortcut or being captured (losing focus
        // would also end the capture).
        let alt_or_win = |k: u16| {
            matches!(
                keys::modifier_kind(k),
                Some(keys::ModifierKind::Alt | keys::ModifierKind::Win)
            )
        };
        let alt_or_win_held = [
            keys::vk::LMENU,
            keys::vk::RMENU,
            keys::vk::LWIN,
            keys::vk::RWIN,
        ]
        .into_iter()
        .any(|k| System.is_down(k));
        if pressed && ((capturing && alt_or_win(vk)) || (shortcut_key && alt_or_win_held)) {
            send_mask_key();
        }
    }
}

impl Drop for WindowsShortcutListener {
    fn drop(&mut self) {
        if let Some((thread_id, thread)) = self.thread.take() {
            // SAFETY: posting a message to a thread id has no memory-safety preconditions.
            let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
            let _ = thread.join();
        }
        let mut sinks = lock(&SINKS);
        sinks.shortcut = None;
        sinks.capture = None;
    }
}

fn install_hook() -> windows::core::Result<HHOOK> {
    // SAFETY: `hook_proc` matches HOOKPROC and lives for the whole program.
    unsafe {
        let module = GetModuleHandleW(None).ok().map(|m| HINSTANCE(m.0));
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), module, 0)
    }
}

/// Installs the hook, reports the outcome, then pumps messages (which is when Windows calls the
/// hook) until `WM_QUIT`, reinstalling the hook on every timer tick.
fn run_hook_thread(started: &mpsc::Sender<Result<u32, String>>) {
    // SAFETY: plain Win32 calls on this thread's own handles and hook.
    unsafe {
        // Best effort: a starved hook thread is the usual reason Windows drops a hook.
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        let mut hook = match install_hook() {
            Ok(hook) => hook,
            Err(error) => {
                let _ = started.send(Err(error.to_string()));
                return;
            }
        };
        SetTimer(None, 0, REINSTALL_EVERY_MS, None);
        let _ = started.send(Ok(GetCurrentThreadId()));
        let mut msg = MSG::default();
        // GetMessageW returns 0 for WM_QUIT and -1 on error; both end the loop.
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            if msg.message == WM_TIMER
                && let Ok(fresh) = install_hook()
            {
                let _ = UnhookWindowsHookEx(hook);
                hook = fresh;
            }
        }
        let _ = UnhookWindowsHookEx(hook);
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, lparam points to a KBDLLHOOKSTRUCT (WH_KEYBOARD_LL contract).
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        // A panic must never unwind into Windows; if one happens, the key passes through.
        if let Ok(true) = catch_unwind(AssertUnwindSafe(|| on_key(info))) {
            return LRESULT(1);
        }
    }
    // SAFETY: forwarding the unchanged arguments to the next hook.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Handles one keystroke; returns whether to swallow it.
fn on_key(info: &KBDLLHOOKSTRUCT) -> bool {
    let stamp = KeyStamp {
        vk: info.vkCode,
        scan: info.scanCode,
        flags: info.flags.0,
        time: info.time,
        extra: info.dwExtraInfo,
    };
    let mut last = lock(&LAST_KEY);
    if let Some((seen, swallowed)) = *last
        && seen == stamp
    {
        return swallowed;
    }
    let swallow = decide(info);
    *last = Some((stamp, swallow));
    swallow
}

fn decide(info: &KBDLLHOOKSTRUCT) -> bool {
    process(RawKey {
        vk: u16::try_from(info.vkCode).unwrap_or(0),
        scan: info.scanCode,
        up: info.flags.contains(LLKHF_UP),
        from_echo: info.dwExtraInfo == ECHO_INPUT_TAG,
    })
}

/// Runs one keystroke through the matcher, delivers its events and masks if asked; returns
/// whether to swallow it.
fn process(key: RawKey) -> bool {
    let mut outputs = Vec::new();
    let verdict = lock(&MATCHER).handle(key, &System, &mut |o| outputs.push(o));
    if !outputs.is_empty() {
        let mut sinks = lock(&SINKS);
        for output in outputs {
            match output {
                Output::Shortcut(event) => {
                    if let Some(sink) = sinks.shortcut.as_mut() {
                        sink(event);
                    }
                }
                Output::Captured(key) => {
                    if let Some(sink) = sinks.capture.as_mut() {
                        sink(key);
                    }
                }
            }
        }
    }
    if verdict.mask {
        send_mask_key();
    }
    verdict.swallow
}

fn send_mask_key() {
    let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(MASK_VK),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: ECHO_INPUT_TAG,
            },
        },
    };
    let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
    // SAFETY: `inputs` is a valid array of keyboard INPUTs of the size we pass.
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

struct System;

impl KeyboardState for System {
    fn is_down(&self, vk: u16) -> bool {
        // SAFETY: GetAsyncKeyState has no preconditions. The high bit means "down now".
        (unsafe { GetAsyncKeyState(i32::from(vk)) } as u16) & 0x8000 != 0
    }

    fn echo_in_foreground(&self) -> bool {
        // SAFETY: plain Win32 queries; a null window yields process id 0.
        unsafe {
            let window: HWND = GetForegroundWindow();
            let mut process = 0u32;
            GetWindowThreadProcessId(window, Some(&mut process));
            process == std::process::id()
        }
    }
}
