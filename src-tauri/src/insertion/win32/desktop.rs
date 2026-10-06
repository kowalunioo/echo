//! [`Desktop`] on Windows: modifier state, `SendInput` and the focused window's privileges.

use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GetLastError, HANDLE};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY, TokenIntegrityLevel,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, VIRTUAL_KEY,
    VK_CONTROL, VK_LWIN, VK_MENU, VK_RETURN, VK_RWIN, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use super::super::paste::{Desktop, Key, KeyEvent, SendError};
use crate::shortcut::ECHO_INPUT_TAG;

/// The real keyboard and focused window.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsDesktop;

impl Desktop for WindowsDesktop {
    fn modifiers_held(&self) -> bool {
        [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            // SAFETY: GetAsyncKeyState has no preconditions. The high bit means "down now".
            .any(|vk| (unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16) & 0x8000 != 0)
    }

    fn foreground_elevated(&self) -> bool {
        // Windows drops simulated input into a process of higher integrity (UIPI) without
        // reporting it, so this is checked up front. Unknown counts as "not elevated".
        // SAFETY: GetCurrentProcess returns a pseudo handle that needs no closing.
        let own = integrity_level(unsafe { GetCurrentProcess() });
        match (foreground_integrity_level(), own) {
            (Some(target), Some(own)) => target > own,
            _ => false,
        }
    }

    fn send(&mut self, events: &[KeyEvent]) -> Result<(), SendError> {
        let inputs: Vec<INPUT> = events.iter().flat_map(|event| inputs_for(*event)).collect();
        if inputs.is_empty() {
            return Ok(());
        }
        // SAFETY: `inputs` is a valid slice of keyboard INPUTs of the size we pass.
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize == inputs.len() {
            return Ok(());
        }
        // SAFETY: GetLastError has no preconditions.
        let error = unsafe { GetLastError() };
        if error == ERROR_ACCESS_DENIED {
            Err(SendError::Blocked)
        } else {
            Err(SendError::Failed(format!(
                "Windows accepted {sent} of {} keystrokes (error {})",
                inputs.len(),
                error.0
            )))
        }
    }
}

fn inputs_for(event: KeyEvent) -> Vec<INPUT> {
    match event {
        KeyEvent::Down(key) => vec![virtual_key(key, KEYBD_EVENT_FLAGS(0))],
        KeyEvent::Up(key) => vec![virtual_key(key, KEYEVENTF_KEYUP)],
        KeyEvent::Unit(unit) => vec![
            keyboard_input(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE),
            keyboard_input(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
        ],
    }
}

fn virtual_key(key: Key, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    let vk = match key {
        Key::Control => VK_CONTROL,
        Key::V => VIRTUAL_KEY(u16::from(b'V')),
        Key::Return => VK_RETURN,
        Key::Tab => VK_TAB,
    };
    // Some applications look at the scan code rather than the virtual key.
    // SAFETY: MapVirtualKeyW has no preconditions.
    let scan = unsafe { MapVirtualKeyW(u32::from(vk.0), MAPVK_VK_TO_VSC) } as u16;
    keyboard_input(vk, scan, flags)
}

fn keyboard_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                // So the Record Shortcut hook ignores Echo's own keystrokes.
                dwExtraInfo: ECHO_INPUT_TAG,
            },
        },
    }
}

fn foreground_integrity_level() -> Option<u32> {
    // SAFETY: plain queries; the process handle is closed before returning.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let level = integrity_level(process);
        let _ = CloseHandle(process);
        level
    }
}

/// The mandatory integrity level of `process` (e.g. 0x2000 medium, 0x3000 high = elevated).
fn integrity_level(process: HANDLE) -> Option<u32> {
    // SAFETY: the token handle is closed before returning; the buffer is 8-byte aligned, sized
    // as Windows asked, and read as the TOKEN_MANDATORY_LABEL it was filled with.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut size = 0;
        let _ = GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut size);
        let mut buffer = vec![0u64; (size as usize).div_ceil(8).max(1)];
        let filled = GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            size,
            &mut size,
        );
        let level = filled.ok().and_then(|()| {
            let label = &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
            let count = *GetSidSubAuthorityCount(label.Label.Sid);
            (count > 0).then(|| *GetSidSubAuthority(label.Label.Sid, u32::from(count) - 1))
        });
        let _ = CloseHandle(token);
        level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_runs_at_a_known_integrity_level() {
        // SAFETY: pseudo handle.
        let level = integrity_level(unsafe { GetCurrentProcess() }).expect("own token readable");
        assert!(level >= 0x1000, "integrity level {level:#x}");
    }

    #[test]
    fn every_injected_keystroke_carries_echo_s_tag() {
        let events = [
            KeyEvent::Down(Key::Control),
            KeyEvent::Unit(0x17C),
            KeyEvent::Up(Key::Return),
        ];
        let inputs: Vec<INPUT> = events.iter().flat_map(|e| inputs_for(*e)).collect();

        assert_eq!(inputs.len(), 4);
        for input in &inputs {
            // SAFETY: every INPUT built here is a keyboard input.
            assert_eq!(unsafe { input.Anonymous.ki.dwExtraInfo }, ECHO_INPUT_TAG);
        }
        // SAFETY: as above.
        let unicode = unsafe { inputs[1].Anonymous.ki };
        assert_eq!(unicode.wScan, 0x17C);
        assert_eq!(unicode.dwFlags, KEYEVENTF_UNICODE);
    }
}
