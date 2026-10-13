//! The Selection Reader's seams on Windows: the Inserter's clipboard and keyboard, and the
//! focused window's class.

use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow};

use super::selection::{
    Copied, EDITOR_DATA_FORMAT, ForegroundWindow, SelectionClipboard, is_from_empty_selection,
    is_terminal_class,
};
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

    fn copied(&self) -> Result<Copied, String> {
        let read = WindowsClipboard::text_with(self, EDITOR_DATA_FORMAT)?;
        Ok(Copied {
            text: read.text,
            from_empty_selection: read
                .named
                .is_some_and(|data| is_from_empty_selection(&decode(&data))),
            sequence: read.sequence,
        })
    }

    fn restore_if_sequence(&self, saved: SavedClipboard, sequence: u32) -> Result<Restore, String> {
        WindowsClipboard::restore_if_sequence(self, saved, sequence)
    }
}

/// The editor data as text. Applications write custom formats as they like; this takes UTF-8,
/// or UTF-16 when the second byte is NUL (ASCII JSON written as UTF-16 starts with `{`, 0).
fn decode(data: &[u8]) -> String {
    if data.get(1) == Some(&0) {
        let units: Vec<u16> = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(data).into_owned()
    }
}

/// The real focused window.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsForegroundWindow;

impl ForegroundWindow for WindowsForegroundWindow {
    fn is_terminal(&self) -> bool {
        self.class().is_some_and(|class| is_terminal_class(&class))
    }

    fn class(&self) -> Option<String> {
        let mut name = [0u16; 256];
        // SAFETY: plain queries; GetClassNameW writes at most `name.len()` units.
        let length = unsafe { GetClassNameW(GetForegroundWindow(), &mut name) };
        let length = usize::try_from(length).ok().filter(|&l| l > 0)?;
        Some(String::from_utf16_lossy(&name[..length.min(name.len())]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_data_is_read_as_utf8_or_utf16() {
        let json = r#"{"isFromEmptySelection":true}"#;
        assert_eq!(decode(json.as_bytes()), json);
        let utf16: Vec<u8> = json
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        assert!(is_from_empty_selection(&decode(&utf16)));
    }
}
