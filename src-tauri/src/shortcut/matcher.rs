//! The decision logic of the keyboard hook, free of Windows calls so it can be tested with
//! synthetic key events: which keystrokes make a bound shortcut pressed or released, which are
//! swallowed, and what the shortcut-capture UI receives.
//!
//! Rules (`record-shortcut.md` rules 7–9, 12–14):
//!
//! - A shortcut is pressed when exactly its keys are down (extra modifiers or keys mean it is
//!   not), with left and right modifiers equivalent. Keys that belong to another shortcut that is
//!   already held do not count as extra, so the Cancel Shortcut works while the Record Shortcut
//!   is held.
//! - Only a fresh key-down can press a shortcut, never auto-repeat or a key-up. For a combination
//!   with a main key, the main key must be the last key pressed (pressing Space before Ctrl types
//!   a space and is not Ctrl+Space); modifier-only combinations may be pressed in any order.
//! - The key-down that completes the combination is swallowed, and so are its repeats and its
//!   release. Keys pressed before it already reached the focused application, so their releases
//!   pass through too: applications always see balanced down/up pairs and no key can get stuck
//!   in them. When such an already-delivered key is Alt or Win, the hook asks for a masking
//!   keystroke so releasing it does not open the Start menu or a menu bar (rule 13).
//! - A shortcut is released when any of its keys goes up.
//! - Missed key-ups (Windows drops hook events in some situations, e.g. on the secure desktop)
//!   would leave a key "down" forever and block every exact match. Before acting on a fresh
//!   key-down the matcher therefore drops keys the system no longer reports as down. Swallowed
//!   keys cannot be checked that way (the system never saw them go down); they recover on their
//!   next press and release.

use super::keys::{self, ModifierKind, vk};
use super::{KeyAction, KeyCombination, Modifiers, Shortcut, ShortcutEvent};

/// The scan code Windows gives the left Ctrl event it synthesises when AltGr (right Alt on
/// layouts such as Polish) is pressed. That event is not a real Ctrl press and is ignored.
const ALTGR_FAKE_CTRL_SCAN: u32 = 0x21D;

/// One keystroke as the low-level hook reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawKey {
    pub vk: u16,
    pub scan: u32,
    pub up: bool,
    /// Injected by Echo itself (the masking keystroke, later the Inserter): never matched.
    pub from_echo: bool,
}

/// A key the shortcut-capture UI receives, by its capture name (`keys::capture_name`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedKey {
    pub key: String,
    pub action: KeyAction,
}

/// Something the hook must report after a keystroke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Shortcut(ShortcutEvent),
    Captured(CapturedKey),
}

/// What the hook does with the keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Verdict {
    /// Keep the keystroke from the focused application.
    pub swallow: bool,
    /// Inject a harmless keystroke so a delivered Alt or Win press is not followed by a bare
    /// release (rule 13).
    pub mask: bool,
}

/// What the matcher may ask the system about.
pub trait KeyboardState {
    /// Whether the system currently reports `vk` as down.
    fn is_down(&self, vk: u16) -> bool;
    /// Whether one of Echo's windows is in the foreground (capture only acts then).
    fn echo_in_foreground(&self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Binding {
    modifiers: Modifiers,
    main: Option<u16>,
}

#[derive(Debug, Clone)]
struct Active {
    /// The keys that were down when the shortcut was pressed; releasing any releases it.
    owned: Vec<u16>,
}

/// The state of the keyboard as the hook has seen it, and the bound shortcuts.
pub struct Matcher {
    down: [bool; 256],
    swallowed: [bool; 256],
    bindings: [Option<Binding>; 2],
    active: [Option<Active>; 2],
    capturing: bool,
}

const SHORTCUTS: [Shortcut; 2] = [Shortcut::Record, Shortcut::Cancel];

fn slot(shortcut: Shortcut) -> usize {
    match shortcut {
        Shortcut::Record => 0,
        Shortcut::Cancel => 1,
    }
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Matcher {
    pub const fn new() -> Self {
        Self {
            down: [false; 256],
            swallowed: [false; 256],
            bindings: [None, None],
            active: [None, None],
            capturing: false,
        }
    }

    /// Sets or clears the combination of `shortcut`. Fails, changing nothing, when the
    /// combination names a key the hook cannot recognise.
    pub fn bind(
        &mut self,
        shortcut: Shortcut,
        combination: Option<&KeyCombination>,
    ) -> Result<(), String> {
        let binding = match combination {
            None => None,
            Some(c) => {
                let main = match &c.key {
                    None => None,
                    Some(key) => {
                        Some(keys::vk_of(&key.0).ok_or_else(|| format!("unknown key {}", key.0))?)
                    }
                };
                Some(Binding {
                    modifiers: c.modifiers,
                    main,
                })
            }
        };
        self.bindings[slot(shortcut)] = binding;
        self.active[slot(shortcut)] = None;
        Ok(())
    }

    /// Turns capture mode on or off. While it is on and Echo is in the foreground, every
    /// keystroke goes to the capture UI instead of applications, and no shortcut is matched.
    pub fn set_capturing(&mut self, capturing: bool) {
        self.capturing = capturing;
    }

    pub fn is_capturing(&self) -> bool {
        self.capturing
    }

    /// Processes one keystroke, reporting events through `out`.
    pub fn handle(
        &mut self,
        key: RawKey,
        system: &impl KeyboardState,
        out: &mut impl FnMut(Output),
    ) -> Verdict {
        let capture = |m: &Self| m.capturing && system.echo_in_foreground();
        if key.from_echo || key.vk == 0 || key.vk > 0xFF {
            return Verdict::default();
        }
        if key.vk == vk::LCONTROL && key.scan == ALTGR_FAKE_CTRL_SCAN {
            return Verdict {
                swallow: capture(self),
                mask: false,
            };
        }
        let code = usize::from(key.vk);

        if key.up {
            let was_down = std::mem::replace(&mut self.down[code], false);
            let swallow = std::mem::replace(&mut self.swallowed[code], false);
            if swallow && capture(self) && was_down {
                report_capture(key.vk, KeyAction::Released, out);
            }
            self.release_owners_of(key.vk, out);
            return Verdict {
                swallow,
                mask: false,
            };
        }

        if std::mem::replace(&mut self.down[code], true) {
            // Auto-repeat never presses anything (rule 9); it follows its first key-down.
            return Verdict {
                swallow: self.swallowed[code],
                mask: false,
            };
        }

        self.forget_missed_releases(key.vk, system, out);

        if capture(self) {
            self.swallowed[code] = true;
            report_capture(key.vk, KeyAction::Pressed, out);
            return Verdict {
                swallow: true,
                mask: false,
            };
        }

        for shortcut in SHORTCUTS {
            let i = slot(shortcut);
            if self.active[i].is_some() {
                continue;
            }
            let Some(binding) = &self.bindings[i] else {
                continue;
            };
            let free = self.free_keys();
            if !completes(binding, key.vk, &free) {
                continue;
            }
            self.swallowed[code] = true;
            let mask = free.iter().any(|&k| {
                !self.swallowed[usize::from(k)]
                    && matches!(
                        keys::modifier_kind(k),
                        Some(ModifierKind::Alt | ModifierKind::Win)
                    )
            });
            self.active[i] = Some(Active { owned: free });
            out(Output::Shortcut(ShortcutEvent {
                shortcut,
                action: KeyAction::Pressed,
            }));
            return Verdict {
                swallow: true,
                mask,
            };
        }
        Verdict::default()
    }

    /// Keys that are down and not part of a shortcut that is already held.
    fn free_keys(&self) -> Vec<u16> {
        (0u16..=0xFF)
            .filter(|&k| self.down[usize::from(k)])
            .filter(|k| !self.active.iter().flatten().any(|a| a.owned.contains(k)))
            .collect()
    }

    fn release_owners_of(&mut self, key: u16, out: &mut impl FnMut(Output)) {
        for shortcut in SHORTCUTS {
            let i = slot(shortcut);
            if self.active[i]
                .as_ref()
                .is_some_and(|a| a.owned.contains(&key))
            {
                self.active[i] = None;
                out(Output::Shortcut(ShortcutEvent {
                    shortcut,
                    action: KeyAction::Released,
                }));
            }
        }
    }

    fn forget_missed_releases(
        &mut self,
        current: u16,
        system: &impl KeyboardState,
        out: &mut impl FnMut(Output),
    ) {
        for k in 0u16..=0xFF {
            let i = usize::from(k);
            if k != current && self.down[i] && !self.swallowed[i] && !system.is_down(k) {
                self.down[i] = false;
                self.release_owners_of(k, out);
            }
        }
    }
}

/// Whether pressing `pressed` makes exactly `binding` held, given the free keys now down
/// (including `pressed`).
fn completes(binding: &Binding, pressed: u16, free: &[u16]) -> bool {
    match binding.main {
        Some(main) if pressed != main => return false,
        None if keys::modifier_kind(pressed).is_none() => return false,
        _ => {}
    }
    let mut held = Modifiers::default();
    for &k in free {
        if Some(k) == binding.main {
            continue;
        }
        match keys::modifier_kind(k) {
            Some(ModifierKind::Ctrl) => held.ctrl = true,
            Some(ModifierKind::Alt) => held.alt = true,
            Some(ModifierKind::Shift) => held.shift = true,
            Some(ModifierKind::Win) => held.win = true,
            None => return false, // another, non-modifier key is down
        }
    }
    held == binding.modifiers
}

fn report_capture(vk: u16, action: KeyAction, out: &mut impl FnMut(Output)) {
    if let Some(key) = keys::capture_name(vk) {
        out(Output::Captured(CapturedKey { key, action }));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;

    use super::*;

    const SPACE: u16 = 0x20;
    const KEY_A: u16 = 0x41;
    const KEY_D: u16 = 0x44;

    /// The system's view of the keyboard: keys the matcher let through are down, unless a test
    /// says a release was lost.
    #[derive(Default)]
    struct System {
        down: RefCell<HashSet<u16>>,
        foreground: bool,
    }

    impl KeyboardState for System {
        fn is_down(&self, vk: u16) -> bool {
            self.down.borrow().contains(&vk)
        }
        fn echo_in_foreground(&self) -> bool {
            self.foreground
        }
    }

    struct Hook {
        matcher: Matcher,
        system: System,
        events: Vec<Output>,
    }

    impl Hook {
        fn with(record: &str) -> Self {
            let mut matcher = Matcher::new();
            matcher
                .bind(Shortcut::Record, Some(&record.parse().unwrap()))
                .unwrap();
            Self {
                matcher,
                system: System::default(),
                events: Vec::new(),
            }
        }

        fn key(&mut self, vk: u16, up: bool, scan: u32) -> Verdict {
            let raw = RawKey {
                vk,
                scan,
                up,
                from_echo: false,
            };
            let mut events = Vec::new();
            let verdict = self
                .matcher
                .handle(raw, &self.system, &mut |o| events.push(o));
            if !verdict.swallow {
                if up {
                    self.system.down.borrow_mut().remove(&vk);
                } else {
                    self.system.down.borrow_mut().insert(vk);
                }
            }
            self.events.extend(events);
            verdict
        }

        fn down(&mut self, vk: u16) -> Verdict {
            self.key(vk, false, 0)
        }

        fn up(&mut self, vk: u16) -> Verdict {
            self.key(vk, true, 0)
        }

        fn shortcut_events(&mut self) -> Vec<(Shortcut, KeyAction)> {
            self.events
                .drain(..)
                .filter_map(|o| match o {
                    Output::Shortcut(e) => Some((e.shortcut, e.action)),
                    Output::Captured(_) => None,
                })
                .collect()
        }
    }

    use KeyAction::{Pressed, Released};
    use Shortcut::{Cancel, Record};

    fn swallowed() -> Verdict {
        Verdict {
            swallow: true,
            mask: false,
        }
    }

    #[test]
    fn ctrl_space_presses_and_releases_the_record_shortcut() {
        let mut hook = Hook::with("Ctrl+Space");

        assert_eq!(hook.down(vk::LCONTROL), Verdict::default());
        assert_eq!(hook.down(SPACE), swallowed());
        assert_eq!(hook.up(SPACE), swallowed());
        assert_eq!(hook.up(vk::LCONTROL), Verdict::default());

        assert_eq!(
            hook.shortcut_events(),
            vec![(Record, Pressed), (Record, Released)]
        );
    }

    /// Acceptance test 7.
    #[test]
    fn right_ctrl_counts_as_ctrl() {
        let mut hook = Hook::with("Ctrl+Space");

        hook.down(vk::RCONTROL);
        hook.down(SPACE);

        assert_eq!(hook.shortcut_events(), vec![(Record, Pressed)]);
    }

    /// Acceptance test 6.
    #[test]
    fn extra_modifiers_mean_it_is_not_the_shortcut() {
        let mut hook = Hook::with("Ctrl+Space");

        hook.down(vk::LCONTROL);
        hook.down(vk::LSHIFT);
        assert_eq!(hook.down(SPACE), Verdict::default());
        hook.up(SPACE);

        assert_eq!(hook.shortcut_events(), vec![]);
    }

    #[test]
    fn missing_modifiers_or_extra_keys_mean_it_is_not_the_shortcut() {
        let mut hook = Hook::with("Ctrl+Alt+D");

        hook.down(vk::LCONTROL);
        assert_eq!(hook.down(KEY_D), Verdict::default());
        hook.up(KEY_D);
        hook.down(vk::LMENU);
        hook.down(KEY_A);
        assert_eq!(hook.down(KEY_D), Verdict::default());

        assert_eq!(hook.shortcut_events(), vec![]);
    }

    /// Acceptance test 5.
    #[test]
    fn auto_repeat_never_presses_again_and_stays_swallowed() {
        let mut hook = Hook::with("Ctrl+Space");

        hook.down(vk::LCONTROL);
        hook.down(SPACE);
        for _ in 0..20 {
            assert_eq!(hook.down(SPACE), swallowed());
            hook.down(vk::LCONTROL);
        }
        hook.up(SPACE);

        assert_eq!(
            hook.shortcut_events(),
            vec![(Record, Pressed), (Record, Released)]
        );
    }

    #[test]
    fn the_main_key_must_be_pressed_last() {
        let mut hook = Hook::with("Ctrl+Space");

        assert_eq!(hook.down(SPACE), Verdict::default());
        assert_eq!(hook.down(vk::LCONTROL), Verdict::default());

        assert_eq!(hook.shortcut_events(), vec![]);
    }

    #[test]
    fn releasing_a_modifier_first_releases_the_shortcut_and_the_main_key_stays_swallowed() {
        let mut hook = Hook::with("Ctrl+Space");

        hook.down(vk::LCONTROL);
        hook.down(SPACE);
        assert_eq!(hook.up(vk::LCONTROL), Verdict::default());
        assert_eq!(hook.down(SPACE), swallowed());
        assert_eq!(hook.up(SPACE), swallowed());

        assert_eq!(
            hook.shortcut_events(),
            vec![(Record, Pressed), (Record, Released)]
        );
    }

    #[test]
    fn other_keys_pass_through_unchanged() {
        let mut hook = Hook::with("Ctrl+Space");

        for vk in [KEY_A, 0x42, 0x43] {
            assert_eq!(hook.down(vk), Verdict::default());
            assert_eq!(hook.up(vk), Verdict::default());
        }
        hook.down(SPACE);
        assert_eq!(hook.up(SPACE), Verdict::default());

        assert_eq!(hook.shortcut_events(), vec![]);
    }

    #[test]
    fn modifier_only_combinations_work_in_any_order_and_swallow_the_completing_key() {
        let mut hook = Hook::with("Ctrl+Win");

        hook.down(vk::LCONTROL);
        assert_eq!(hook.down(vk::LWIN), swallowed());
        assert_eq!(hook.up(vk::LWIN), swallowed());
        hook.up(vk::LCONTROL);

        // Win first: Win already reached Windows, so its bare release must be masked.
        hook.down(vk::RWIN);
        assert_eq!(
            hook.down(vk::RCONTROL),
            Verdict {
                swallow: true,
                mask: true
            }
        );
        hook.up(vk::RCONTROL);
        hook.up(vk::RWIN);

        assert_eq!(
            hook.shortcut_events(),
            vec![
                (Record, Pressed),
                (Record, Released),
                (Record, Pressed),
                (Record, Released)
            ]
        );
    }

    #[test]
    fn alt_combinations_ask_for_a_mask_so_the_menu_bar_does_not_open() {
        let mut hook = Hook::with("Ctrl+Alt+D");

        hook.down(vk::LCONTROL);
        hook.down(vk::LMENU);
        assert_eq!(
            hook.down(KEY_D),
            Verdict {
                swallow: true,
                mask: true
            }
        );
    }

    #[test]
    fn modifier_only_combinations_do_not_trigger_with_a_main_key_down() {
        let mut hook = Hook::with("Ctrl+Shift");

        hook.down(KEY_A);
        hook.down(vk::LCONTROL);
        assert_eq!(hook.down(vk::LSHIFT), Verdict::default());

        assert_eq!(hook.shortcut_events(), vec![]);
    }

    #[test]
    fn right_alt_alone_ignores_the_ctrl_that_altgr_synthesises() {
        let mut hook = Hook::with("RightAlt");

        assert_eq!(
            hook.key(vk::LCONTROL, false, ALTGR_FAKE_CTRL_SCAN),
            Verdict::default()
        );
        assert_eq!(hook.down(vk::RMENU), swallowed());
        assert_eq!(hook.up(vk::RMENU), swallowed());
        assert_eq!(
            hook.key(vk::LCONTROL, true, ALTGR_FAKE_CTRL_SCAN),
            Verdict::default()
        );

        assert_eq!(
            hook.shortcut_events(),
            vec![(Record, Pressed), (Record, Released)]
        );
    }

    #[test]
    fn left_alt_does_not_trigger_right_alt() {
        let mut hook = Hook::with("RightAlt");

        assert_eq!(hook.down(vk::LMENU), Verdict::default());
        assert_eq!(hook.shortcut_events(), vec![]);
    }

    #[test]
    fn right_ctrl_alone_is_not_left_ctrl_and_not_ctrl_plus_another_key() {
        let mut hook = Hook::with("RightCtrl");

        hook.down(vk::LCONTROL);
        assert_eq!(hook.down(vk::RCONTROL), Verdict::default());
        hook.up(vk::RCONTROL);
        hook.up(vk::LCONTROL);
        assert_eq!(hook.down(vk::RCONTROL), swallowed());

        assert_eq!(hook.shortcut_events(), vec![(Record, Pressed)]);
    }

    #[test]
    fn the_cancel_shortcut_works_while_the_record_shortcut_is_held() {
        let mut hook = Hook::with("Ctrl+Space");
        hook.matcher
            .bind(Cancel, Some(&"Escape".parse().unwrap()))
            .unwrap();

        hook.down(vk::LCONTROL);
        hook.down(SPACE);
        assert_eq!(hook.down(vk::ESCAPE), swallowed());
        hook.up(vk::ESCAPE);
        hook.up(SPACE);

        assert_eq!(
            hook.shortcut_events(),
            vec![
                (Record, Pressed),
                (Cancel, Pressed),
                (Cancel, Released),
                (Record, Released)
            ]
        );
    }

    #[test]
    fn other_keys_pressed_while_the_shortcut_is_held_pass_through_and_do_not_release_it() {
        let mut hook = Hook::with("Ctrl+Space");

        hook.down(vk::LCONTROL);
        hook.down(SPACE);
        assert_eq!(hook.down(KEY_A), Verdict::default());
        assert_eq!(hook.up(KEY_A), Verdict::default());

        assert_eq!(hook.shortcut_events(), vec![(Record, Pressed)]);
    }

    #[test]
    fn a_lost_release_of_a_delivered_key_does_not_block_the_shortcut() {
        let mut hook = Hook::with("Ctrl+Space");

        hook.down(KEY_A);
        // Windows drops the hook event for A's release (e.g. while the secure desktop was up).
        hook.system.down.borrow_mut().remove(&KEY_A);
        hook.down(vk::LCONTROL);
        hook.down(SPACE);

        assert_eq!(hook.shortcut_events(), vec![(Record, Pressed)]);
    }

    #[test]
    fn a_lost_release_of_a_shortcut_modifier_releases_the_shortcut() {
        let mut hook = Hook::with("Ctrl+Space");

        hook.down(vk::LCONTROL);
        hook.down(SPACE);
        hook.system.down.borrow_mut().remove(&vk::LCONTROL);
        hook.down(KEY_A);

        assert_eq!(
            hook.shortcut_events(),
            vec![(Record, Pressed), (Record, Released)]
        );
    }

    #[test]
    fn a_lost_release_of_the_swallowed_key_heals_on_the_next_press_and_release() {
        let mut hook = Hook::with("Ctrl+Space");

        hook.down(vk::LCONTROL);
        hook.down(SPACE);
        // Space's release is lost; the next press looks like auto-repeat, its release releases.
        hook.down(SPACE);
        assert_eq!(hook.up(SPACE), swallowed());
        hook.up(vk::LCONTROL);
        hook.down(vk::LCONTROL);
        hook.down(SPACE);

        assert_eq!(
            hook.shortcut_events(),
            vec![(Record, Pressed), (Record, Released), (Record, Pressed)]
        );
    }

    #[test]
    fn unbinding_stops_matching_but_keeps_releases_of_swallowed_keys_swallowed() {
        let mut hook = Hook::with("Ctrl+Space");

        hook.down(vk::LCONTROL);
        hook.down(SPACE);
        hook.matcher.bind(Record, None).unwrap();
        assert_eq!(hook.up(SPACE), swallowed());
        hook.up(vk::LCONTROL);
        hook.down(vk::LCONTROL);
        assert_eq!(hook.down(SPACE), Verdict::default());

        assert_eq!(hook.shortcut_events(), vec![(Record, Pressed)]);
    }

    #[test]
    fn keys_injected_by_echo_are_ignored() {
        let mut hook = Hook::with("Ctrl+Space");
        let mut events = Vec::new();
        for (vk, up) in [(vk::LCONTROL, false), (SPACE, false)] {
            let raw = RawKey {
                vk,
                scan: 0,
                up,
                from_echo: true,
            };
            let verdict = hook
                .matcher
                .handle(raw, &hook.system, &mut |o| events.push(o));
            assert_eq!(verdict, Verdict::default());
        }
        assert!(events.is_empty());
    }

    #[test]
    fn binding_an_unknown_key_fails_and_keeps_the_old_binding() {
        let mut hook = Hook::with("Ctrl+Space");
        let bogus = KeyCombination {
            modifiers: Modifiers::default(),
            key: Some(super::super::Key("Bogus".into())),
        };

        assert!(hook.matcher.bind(Record, Some(&bogus)).is_err());
        hook.down(vk::LCONTROL);
        hook.down(SPACE);
        assert_eq!(hook.shortcut_events(), vec![(Record, Pressed)]);
    }

    #[test]
    fn capture_reports_side_specific_keys_and_swallows_them_while_echo_is_in_front() {
        let mut hook = Hook::with("Ctrl+Win");
        hook.system.foreground = true;
        hook.matcher.set_capturing(true);

        assert_eq!(hook.down(vk::LCONTROL), swallowed());
        assert_eq!(hook.down(vk::RWIN), swallowed());
        assert_eq!(hook.down(vk::RWIN), swallowed()); // repeat, not reported
        assert_eq!(hook.up(vk::RWIN), swallowed());
        assert_eq!(hook.up(vk::LCONTROL), swallowed());

        let captured: Vec<_> = hook
            .events
            .drain(..)
            .map(|o| match o {
                Output::Captured(c) => (c.key, c.action),
                Output::Shortcut(e) => panic!("shortcut event during capture: {e:?}"),
            })
            .collect();
        assert_eq!(
            captured,
            vec![
                ("LeftCtrl".to_owned(), Pressed),
                ("RightWin".to_owned(), Pressed),
                ("RightWin".to_owned(), Released),
                ("LeftCtrl".to_owned(), Released),
            ]
        );
    }

    #[test]
    fn capture_does_nothing_when_another_application_is_in_front() {
        let mut hook = Hook::with("Ctrl+Space");
        hook.matcher.bind(Record, None).unwrap();
        hook.matcher.set_capturing(true);

        assert_eq!(hook.down(KEY_A), Verdict::default());
        assert_eq!(hook.up(KEY_A), Verdict::default());
        assert!(hook.events.is_empty());
    }

    #[test]
    fn a_key_held_before_capture_started_is_released_to_its_application() {
        let mut hook = Hook::with("Ctrl+Space");
        hook.system.foreground = true;

        hook.down(KEY_A);
        hook.matcher.set_capturing(true);
        assert_eq!(hook.up(KEY_A), Verdict::default());
    }
}
