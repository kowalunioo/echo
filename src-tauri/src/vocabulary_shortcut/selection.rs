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
    /// Selections cannot be read at all (the clipboard could not be set up): the attempt shows
    /// no message, because "select a word first" would send the user looking for the wrong fix.
    Unavailable,
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
    /// What the clipboard holds, read in one opening so the text, the editor's marker and the
    /// change counter belong together.
    fn copied(&self) -> Result<Copied, String>;
    /// Puts `saved` back unless the counter has moved on from `sequence` (rule 8, as
    /// `dictation-pipeline.md` rules 34.5–34.7).
    fn restore_if_sequence(&self, saved: Self::Saved, sequence: u32) -> Result<Restore, String>;
}

/// What the clipboard held after a copy keystroke.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Copied {
    /// Its text, if it holds any.
    pub text: Option<String>,
    /// The editor marked the copy as made from an empty selection, i.e. a whole-line copy
    /// (rule 7a).
    pub from_empty_selection: bool,
    /// The change counter at the moment it was read.
    pub sequence: u32,
}

impl Copied {
    /// Whether the copy is an editor's whole-line copy rather than a selection (rule 7a): marked
    /// as such, or ending in a line break (LF or CRLF).
    pub fn is_whole_line(&self) -> bool {
        self.from_empty_selection || self.text.as_deref().is_some_and(|t| t.ends_with('\n'))
    }
}

/// The clipboard format in which VS Code and editors built on it describe a copy.
pub const EDITOR_DATA_FORMAT: &str = "vscode-editor-data";

/// Whether `editor_data` (the JSON in [`EDITOR_DATA_FORMAT`]) marks a copy made with nothing
/// selected (`"isFromEmptySelection": true`). Anything unreadable counts as unmarked.
pub fn is_from_empty_selection(editor_data: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(editor_data.trim_end_matches('\0'))
        .ok()
        .and_then(|data| {
            data.get("isFromEmptySelection")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(false)
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
    /// `before`. Returns what it then holds, or `None` if it did not change.
    ///
    /// An application copies by emptying the clipboard and then writing it, and emptying alone
    /// already changes the sequence number. So a change that left no text is not taken as the
    /// copy until the wait is over: the reader keeps polling for a further change that brings
    /// the text.
    fn copy_with(&mut self, key: Key, before: u32) -> Option<Copied> {
        let events = [
            KeyEvent::Down(Key::Control),
            KeyEvent::Down(key),
            KeyEvent::Up(key),
            KeyEvent::Up(Key::Control),
        ];
        if let Err(error) = self.desktop.send(&events) {
            log::warn!("could not send the copy keystroke: {error}");
            return None;
        }
        let deadline = self.clock.now() + COPY_WAIT;
        let mut seen = before;
        let mut copied = None;
        loop {
            let sequence = self.clipboard.sequence();
            if sequence != seen {
                match self.clipboard.copied() {
                    Ok(read) if read.text.is_some() => return Some(read),
                    Ok(read) => {
                        seen = read.sequence;
                        copied = Some(read);
                    }
                    // Still held open by the application: try again on the next poll.
                    Err(error) => {
                        log::debug!("could not read the clipboard yet: {error}");
                        copied.get_or_insert(Copied {
                            sequence,
                            ..Copied::default()
                        });
                    }
                }
            }
            if self.clock.now() >= deadline {
                return copied;
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
        let copied = match self.copy_with(Key::Insert, before) {
            Some(copied) => copied,
            None if self.window.is_terminal() => return Selection::Nothing,
            None => match self.copy_with(Key::C, before) {
                Some(copied) => copied,
                None => return Selection::Nothing,
            },
        };
        match self.clipboard.restore_if_sequence(saved, copied.sequence) {
            Ok(Restore::Restored) => {}
            Ok(Restore::KeptNewer) => log::info!("the clipboard changed again; not restoring it"),
            Err(error) => log::warn!("could not restore the clipboard: {error}"),
        }
        // An editor's whole-line copy, made with nothing selected (rule 7a).
        if copied.is_whole_line() {
            return Selection::Nothing;
        }
        copied.text.map_or(Selection::Nothing, Selection::Text)
    }
}

/// The [`SelectionReader`] used when the real one could not be set up: every attempt is
/// [`Selection::Unavailable`], with a warning in the log.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableSelectionReader;

impl SelectionReader for UnavailableSelectionReader {
    fn read(&mut self, _release_by: Instant) -> Selection {
        log::warn!("Vocabulary Shortcut unavailable: clipboard");
        Selection::Unavailable
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
        /// The application empties the clipboard at once but writes the copy only this much
        /// later, as `EmptyClipboard` then `SetClipboardData` do.
        copy_lands_after: Option<Duration>,
        /// The copy is written for `at`.
        pending: Option<(Instant, Option<String>)>,
        /// The copy carries the editor's empty-selection marker.
        copy_marked: bool,
        /// The clipboard carries the marker now.
        marked: bool,
        now: Instant,
    }

    impl Inner {
        /// Lets a pending copy land once its time has come.
        fn settle(&mut self) {
            if let Some((at, text)) = self.pending.take() {
                if self.now >= at {
                    self.sequence += 1;
                    self.text = text;
                } else {
                    self.pending = Some((at, text));
                }
            }
        }
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
                    copy_lands_after: None,
                    pending: None,
                    copy_marked: false,
                    marked: false,
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
            let mut w = self.inner.lock().unwrap();
            w.settle();
            w.sequence
        }
        fn copied(&self) -> Result<Copied, String> {
            let mut w = self.inner.lock().unwrap();
            w.settle();
            let read = Copied {
                text: w.text.clone(),
                from_empty_selection: w.marked,
                sequence: w.sequence,
            };
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
            w.marked = false;
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
                w.marked = w.copy_marked;
                match w.copy_lands_after {
                    Some(after) => {
                        w.text = None;
                        w.pending = Some((w.now + after, text));
                    }
                    None => w.text = text,
                }
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

    /// The application empties the clipboard before writing the copy: the first change is not
    /// the copy yet, so the reader waits for the text instead of reporting nothing selected.
    #[test]
    fn a_copy_written_after_emptying_the_clipboard_is_waited_for() {
        let world = World::new();
        world.with(|w| {
            w.copies_on = Some((Key::Insert, Some("Tauri".into())));
            w.copy_lands_after = Some(Duration::from_millis(40));
        });

        assert_eq!(
            world.reader().read(far(&world)),
            Selection::Text("Tauri".into())
        );
        assert_eq!(world.sent(), [Key::Insert], "Ctrl+Insert was enough");
        assert_eq!(
            world.inner.lock().unwrap().text.as_deref(),
            Some("previous")
        );
    }

    /// Acceptance test 9a: an editor's whole-line copy (marked, or ending in a line break) is
    /// nothing selected, and the clipboard is still restored.
    #[test]
    fn a_whole_line_copy_means_nothing_selected() {
        let marked = World::new();
        marked.with(|w| {
            w.copies_on = Some((Key::Insert, Some("let x = 1;".into())));
            w.copy_marked = true;
        });
        let lf = World::new();
        lf.with(|w| w.copies_on = Some((Key::Insert, Some("GitHub\n".into()))));
        let crlf = World::new();
        crlf.with(|w| w.copies_on = Some((Key::Insert, Some("GitHub\r\n".into()))));

        for world in [marked, lf, crlf] {
            assert_eq!(world.reader().read(far(&world)), Selection::Nothing);
            assert_eq!(world.sent(), [Key::Insert], "the copy counted; no Ctrl+C");
            assert_eq!(
                world.inner.lock().unwrap().text.as_deref(),
                Some("previous")
            );
        }
    }

    /// Rule 7a: only `isFromEmptySelection: true` marks a whole-line copy.
    #[test]
    fn the_editor_marker_is_read_from_its_json() {
        assert!(is_from_empty_selection(
            r#"{"version":1,"isFromEmptySelection":true,"multicursorText":null,"mode":"rust"}"#
        ));
        assert!(is_from_empty_selection("{\"isFromEmptySelection\":true}\0"));
        assert!(!is_from_empty_selection(
            r#"{"version":1,"isFromEmptySelection":false,"mode":"rust"}"#
        ));
        assert!(!is_from_empty_selection(r#"{"version":1}"#));
        assert!(!is_from_empty_selection("not json"));
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
