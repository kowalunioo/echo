//! The **Selection Reader** seam: reads the text selected in the focused application by copying
//! it (`vocabulary-shortcut.md` rules 4, 6–9).
//!
//! [`CopySelectionReader`] holds the decisions — wait for the keys to be released, Ctrl+Insert
//! first, Ctrl+C only outside terminals, restore the clipboard unless something else changed it —
//! over three seams: [`SelectionClipboard`], the Inserter's [`Desktop`] (keyboard state, injected
//! keystrokes) plus [`ForegroundWindow`], and a [`Clock`]. The Windows implementations live in
//! `win32`; tests use fakes, never the real keyboard.

use std::time::{Duration, Instant};

use crate::insertion::paste::{Clock, Desktop, Key, KeyEvent, POLL, Restore};

/// How long each copy keystroke may take to change the clipboard (rule 6).
pub const COPY_WAIT: Duration = Duration::from_millis(250);

/// What reading the selection found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// The copied text (not yet normalised).
    Text(String),
    /// Nothing is selected, or it is not text, or the copy could not reach the application.
    Nothing,
    /// The shortcut's keys were still held at the deadline: the attempt is dropped silently
    /// (rule 4).
    KeysHeld,
}

/// Reads the selected text of the focused application.
pub trait SelectionReader: Send {
    /// Waits until no key is held (at most until `release_by`), then reads the selection.
    fn read(&mut self, release_by: Instant) -> Selection;
}

/// The clipboard as reading the selection needs it.
pub trait SelectionClipboard: Send {
    /// Everything [`save`](Self::save) captured.
    type Saved: Send;

    /// Saves the current contents in every format that can be read (rule 6.1).
    fn save(&self) -> Result<Self::Saved, String>;
    /// The change counter, which changes whenever anything writes the clipboard.
    fn sequence(&self) -> u32;
    /// The clipboard's text, if it holds any, and the change counter at the moment it was read.
    fn text(&self) -> Result<(Option<String>, u32), String>;
    /// Puts `saved` back unless the counter has moved on from `sequence` (rule 8, as
    /// `dictation-pipeline.md` rules 34.5–34.7).
    fn restore_if_sequence(&self, saved: Self::Saved, sequence: u32) -> Result<Restore, String>;
}

/// The focused window, as far as copying needs to know it.
pub trait ForegroundWindow: Send {
    /// Whether it is a terminal, where Ctrl+C interrupts the running program (rule 6.3).
    fn is_terminal(&self) -> bool;
}

/// Window classes of terminals, where Ctrl+C with nothing selected interrupts the running
/// program (rule 6.3). Terminals hosted in another application's window (e.g. an editor's
/// integrated terminal) cannot be told apart this way and are not covered.
pub const TERMINAL_WINDOW_CLASSES: [&str; 7] = [
    // Console host: Command Prompt, PowerShell and other console programs.
    "ConsoleWindowClass",
    // Windows Terminal.
    "CASCADIA_HOSTING_WINDOW_CLASS",
    // mintty: Git Bash, Cygwin, MSYS2.
    "mintty",
    "PuTTY",
    "KiTTY",
    // ConEmu and Cmder.
    "VirtualConsoleClass",
    "org.wezfurlong.wezterm",
];

/// Whether a window of class `class` is a terminal ([`TERMINAL_WINDOW_CLASSES`]).
pub fn is_terminal_class(class: &str) -> bool {
    TERMINAL_WINDOW_CLASSES.contains(&class)
}

/// The real selection reader's logic.
pub struct CopySelectionReader<C, D, W, T> {
    clipboard: C,
    desktop: D,
    window: W,
    clock: T,
}

impl<C, D, W, T> CopySelectionReader<C, D, W, T>
where
    C: SelectionClipboard,
    D: Desktop,
    W: ForegroundWindow,
    T: Clock,
{
    pub fn new(clipboard: C, desktop: D, window: W, clock: T) -> Self {
        Self {
            clipboard,
            desktop,
            window,
            clock,
        }
    }

    /// Waits until Ctrl, Alt, Shift and Win are all up, so a held modifier cannot turn the copy
    /// keystroke into another combination (rule 4). The shortcut's other key was swallowed, so
    /// applications never saw it down. Returns whether they were released by `deadline`.
    fn keys_released(&self, deadline: Instant) -> bool {
        loop {
            if !self.desktop.modifiers_held() {
                return true;
            }
            if self.clock.now() >= deadline {
                return false;
            }
            self.clock.sleep(POLL);
        }
    }

    /// Sends Ctrl+`key` and waits up to [`COPY_WAIT`] for the clipboard to move on from
    /// `before`. Returns whether it did.
    fn copy_with(&mut self, key: Key, before: u32) -> bool {
        let events = [
            KeyEvent::Down(Key::Control),
            KeyEvent::Down(key),
            KeyEvent::Up(key),
            KeyEvent::Up(Key::Control),
        ];
        if let Err(error) = self.desktop.send(&events) {
            log::warn!("could not send the copy keystroke: {error}");
            return false;
        }
        let deadline = self.clock.now() + COPY_WAIT;
        loop {
            if self.clipboard.sequence() != before {
                return true;
            }
            if self.clock.now() >= deadline {
                return false;
            }
            self.clock.sleep(POLL);
        }
    }
}

impl<C, D, W, T> SelectionReader for CopySelectionReader<C, D, W, T>
where
    C: SelectionClipboard,
    D: Desktop,
    W: ForegroundWindow,
    T: Clock,
{
    fn read(&mut self, release_by: Instant) -> Selection {
        if !self.keys_released(release_by) {
            return Selection::KeysHeld;
        }
        // Windows silently drops simulated input into an elevated window (rule 9).
        if self.desktop.foreground_elevated() {
            return Selection::Nothing;
        }
        let saved = match self.clipboard.save() {
            Ok(saved) => saved,
            Err(error) => {
                // Copying now could lose the user's clipboard for good.
                log::warn!("could not save the clipboard; not copying the selection: {error}");
                return Selection::Nothing;
            }
        };
        let before = self.clipboard.sequence();
        let copied = self.copy_with(Key::Insert, before)
            || (!self.window.is_terminal() && self.copy_with(Key::C, before));
        if !copied {
            return Selection::Nothing;
        }
        let (text, sequence) = match self.clipboard.text() {
            Ok(read) => read,
            Err(error) => {
                log::warn!("could not read the copied selection: {error}");
                (None, self.clipboard.sequence())
            }
        };
        match self.clipboard.restore_if_sequence(saved, sequence) {
            Ok(Restore::Restored) => {}
            Ok(Restore::KeptNewer) => log::info!("the clipboard changed again; not restoring it"),
            Err(error) => log::warn!("could not restore the clipboard: {error}"),
        }
        text.map_or(Selection::Nothing, Selection::Text)
    }
}

/// A [`SelectionReader`] that returns what the test gives it and counts its calls.
#[derive(Clone, Default)]
pub struct FakeSelectionReader {
    state: std::sync::Arc<std::sync::Mutex<FakeState>>,
}

#[derive(Default)]
struct FakeState {
    next: Option<Selection>,
    calls: usize,
}

impl FakeSelectionReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// The selection the next reads return.
    pub fn select(&self, selection: Selection) {
        self.lock().next = Some(selection);
    }

    /// How often [`read`](SelectionReader::read) was called.
    pub fn calls(&self) -> usize {
        self.lock().calls
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SelectionReader for FakeSelectionReader {
    fn read(&mut self, _release_by: Instant) -> Selection {
        let mut state = self.lock();
        state.calls += 1;
        state.next.clone().unwrap_or(Selection::Nothing)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::insertion::paste::SendError;

    /// What the fake desktop and clipboard saw, in order.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Event {
        Saved,
        Sent(Key),
        Restored { sequence: u32 },
    }

    /// A clipboard holding "previous" whose change counter the test moves, as the focused
    /// application would by copying.
    #[derive(Clone)]
    struct World {
        inner: Arc<Mutex<Inner>>,
    }

    struct Inner {
        events: Vec<Event>,
        sequence: u32,
        text: Option<String>,
        /// Which copy keystroke makes the application write the clipboard, and what.
        copies_on: Option<(Key, Option<String>)>,
        modifiers_held_until: Option<Instant>,
        elevated: bool,
        terminal: bool,
        /// Another program writes the clipboard right after the copy.
        written_after_copy: bool,
        now: Instant,
    }

    impl World {
        fn new() -> Self {
            Self {
                inner: Arc::new(Mutex::new(Inner {
                    events: Vec::new(),
                    sequence: 7,
                    text: Some("previous".into()),
                    copies_on: None,
                    modifiers_held_until: None,
                    elevated: false,
                    terminal: false,
                    written_after_copy: false,
                    now: Instant::now(),
                })),
            }
        }

        fn with(&self, f: impl FnOnce(&mut Inner)) -> &Self {
            f(&mut self.inner.lock().unwrap());
            self
        }

        fn events(&self) -> Vec<Event> {
            self.inner.lock().unwrap().events.clone()
        }

        fn sent(&self) -> Vec<Key> {
            self.events()
                .into_iter()
                .filter_map(|e| match e {
                    Event::Sent(k) => Some(k),
                    _ => None,
                })
                .collect()
        }

        fn now(&self) -> Instant {
            self.inner.lock().unwrap().now
        }

        fn reader(&self) -> CopySelectionReader<World, World, World, World> {
            CopySelectionReader::new(self.clone(), self.clone(), self.clone(), self.clone())
        }
    }

    impl SelectionClipboard for World {
        type Saved = Option<String>;
        fn save(&self) -> Result<Option<String>, String> {
            let mut w = self.inner.lock().unwrap();
            w.events.push(Event::Saved);
            Ok(w.text.clone())
        }
        fn sequence(&self) -> u32 {
            self.inner.lock().unwrap().sequence
        }
        fn text(&self) -> Result<(Option<String>, u32), String> {
            let mut w = self.inner.lock().unwrap();
            let read = (w.text.clone(), w.sequence);
            if w.written_after_copy {
                w.sequence += 1;
                w.text = Some("newer".into());
            }
            Ok(read)
        }
        fn restore_if_sequence(
            &self,
            saved: Option<String>,
            sequence: u32,
        ) -> Result<Restore, String> {
            let mut w = self.inner.lock().unwrap();
            if w.sequence != sequence {
                return Ok(Restore::KeptNewer);
            }
            w.events.push(Event::Restored { sequence });
            w.text = saved;
            w.sequence += 1;
            Ok(Restore::Restored)
        }
    }

    impl Desktop for World {
        fn modifiers_held(&self) -> bool {
            let w = self.inner.lock().unwrap();
            w.modifiers_held_until.is_some_and(|until| w.now < until)
        }
        fn foreground_elevated(&self) -> bool {
            self.inner.lock().unwrap().elevated
        }
        fn send(&mut self, events: &[KeyEvent]) -> Result<(), SendError> {
            let mut w = self.inner.lock().unwrap();
            let key = match events {
                [
                    KeyEvent::Down(Key::Control),
                    KeyEvent::Down(k),
                    KeyEvent::Up(u),
                    KeyEvent::Up(Key::Control),
                ] if k == u => *k,
                other => panic!("not a Ctrl+key chord: {other:?}"),
            };
            w.events.push(Event::Sent(key));
            if let Some((on, text)) = w.copies_on.clone()
                && on == key
            {
                w.sequence += 1;
                w.text = text;
            }
            Ok(())
        }
    }

    impl ForegroundWindow for World {
        fn is_terminal(&self) -> bool {
            self.inner.lock().unwrap().terminal
        }
    }

    impl Clock for World {
        fn now(&self) -> Instant {
            self.inner.lock().unwrap().now
        }
        fn sleep(&self, duration: Duration) {
            self.inner.lock().unwrap().now += duration;
        }
    }

    fn far(world: &World) -> Instant {
        world.now() + Duration::from_secs(2)
    }

    /// Acceptance test 9, first case: Ctrl+Insert copied, so Ctrl+C is not sent.
    #[test]
    fn ctrl_insert_that_copies_is_enough() {
        let world = World::new();
        world.with(|w| w.copies_on = Some((Key::Insert, Some("Tauri".into()))));

        let selection = world.reader().read(far(&world));

        assert_eq!(selection, Selection::Text("Tauri".into()));
        assert_eq!(world.sent(), [Key::Insert]);
        // Saved first, restored afterwards: the clipboard holds "previous" again (test 16).
        assert_eq!(world.events()[0], Event::Saved);
        assert!(matches!(
            world.events().last(),
            Some(Event::Restored { .. })
        ));
        assert_eq!(
            world.inner.lock().unwrap().text.as_deref(),
            Some("previous")
        );
    }

    /// Acceptance test 9, second case.
    #[test]
    fn ctrl_c_follows_when_ctrl_insert_does_nothing_outside_a_terminal() {
        let world = World::new();
        world.with(|w| w.copies_on = Some((Key::C, Some("Tauri".into()))));
        let start = world.now();

        let selection = world.reader().read(far(&world));

        assert_eq!(selection, Selection::Text("Tauri".into()));
        assert_eq!(world.sent(), [Key::Insert, Key::C]);
        assert!(
            world.now() >= start + COPY_WAIT,
            "waited for Ctrl+Insert first"
        );
        assert_eq!(
            world.inner.lock().unwrap().text.as_deref(),
            Some("previous")
        );
    }

    /// Acceptance test 9, third case: Ctrl+C would interrupt the terminal's program.
    #[test]
    fn a_terminal_never_gets_ctrl_c() {
        let world = World::new();
        world.with(|w| {
            w.terminal = true;
            w.copies_on = Some((Key::C, Some("oops".into())));
        });

        assert_eq!(world.reader().read(far(&world)), Selection::Nothing);
        assert_eq!(world.sent(), [Key::Insert]);
        // Nothing changed, so nothing is restored.
        assert!(
            !world
                .events()
                .iter()
                .any(|e| matches!(e, Event::Restored { .. }))
        );
    }

    /// Rule 7: neither keystroke changed the clipboard, or it holds no text.
    #[test]
    fn no_change_or_no_text_means_nothing_selected() {
        let world = World::new();
        assert_eq!(world.reader().read(far(&world)), Selection::Nothing);
        assert_eq!(world.sent(), [Key::Insert, Key::C]);

        let world = World::new();
        world.with(|w| w.copies_on = Some((Key::Insert, None)));
        assert_eq!(world.reader().read(far(&world)), Selection::Nothing);
        assert_eq!(
            world.inner.lock().unwrap().text.as_deref(),
            Some("previous")
        );
    }

    /// Rule 8: a clipboard written by someone else after the copy is left alone.
    #[test]
    fn a_newer_clipboard_is_not_overwritten() {
        let world = World::new();
        world.with(|w| {
            w.copies_on = Some((Key::Insert, Some("Tauri".into())));
            w.written_after_copy = true;
        });

        assert_eq!(
            world.reader().read(far(&world)),
            Selection::Text("Tauri".into())
        );
        assert_eq!(world.inner.lock().unwrap().text.as_deref(), Some("newer"));
    }

    /// Rule 8 / dictation-pipeline.md rule 34.6: an empty clipboard is emptied again.
    #[test]
    fn an_empty_clipboard_is_emptied_again() {
        let world = World::new();
        world.with(|w| {
            w.text = None;
            w.copies_on = Some((Key::Insert, Some("Tauri".into())));
        });

        assert_eq!(
            world.reader().read(far(&world)),
            Selection::Text("Tauri".into())
        );
        assert_eq!(world.inner.lock().unwrap().text, None);
    }

    /// Rule 4: the copy waits for the modifiers, and gives up silently at the deadline.
    #[test]
    fn the_copy_waits_for_the_keys_to_be_released() {
        let world = World::new();
        let start = world.now();
        world.with(|w| {
            w.modifiers_held_until = Some(start + Duration::from_millis(300));
            w.copies_on = Some((Key::Insert, Some("Tauri".into())));
        });
        assert_eq!(
            world.reader().read(far(&world)),
            Selection::Text("Tauri".into())
        );
        assert!(world.now() >= start + Duration::from_millis(300));

        let world = World::new();
        let start = world.now();
        world.with(|w| w.modifiers_held_until = Some(start + Duration::from_secs(5)));
        assert_eq!(
            world.reader().read(start + Duration::from_secs(2)),
            Selection::KeysHeld
        );
        assert!(world.sent().is_empty());
        assert!(world.events().is_empty(), "the clipboard was not touched");
    }

    /// Rule 9: an elevated window cannot receive the copy keystrokes.
    #[test]
    fn an_elevated_window_means_nothing_selected_without_keystrokes() {
        let world = World::new();
        world.with(|w| w.elevated = true);
        assert_eq!(world.reader().read(far(&world)), Selection::Nothing);
        assert!(world.events().is_empty());
    }

    #[test]
    fn consoles_and_known_terminals_are_terminals() {
        assert!(is_terminal_class("ConsoleWindowClass"));
        assert!(is_terminal_class("CASCADIA_HOSTING_WINDOW_CLASS"));
        assert!(is_terminal_class("mintty"));
        assert!(!is_terminal_class("Notepad"));
        assert!(!is_terminal_class("Chrome_WidgetWin_1"));
    }

    #[test]
    fn the_fake_reader_counts_its_calls() {
        let mut fake = FakeSelectionReader::new();
        fake.select(Selection::Text("x".into()));
        assert_eq!(fake.read(Instant::now()), Selection::Text("x".into()));
        assert_eq!(fake.clone().calls(), 1);
    }
}
