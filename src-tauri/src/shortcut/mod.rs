//! The **ShortcutListener** seam: reports presses and releases of the Record Shortcut and the
//! Cancel Shortcut from anywhere in Windows.
//!
//! The real listener is our own low-level keyboard hook (ADR 0002) and arrives in a later slice.
//! [`FakeShortcutListener`] injects events in tests.

mod fake;

use std::fmt;

pub use fake::FakeShortcutListener;

/// The global shortcuts Echo listens for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shortcut {
    /// Starts and stops a Recording (`docs/specs/record-shortcut.md`).
    Record,
    /// Triggers Cancellation of the current Dictation (`docs/specs/cancel-shortcut.md`).
    Cancel,
}

/// Whether a shortcut went down or up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Pressed,
    Released,
}

/// One press or release of a bound shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutEvent {
    pub shortcut: Shortcut,
    pub action: KeyAction,
}

/// The modifier keys of a [`KeyCombination`]. Left and right variants are equivalent
/// (`record-shortcut.md` rule 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// A key that is not one of the [`Modifiers`], by its canonical name: `"Space"`, `"D"`, `"F5"`,
/// `"Escape"`, or a side-specific modifier used as a key on its own (`"RightAlt"`,
/// `"RightCtrl"`). The Record Shortcut slice defines the full list and validation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key(pub String);

/// A key combination as the user sets it: modifiers and at most one main key. Which
/// combinations are allowed is decided by the Record Shortcut and Cancel Shortcut slices, not by
/// the listener.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct KeyCombination {
    pub modifiers: Modifiers,
    pub key: Option<Key>,
}

/// Canonical text form: Ctrl, Alt, Shift, Win, then the main key, joined by `+`
/// (`record-shortcut.md` rule 21), e.g. `Ctrl+Space`.
impl fmt::Display for KeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Modifiers {
            ctrl,
            alt,
            shift,
            win,
        } = self.modifiers;
        let modifiers = [(ctrl, "Ctrl"), (alt, "Alt"), (shift, "Shift"), (win, "Win")];
        let parts: Vec<&str> = modifiers
            .iter()
            .filter(|(held, _)| *held)
            .map(|(_, name)| *name)
            .chain(self.key.as_ref().map(|k| k.0.as_str()))
            .collect();
        f.write_str(&parts.join("+"))
    }
}

/// Why listening or binding failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShortcutError {
    /// The listener could not start, e.g. the keyboard hook could not be installed.
    #[error("cannot listen for shortcuts: {0}")]
    Unavailable(String),
    /// This combination cannot be activated (`record-shortcut.md` rule 23).
    #[error("cannot use {combination}: {reason}")]
    Rejected { combination: String, reason: String },
}

/// Receives [`ShortcutEvent`]s on the listener's thread (for the keyboard hook: the hook
/// thread, which Windows gives very little time), so it must return quickly — typically it
/// forwards the event into a channel.
pub type ShortcutSink = Box<dyn FnMut(ShortcutEvent) + Send>;

/// Reports presses and releases of the bound shortcuts from anywhere in Windows.
///
/// # Contract
///
/// - [`start`](ShortcutListener::start) is called once; events flow to the sink until the
///   listener is dropped.
/// - [`bind`](ShortcutListener::bind) sets (`Some`) or clears (`None`) the combination of one
///   [`Shortcut`] and takes effect immediately, before or after `start`. If it fails, the
///   previous binding stays active and unchanged (`record-shortcut.md` rule 23). Clearing is how
///   the Cancel Shortcut is active only during a Dictation and how the shortcut-capture UI
///   suspends the Record Shortcut (rule 14).
/// - A press is reported when exactly the bound keys are down (rule 7), a release when the
///   combination stops being held. Key auto-repeat never produces another press (rule 9).
///   Debounce and the Push-to-Talk release grace (rules 10–11) depend on the mode and are applied
///   by the consumer, so they are testable with the fake.
/// - While a bound combination is held, its keystrokes are swallowed; all other keystrokes pass
///   through unchanged (rule 12). Unbound combinations are never swallowed.
pub trait ShortcutListener: Send {
    /// Starts delivering events of bound shortcuts to `sink`.
    fn start(&mut self, sink: ShortcutSink) -> Result<(), ShortcutError>;

    /// Sets or clears the key combination of `shortcut`.
    fn bind(
        &mut self,
        shortcut: Shortcut,
        combination: Option<KeyCombination>,
    ) -> Result<(), ShortcutError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combination(ctrl: bool, alt: bool, shift: bool, win: bool, key: Option<&str>) -> String {
        KeyCombination {
            modifiers: Modifiers {
                ctrl,
                alt,
                shift,
                win,
            },
            key: key.map(|k| Key(k.into())),
        }
        .to_string()
    }

    #[test]
    fn displays_in_canonical_order() {
        assert_eq!(
            combination(true, false, false, false, Some("Space")),
            "Ctrl+Space"
        );
        assert_eq!(
            combination(true, true, true, true, Some("D")),
            "Ctrl+Alt+Shift+Win+D"
        );
        assert_eq!(combination(true, false, false, true, None), "Ctrl+Win");
        assert_eq!(
            combination(false, false, false, false, Some("RightAlt")),
            "RightAlt"
        );
    }
}
