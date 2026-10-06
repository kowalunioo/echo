//! The real keyboard hook end to end: installs `WH_KEYBOARD_LL`, injects key presses with
//! `SendInput` and checks what the hook reports and swallows.
//!
//! Ignored by default because it presses real keys in the user's session. It only uses F24
//! (absent from almost every keyboard) and left Ctrl, and releases everything it presses.
//! Run with `cargo test --test keyboard_hook -- --ignored`.
#![cfg(windows)]

use std::sync::mpsc;
use std::time::Duration;

use echo_lib::shortcut::{
    KeyAction, Shortcut, ShortcutEvent, ShortcutListener, WindowsShortcutListener,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY,
};

const VK_F24: u16 = 0x87;
const VK_LCONTROL: u16 = 0xA2;
const WAIT: Duration = Duration::from_secs(2);

fn send(vk: u16, up: bool) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    // SAFETY: one valid keyboard INPUT of the size we pass.
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    assert_eq!(sent, 1, "SendInput failed");
}

fn is_down(vk: u16) -> bool {
    // SAFETY: no preconditions.
    (unsafe { GetAsyncKeyState(i32::from(vk)) } as u16) & 0x8000 != 0
}

fn event(action: KeyAction) -> ShortcutEvent {
    ShortcutEvent {
        shortcut: Shortcut::Record,
        action,
    }
}

#[test]
#[ignore = "presses real keys in the user's session"]
fn the_hook_reports_and_swallows_the_bound_shortcut_and_passes_everything_else() {
    let mut listener = WindowsShortcutListener::new();
    let (tx, events) = mpsc::channel();
    listener
        .start(Box::new(move |e| tx.send(e).unwrap()))
        .expect("install the hook");

    // F24 alone: reported and swallowed (Windows never sees it go down).
    listener
        .bind(Shortcut::Record, Some("F24".parse().unwrap()))
        .unwrap();
    send(VK_F24, false);
    assert_eq!(events.recv_timeout(WAIT), Ok(event(KeyAction::Pressed)));
    assert!(!is_down(VK_F24), "the shortcut's key-down reached Windows");
    send(VK_F24, true);
    assert_eq!(events.recv_timeout(WAIT), Ok(event(KeyAction::Released)));

    // Ctrl+F24: Ctrl passes through, F24 is swallowed.
    listener
        .bind(Shortcut::Record, Some("Ctrl+F24".parse().unwrap()))
        .unwrap();
    send(VK_LCONTROL, false);
    send(VK_F24, false);
    assert_eq!(events.recv_timeout(WAIT), Ok(event(KeyAction::Pressed)));
    assert!(is_down(VK_LCONTROL), "Ctrl should pass through");
    assert!(!is_down(VK_F24));
    send(VK_F24, true);
    send(VK_LCONTROL, true);
    assert_eq!(events.recv_timeout(WAIT), Ok(event(KeyAction::Released)));

    // Unbound: nothing is reported and the key reaches Windows.
    listener.bind(Shortcut::Record, None).unwrap();
    send(VK_F24, false);
    std::thread::sleep(Duration::from_millis(150));
    let reached_windows = is_down(VK_F24);
    send(VK_F24, true);
    assert!(reached_windows, "an unbound key was swallowed");
    assert!(events.recv_timeout(Duration::from_millis(200)).is_err());

    drop(listener);
    assert!(
        !is_down(VK_LCONTROL) && !is_down(VK_F24),
        "a key was left down"
    );
}
