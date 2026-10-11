//! The Selection Reader's seams on Windows: the Inserter's clipboard and keyboard, and the
//! focused window's class.

use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow};

use super::selection::{ForegroundWindow, SelectionClipboard, is_terminal_class};
use crate::insertion::paste::Restore;
use crate::insertion::{SavedClipboard, WindowsClipboard};

impl SelectionClipboard for WindowsClipboard {
    type Saved = SavedClipboard;

    fn save(&self) -> Result<SavedClipboard, String> {
        crate::insertion::paste::Clipboard::save(self)
    }

    fn sequence(&self) -> u32 {
        WindowsClipboard::sequence(self)
    }

    fn text(&self) -> Result<(Option<String>, u32), String> {
        WindowsClipboard::text(self)
    }

    fn restore_if_sequence(&self, saved: SavedClipboard, sequence: u32) -> Result<Restore, String> {
        WindowsClipboard::restore_if_sequence(self, saved, sequence)
    }
}

/// The real focused window.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsForegroundWindow;

impl ForegroundWindow for WindowsForegroundWindow {
    fn is_terminal(&self) -> bool {
        let mut name = [0u16; 256];
        // SAFETY: plain queries; GetClassNameW writes at most `name.len()` units.
        let length = unsafe { GetClassNameW(GetForegroundWindow(), &mut name) };
        let Ok(length) = usize::try_from(length) else {
            return false;
        };
        is_terminal_class(&String::from_utf16_lossy(&name[..length.min(name.len())]))
    }
}
