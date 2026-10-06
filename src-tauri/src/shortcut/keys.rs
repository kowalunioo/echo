//! The keys a shortcut can use: their canonical names (as stored and shown, e.g. `"Space"`,
//! `"F9"`, `"RightAlt"`) and the Windows virtual-key codes the keyboard hook sees.
//!
//! Names are layout-independent: punctuation keys are named after their position on a US
//! keyboard (`"Minus"`, `"BracketLeft"`), so a combination never depends on the active keyboard
//! layout and no name contains `+`, the separator of the text form.

/// Windows virtual-key codes of the modifier keys (side-specific, as a low-level hook reports
/// them) and of the generic variants some sources send instead.
pub mod vk {
    pub const SHIFT: u16 = 0x10;
    pub const CONTROL: u16 = 0x11;
    pub const MENU: u16 = 0x12;
    pub const LSHIFT: u16 = 0xA0;
    pub const RSHIFT: u16 = 0xA1;
    pub const LCONTROL: u16 = 0xA2;
    pub const RCONTROL: u16 = 0xA3;
    pub const LMENU: u16 = 0xA4;
    pub const RMENU: u16 = 0xA5;
    pub const LWIN: u16 = 0x5B;
    pub const RWIN: u16 = 0x5C;
    pub const ESCAPE: u16 = 0x1B;
}

/// The modifier a key counts as, if it is one. Left and right are equivalent
/// (`record-shortcut.md` rule 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKind {
    Ctrl,
    Alt,
    Shift,
    Win,
}

/// The modifier `vk` counts as, if any.
pub fn modifier_kind(vk: u16) -> Option<ModifierKind> {
    match vk {
        vk::CONTROL | vk::LCONTROL | vk::RCONTROL => Some(ModifierKind::Ctrl),
        vk::MENU | vk::LMENU | vk::RMENU => Some(ModifierKind::Alt),
        vk::SHIFT | vk::LSHIFT | vk::RSHIFT => Some(ModifierKind::Shift),
        vk::LWIN | vk::RWIN => Some(ModifierKind::Win),
        _ => None,
    }
}

/// Side-specific names of the modifier keys, as the shortcut-capture UI receives them.
fn modifier_name(vk: u16) -> Option<&'static str> {
    Some(match vk {
        vk::CONTROL | vk::LCONTROL => "LeftCtrl",
        vk::RCONTROL => "RightCtrl",
        vk::MENU | vk::LMENU => "LeftAlt",
        vk::RMENU => "RightAlt",
        vk::SHIFT | vk::LSHIFT => "LeftShift",
        vk::RSHIFT => "RightShift",
        vk::LWIN => "LeftWin",
        vk::RWIN => "RightWin",
        _ => return None,
    })
}

/// Named keys other than letters, digits and F-keys.
const NAMED: &[(&str, u16)] = &[
    ("Space", 0x20),
    ("Enter", 0x0D),
    ("Tab", 0x09),
    ("Backspace", 0x08),
    ("Escape", vk::ESCAPE),
    ("Insert", 0x2D),
    ("Delete", 0x2E),
    ("Home", 0x24),
    ("End", 0x23),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
    ("Left", 0x25),
    ("Up", 0x26),
    ("Right", 0x27),
    ("Down", 0x28),
    ("Pause", 0x13),
    ("ScrollLock", 0x91),
    ("CapsLock", 0x14),
    ("NumLock", 0x90),
    ("PrintScreen", 0x2C),
    ("ContextMenu", 0x5D),
    ("NumpadMultiply", 0x6A),
    ("NumpadAdd", 0x6B),
    ("NumpadSubtract", 0x6D),
    ("NumpadDecimal", 0x6E),
    ("NumpadDivide", 0x6F),
    ("Semicolon", 0xBA),
    ("Equal", 0xBB),
    ("Comma", 0xBC),
    ("Minus", 0xBD),
    ("Period", 0xBE),
    ("Slash", 0xBF),
    ("Backquote", 0xC0),
    ("BracketLeft", 0xDB),
    ("Backslash", 0xDC),
    ("BracketRight", 0xDD),
    ("Quote", 0xDE),
    ("IntlBackslash", 0xE2),
    // Side-specific modifiers used alone as the whole shortcut (rule 19).
    ("RightCtrl", vk::RCONTROL),
    ("RightAlt", vk::RMENU),
];

/// The virtual-key code of the main key named `name`, if it is a known key name.
pub fn vk_of(name: &str) -> Option<u16> {
    if let Some(&(_, code)) = NAMED.iter().find(|(n, _)| *n == name) {
        return Some(code);
    }
    if let [c] = name.as_bytes()
        && (c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Some(u16::from(*c)); // VK codes of A–Z and 0–9 are their ASCII codes.
    }
    if let Some(n) = name.strip_prefix("Numpad").and_then(plain_number)
        && n <= 9
    {
        return Some(0x60 + n);
    }
    if let Some(n) = name.strip_prefix('F').and_then(plain_number)
        && (1..=24).contains(&n)
    {
        return Some(0x70 + n - 1);
    }
    None
}

/// A decimal number written without sign or leading zeros.
fn plain_number(text: &str) -> Option<u16> {
    let canonical = text.bytes().all(|b| b.is_ascii_digit())
        && !text.is_empty()
        && (text == "0" || !text.starts_with('0'));
    canonical.then(|| text.parse().ok()).flatten()
}

/// The canonical name of the main key with virtual-key code `vk`, if it is one Echo knows.
/// Modifier keys have no main-key name except right Ctrl and right Alt.
pub fn main_key_name(vk: u16) -> Option<String> {
    if let Some(&(name, _)) = NAMED.iter().find(|(_, code)| *code == vk) {
        return Some(name.to_owned());
    }
    match vk {
        0x30..=0x39 | 0x41..=0x5A => Some(char::from(vk as u8).to_string()),
        0x60..=0x69 => Some(format!("Numpad{}", vk - 0x60)),
        0x70..=0x87 => Some(format!("F{}", vk - 0x70 + 1)),
        _ => None,
    }
}

/// The name the shortcut-capture UI receives for `vk`: side-specific for modifiers
/// (`"LeftCtrl"`, `"RightAlt"`), the canonical main-key name otherwise.
pub fn capture_name(vk: u16) -> Option<String> {
    modifier_name(vk)
        .map(str::to_owned)
        .or_else(|| main_key_name(vk))
}

/// The virtual-key code of a capture name (the reverse of [`capture_name`]).
pub fn vk_of_capture_name(name: &str) -> Option<u16> {
    Some(match name {
        "LeftCtrl" => vk::LCONTROL,
        "RightCtrl" => vk::RCONTROL,
        "LeftAlt" => vk::LMENU,
        "RightAlt" => vk::RMENU,
        "LeftShift" => vk::LSHIFT,
        "RightShift" => vk::RSHIFT,
        "LeftWin" => vk::LWIN,
        "RightWin" => vk::RWIN,
        main => return vk_of(main),
    })
}

/// Whether `name` may be the whole shortcut on its own, without modifiers (rule 19): an F-key,
/// Pause, Scroll Lock, Insert, or right Ctrl / right Alt.
pub fn usable_alone(name: &str) -> bool {
    matches!(
        name,
        "Pause" | "ScrollLock" | "Insert" | "RightCtrl" | "RightAlt"
    ) || (name.starts_with('F') && name.len() > 1 && vk_of(name).is_some())
}

/// Whether `name` is right Ctrl or right Alt, which are modifiers that may only be used alone.
pub fn is_side_modifier(name: &str) -> bool {
    matches!(name, "RightCtrl" | "RightAlt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_codes_round_trip() {
        for code in 0u16..=0xFF {
            if let Some(name) = main_key_name(code) {
                assert_eq!(vk_of(&name), Some(code), "{name}");
            }
        }
        assert_eq!(vk_of("Space"), Some(0x20));
        assert_eq!(vk_of("D"), Some(0x44));
        assert_eq!(vk_of("7"), Some(0x37));
        assert_eq!(vk_of("F1"), Some(0x70));
        assert_eq!(vk_of("F24"), Some(0x87));
        assert_eq!(vk_of("Numpad5"), Some(0x65));
    }

    #[test]
    fn unknown_names_have_no_code() {
        for name in [
            "", "d", "F0", "F25", "F01", "F+5", "Numpad05", "Ctrl", "LeftCtrl", "Spacebar", "+",
        ] {
            assert_eq!(vk_of(name), None, "{name}");
        }
    }

    #[test]
    fn modifiers_are_side_specific_in_capture_and_equivalent_as_kinds() {
        assert_eq!(capture_name(vk::LCONTROL).as_deref(), Some("LeftCtrl"));
        assert_eq!(capture_name(vk::RMENU).as_deref(), Some("RightAlt"));
        assert_eq!(capture_name(0x20).as_deref(), Some("Space"));
        assert_eq!(modifier_kind(vk::LCONTROL), modifier_kind(vk::RCONTROL));
        assert_eq!(modifier_kind(0x20), None);
    }

    #[test]
    fn capture_names_round_trip() {
        for code in 0u16..=0xFF {
            if let Some(name) = capture_name(code) {
                let back = vk_of_capture_name(&name).unwrap();
                assert_eq!(capture_name(back), Some(name));
            }
        }
        assert_eq!(vk_of_capture_name("LeftWin"), Some(vk::LWIN));
        assert_eq!(vk_of_capture_name("Space"), Some(0x20));
        assert_eq!(vk_of_capture_name("Nope"), None);
    }

    #[test]
    fn keys_usable_alone() {
        for name in [
            "F1",
            "F9",
            "F24",
            "Pause",
            "ScrollLock",
            "Insert",
            "RightAlt",
        ] {
            assert!(usable_alone(name), "{name}");
        }
        for name in ["Space", "A", "1", "Escape", "Enter", "Home", "F"] {
            assert!(!usable_alone(name), "{name}");
        }
    }
}
