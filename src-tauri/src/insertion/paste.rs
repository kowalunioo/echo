//! The platform-independent half of the real Inserter: the order, timing and restore decisions
//! of `docs/specs/dictation-pipeline.md` rules 33–36.
//!
//! The operating system is reached only through three small seams — [`Clipboard`], [`Desktop`]
//! (keyboard state, injected keystrokes, the focused window) and [`Clock`] — so every decision
//! here is unit-tested with fakes. The Windows implementations live in `insertion::win32`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{Inserter, InsertionError};

/// How long Insertion waits for the user to release Ctrl, Alt, Shift and Win (rule 33).
pub const MODIFIER_WAIT: Duration = Duration::from_millis(1500);
/// How often waiting loops look again.
pub const POLL: Duration = Duration::from_millis(10);
/// How long Ctrl stays down after V, so slow applications register Ctrl+V (rule 34.3).
pub const PASTE_HOLD: Duration = Duration::from_millis(100);
/// Quiet time after the target's last clipboard read before restoring (rule 34.4).
pub const QUIET_AFTER_READ: Duration = Duration::from_millis(200);
/// Longest wait for the restore when the paste keystroke was sent (rule 34.4).
pub const READ_TIMEOUT: Duration = Duration::from_secs(8);
/// Wait for the restore when the paste keystroke could not be sent (rule 34.4).
pub const NOT_SENT_TIMEOUT: Duration = Duration::from_millis(500);
/// Characters per injected batch when typing, so very long Transcripts reach slow applications
/// in digestible pieces.
const TYPING_BATCH_CHARS: usize = 64;

/// A key Echo presses itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Control,
    V,
    Return,
    Tab,
}

/// One injected keyboard event. Every event carries `ECHO_INPUT_TAG` so the Record Shortcut
/// hook ignores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Down(Key),
    Up(Key),
    /// One UTF-16 code unit, pressed and released as a Unicode keystroke.
    Unit(u16),
}

/// Why injected keystrokes did not all arrive.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    /// Windows refused the input (e.g. the focused window runs as administrator).
    #[error("Windows refused the simulated keystrokes")]
    Blocked,
    #[error("simulated keystrokes failed: {0}")]
    Failed(String),
}

/// Keyboard and focused-window access.
pub trait Desktop: Send {
    /// Whether Ctrl, Alt, Shift or Win is held down right now.
    fn modifiers_held(&self) -> bool;
    /// Whether the focused window belongs to a process with higher privileges than Echo, into
    /// which Windows silently drops simulated input. `false` when it cannot be determined.
    fn foreground_elevated(&self) -> bool;
    /// Injects `events` in order. `Ok` only if Windows accepted all of them.
    fn send(&mut self, events: &[KeyEvent]) -> Result<(), SendError>;
}

/// What a restore did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restore {
    /// The saved contents are back.
    Restored,
    /// Something else changed the clipboard after the Transcript was put there; it was left
    /// alone (rule 34.5).
    KeptNewer,
}

/// The clipboard as the paste method needs it. Shared between the inserting thread and the
/// background restore.
pub trait Clipboard: Send + Sync + 'static {
    /// Everything [`save`](Self::save) captured.
    type Saved: Send + 'static;

    /// Saves the current contents in every format that can be read (rule 34.1). Fails only if
    /// the clipboard cannot be opened.
    fn save(&self) -> Result<Self::Saved, String>;
    /// Puts `text` on the clipboard as Unicode text, excluded from clipboard history, cloud
    /// sync and clipboard monitors (rule 34.2).
    fn put(&self, text: &str) -> Result<(), String>;
    /// When an application last read the Transcript put by the latest [`put`](Self::put).
    fn last_read(&self) -> Option<Instant>;
    /// Whether anything else changed the clipboard since the latest [`put`](Self::put).
    fn changed(&self) -> bool;
    /// Puts `saved` back unless the clipboard [`changed`](Self::changed) — checked atomically
    /// with the restore. An empty `saved` leaves the clipboard empty (rule 34.6); the restored
    /// contents are not added to clipboard history (rule 34.7).
    fn restore(&self, saved: Self::Saved) -> Result<Restore, String>;
}

/// Time, so waits are testable.
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Instant;
    fn sleep(&self, duration: Duration);
}

/// The real clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// The real Inserter's logic: clipboard paste with a read-confirmed restore, Unicode typing as
/// the fallback.
///
/// [`insert`](Inserter::insert) returns once the text was pasted or typed; the restore of the
/// user's clipboard then runs on a background thread. A following Insertion (or dropping the
/// Inserter) finishes a still-waiting restore at once first, so it never saves Echo's own
/// Transcript as "the user's clipboard".
pub struct PasteInserter<C: Clipboard, D: Desktop, T: Clock> {
    clipboard: Arc<C>,
    desktop: D,
    clock: Arc<T>,
    pending: Option<PendingRestore>,
}

struct PendingRestore {
    hurry: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

/// What a clipboard restore needs: the saved contents and how long to wait for the read.
type RestoreJob<S> = (S, Duration);

/// Why the paste method did not deliver the text.
enum PasteError {
    Clipboard(String),
    Keys(SendError),
}

impl<C: Clipboard, D: Desktop, T: Clock> PasteInserter<C, D, T> {
    pub fn new(clipboard: Arc<C>, desktop: D, clock: Arc<T>) -> Self {
        Self {
            clipboard,
            desktop,
            clock,
            pending: None,
        }
    }

    /// Ends a restore that is still waiting: restores now (unless the user changed the
    /// clipboard) and waits for it.
    pub fn finish_restore(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.hurry.store(true, Ordering::SeqCst);
            let _ = pending.thread.join();
        }
    }

    /// Waits until the background restore has finished by its own rules.
    #[cfg(test)]
    pub(crate) fn wait_for_restore(&mut self) {
        if let Some(pending) = self.pending.take() {
            let _ = pending.thread.join();
        }
    }

    fn wait_for_modifiers(&self) {
        let start = self.clock.now();
        while self.desktop.modifiers_held() && self.clock.now() - start < MODIFIER_WAIT {
            self.clock.sleep(POLL);
        }
    }

    /// Pastes `text`. Whenever the clipboard was touched, also returns what its restore needs —
    /// the saved contents and the timeout; the caller starts the restore once it is done with
    /// the keyboard.
    fn paste(&mut self, text: &str) -> (Result<(), PasteError>, Option<RestoreJob<C::Saved>>) {
        let saved = match self.clipboard.save() {
            Ok(saved) => saved,
            Err(error) => return (Err(PasteError::Clipboard(error)), None),
        };
        if let Err(error) = self.clipboard.put(text) {
            // The clipboard may be half-written; put the user's contents back straight away.
            return (
                Err(PasteError::Clipboard(error)),
                Some((saved, Duration::ZERO)),
            );
        }
        match self.send_paste_keystroke() {
            Ok(()) => (Ok(()), Some((saved, READ_TIMEOUT))),
            Err(error) => (
                Err(PasteError::Keys(error)),
                Some((saved, NOT_SENT_TIMEOUT)),
            ),
        }
    }

    fn send_paste_keystroke(&mut self) -> Result<(), SendError> {
        let pressed = self.desktop.send(&[
            KeyEvent::Down(Key::Control),
            KeyEvent::Down(Key::V),
            KeyEvent::Up(Key::V),
        ]);
        if pressed.is_ok() {
            self.clock.sleep(PASTE_HOLD);
        }
        // Release Ctrl even after a partial failure so it is never left stuck down. V already
        // went out, so a failure here does not undo the paste.
        if let Err(error) = self.desktop.send(&[KeyEvent::Up(Key::Control)]) {
            log::warn!("could not release Ctrl after pasting: {error}");
        }
        pressed
    }

    fn type_text(&mut self, text: &str) -> Result<(), SendError> {
        let chars: Vec<char> = text.chars().collect();
        for batch in chars.chunks(TYPING_BATCH_CHARS) {
            let events = typing_events(batch);
            if !events.is_empty() {
                self.desktop.send(&events)?;
            }
        }
        Ok(())
    }

    fn spawn_restore(&mut self, saved: C::Saved, timeout: Duration) {
        let hurry = Arc::new(AtomicBool::new(false));
        let clipboard = Arc::clone(&self.clipboard);
        let clock = Arc::clone(&self.clock);
        let flag = Arc::clone(&hurry);
        let thread = std::thread::Builder::new()
            .name("echo-clipboard-restore".into())
            .spawn(move || restore_when_read(&*clipboard, &*clock, saved, timeout, &flag));
        match thread {
            Ok(thread) => self.pending = Some(PendingRestore { hurry, thread }),
            Err(error) => log::error!("could not start the clipboard restore: {error}"),
        }
    }
}

impl<C: Clipboard, D: Desktop, T: Clock> Inserter for PasteInserter<C, D, T> {
    fn insert(&mut self, text: &str) -> Result<(), InsertionError> {
        self.finish_restore();
        if self.desktop.foreground_elevated() {
            return Err(InsertionError::Blocked(
                "the focused window runs as administrator".into(),
            ));
        }
        self.wait_for_modifiers();

        let (pasted, restore) = self.paste(text);
        let result = match pasted {
            Ok(()) => Ok(()),
            // Typing would go through the same refused channel.
            Err(PasteError::Keys(SendError::Blocked)) => {
                Err(InsertionError::Blocked(SendError::Blocked.to_string()))
            }
            Err(PasteError::Clipboard(error)) => {
                self.type_instead(text, &format!("clipboard: {error}"))
            }
            Err(PasteError::Keys(error)) => self.type_instead(text, &error.to_string()),
        };
        // Started only now, so its timeout counts from the end of the keyboard work.
        if let Some((saved, timeout)) = restore {
            self.spawn_restore(saved, timeout);
        }
        result
    }
}

impl<C: Clipboard, D: Desktop, T: Clock> PasteInserter<C, D, T> {
    /// The typing fallback (rule 35).
    fn type_instead(&mut self, text: &str, paste_error: &str) -> Result<(), InsertionError> {
        log::warn!("clipboard paste failed ({paste_error}); typing the text instead");
        self.type_text(text).map_err(|error| match error {
            SendError::Blocked => InsertionError::Blocked(error.to_string()),
            SendError::Failed(_) => InsertionError::Failed(format!(
                "paste failed ({paste_error}); typing failed ({error})"
            )),
        })
    }
}

impl<C: Clipboard, D: Desktop, T: Clock> Drop for PasteInserter<C, D, T> {
    fn drop(&mut self) {
        self.finish_restore();
    }
}

/// Waits until the target has read the Transcript (plus a quiet period), `timeout` has passed or
/// `hurry` is set, then restores — unless the clipboard changed meanwhile (rule 34.4–34.5).
fn restore_when_read<C: Clipboard>(
    clipboard: &C,
    clock: &impl Clock,
    saved: C::Saved,
    timeout: Duration,
    hurry: &AtomicBool,
) {
    let start = clock.now();
    loop {
        if clipboard.changed() {
            log::info!("the clipboard changed during Insertion; keeping the newer contents");
            return;
        }
        let now = clock.now();
        let quiet = clipboard
            .last_read()
            .is_some_and(|read| now.saturating_duration_since(read) >= QUIET_AFTER_READ);
        if quiet || now - start >= timeout || hurry.load(Ordering::SeqCst) {
            break;
        }
        clock.sleep(POLL);
    }
    match clipboard.restore(saved) {
        Ok(Restore::Restored) => {}
        Ok(Restore::KeptNewer) => {
            log::info!("the clipboard changed during Insertion; keeping the newer contents");
        }
        Err(error) => log::warn!("could not restore the clipboard: {error}"),
    }
}

/// The keystrokes that type `chars`: Unicode keystrokes, except line breaks and tabs, which are
/// sent as Enter and Tab because many applications ignore them as Unicode input.
fn typing_events(chars: &[char]) -> Vec<KeyEvent> {
    let mut events = Vec::with_capacity(chars.len());
    let mut buf = [0u16; 2];
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\r' if chars.get(i + 1) == Some(&'\n') => {}
            '\r' | '\n' => events.extend([KeyEvent::Down(Key::Return), KeyEvent::Up(Key::Return)]),
            '\t' => events.extend([KeyEvent::Down(Key::Tab), KeyEvent::Up(Key::Tab)]),
            _ => events.extend(c.encode_utf16(&mut buf).iter().map(|&u| KeyEvent::Unit(u))),
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    /// A virtual clock shared by all threads: `sleep` advances it instantly.
    struct FakeClock {
        start: Instant,
        elapsed: Mutex<Duration>,
    }

    impl FakeClock {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                start: Instant::now(),
                elapsed: Mutex::new(Duration::ZERO),
            })
        }

        fn ms(&self) -> u128 {
            self.elapsed.lock().unwrap().as_millis()
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            self.start + *self.elapsed.lock().unwrap()
        }

        fn sleep(&self, duration: Duration) {
            *self.elapsed.lock().unwrap() += duration;
        }
    }

    /// What happened, in order, with the virtual time in ms.
    type Journal = Arc<Mutex<Vec<String>>>;

    fn note(journal: &Journal, clock: &FakeClock, what: String) {
        journal
            .lock()
            .unwrap()
            .push(format!("{} {what}", clock.ms()));
    }

    #[derive(Default)]
    struct ClipState {
        /// `None` is an empty clipboard.
        content: Option<String>,
        put_at: Option<Instant>,
        open_fails: bool,
        put_fails: bool,
        /// The target application reads this long after the put.
        reads_after: Vec<Duration>,
        /// The user copies "newer" this long after the put.
        user_copies_after: Option<Duration>,
    }

    struct FakeClipboard {
        clock: Arc<FakeClock>,
        journal: Journal,
        state: Mutex<ClipState>,
    }

    impl FakeClipboard {
        fn new(clock: &Arc<FakeClock>, journal: &Journal, content: Option<&str>) -> Arc<Self> {
            Arc::new(Self {
                clock: Arc::clone(clock),
                journal: Arc::clone(journal),
                state: Mutex::new(ClipState {
                    content: content.map(str::to_owned),
                    ..ClipState::default()
                }),
            })
        }

        fn state(&self) -> MutexGuard<'_, ClipState> {
            self.state.lock().unwrap()
        }

        fn user_copied(&self, state: &ClipState) -> bool {
            match (state.put_at, state.user_copies_after) {
                (Some(put), Some(after)) => self.clock.now() >= put + after,
                _ => false,
            }
        }

        fn content(&self) -> Option<String> {
            let state = self.state();
            if self.user_copied(&state) {
                Some("newer".into())
            } else {
                state.content.clone()
            }
        }
    }

    impl Clipboard for FakeClipboard {
        type Saved = Option<String>;

        fn save(&self) -> Result<Option<String>, String> {
            let state = self.state();
            if state.open_fails {
                return Err("held open by another process".into());
            }
            note(
                &self.journal,
                &self.clock,
                format!("save {:?}", state.content),
            );
            Ok(state.content.clone())
        }

        fn put(&self, text: &str) -> Result<(), String> {
            let mut state = self.state();
            if state.put_fails {
                return Err("write failed".into());
            }
            note(&self.journal, &self.clock, format!("put {text:?}"));
            state.content = Some(text.to_owned());
            state.put_at = Some(self.clock.now());
            Ok(())
        }

        fn last_read(&self) -> Option<Instant> {
            let state = self.state();
            let put = state.put_at?;
            let now = self.clock.now();
            state
                .reads_after
                .iter()
                .map(|after| put + *after)
                .filter(|read| *read <= now)
                .max()
        }

        fn changed(&self) -> bool {
            self.user_copied(&self.state())
        }

        fn restore(&self, saved: Option<String>) -> Result<Restore, String> {
            let mut state = self.state();
            if self.user_copied(&state) {
                note(&self.journal, &self.clock, "kept newer".into());
                return Ok(Restore::KeptNewer);
            }
            note(&self.journal, &self.clock, format!("restore {saved:?}"));
            state.content = saved;
            Ok(Restore::Restored)
        }
    }

    /// Sends whose first event matches fail with the error.
    type Failure = (fn(&KeyEvent) -> bool, SendError);

    struct FakeDesktop {
        clock: Arc<FakeClock>,
        journal: Journal,
        modifiers_until: Duration,
        elevated: bool,
        /// Sends whose first event matches this fail with the error.
        fail: Option<Failure>,
    }

    impl FakeDesktop {
        fn new(clock: &Arc<FakeClock>, journal: &Journal) -> Self {
            Self {
                clock: Arc::clone(clock),
                journal: Arc::clone(journal),
                modifiers_until: Duration::ZERO,
                elevated: false,
                fail: None,
            }
        }
    }

    fn is_paste(event: &KeyEvent) -> bool {
        *event == KeyEvent::Down(Key::Control)
    }

    fn is_typing(event: &KeyEvent) -> bool {
        matches!(event, KeyEvent::Unit(_))
    }

    fn any(_: &KeyEvent) -> bool {
        true
    }

    impl Desktop for FakeDesktop {
        fn modifiers_held(&self) -> bool {
            self.clock.now() < self.clock.start + self.modifiers_until
        }

        fn foreground_elevated(&self) -> bool {
            self.elevated
        }

        fn send(&mut self, events: &[KeyEvent]) -> Result<(), SendError> {
            if let Some((matches, error)) = &self.fail
                && events.first().is_some_and(matches)
            {
                note(
                    &self.journal,
                    &self.clock,
                    format!("refused {:?}", events[0]),
                );
                return Err(error.clone());
            }
            let text: String = events
                .iter()
                .map(|e| match e {
                    KeyEvent::Unit(u) => format!("{u:04x}"),
                    other => format!("{other:?}"),
                })
                .collect::<Vec<_>>()
                .join(" ");
            note(&self.journal, &self.clock, format!("keys {text}"));
            Ok(())
        }
    }

    struct Rig {
        clock: Arc<FakeClock>,
        journal: Journal,
        clipboard: Arc<FakeClipboard>,
        desktop: FakeDesktop,
    }

    fn rig(content: Option<&str>) -> Rig {
        let clock = FakeClock::new();
        let journal = Journal::default();
        let clipboard = FakeClipboard::new(&clock, &journal, content);
        let desktop = FakeDesktop::new(&clock, &journal);
        Rig {
            clock,
            journal,
            clipboard,
            desktop,
        }
    }

    impl Rig {
        fn inserter(self) -> (PasteInserter<FakeClipboard, FakeDesktop, FakeClock>, Probe) {
            let probe = Probe {
                journal: Arc::clone(&self.journal),
                clipboard: Arc::clone(&self.clipboard),
            };
            (
                PasteInserter::new(self.clipboard, self.desktop, self.clock),
                probe,
            )
        }
    }

    struct Probe {
        journal: Journal,
        clipboard: Arc<FakeClipboard>,
    }

    impl Probe {
        fn journal(&self) -> Vec<String> {
            self.journal.lock().unwrap().clone()
        }
    }

    const PASTE: &str = "keys Down(Control) Down(V) Up(V)";
    const CTRL_UP: &str = "keys Up(Control)";

    #[test]
    fn pastes_through_the_clipboard_and_restores_after_the_target_read_it() {
        let rig = rig(Some("previous"));
        rig.clipboard.state().reads_after = vec![Duration::from_millis(30)];
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("hello").unwrap();
        inserter.wait_for_restore();

        // Read at 30 ms, quiet until 230 ms (the restore poll starts once Ctrl is up at 100).
        assert_eq!(
            probe.journal(),
            vec![
                "0 save Some(\"previous\")".to_owned(),
                "0 put \"hello\"".into(),
                format!("0 {PASTE}"),
                format!("100 {CTRL_UP}"),
                "230 restore Some(\"previous\")".into(),
            ]
        );
        assert_eq!(probe.clipboard.content().as_deref(), Some("previous"));
    }

    #[test]
    fn inserts_the_text_exactly_without_added_spaces() {
        let rig = rig(None);
        let (mut inserter, probe) = rig.inserter();

        inserter.insert(" zażółć ").unwrap();

        assert!(probe.journal().contains(&"0 put \" zażółć \"".to_owned()));
    }

    #[test]
    fn several_reads_restore_200_ms_after_the_last_one() {
        let rig = rig(Some("previous"));
        rig.clipboard.state().reads_after = [20, 150, 290].map(Duration::from_millis).to_vec();
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("hello").unwrap();
        inserter.wait_for_restore();

        assert_eq!(
            probe.journal().last().unwrap(),
            "490 restore Some(\"previous\")"
        );
    }

    #[test]
    fn without_an_observed_read_restores_after_8_s() {
        let rig = rig(Some("previous"));
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("hello").unwrap();
        inserter.wait_for_restore();

        // The 8 s count starts after the keystroke (Ctrl released at 100 ms).
        assert_eq!(
            probe.journal().last().unwrap(),
            "8100 restore Some(\"previous\")"
        );
    }

    #[test]
    fn an_empty_clipboard_is_restored_as_empty() {
        let rig = rig(None);
        rig.clipboard.state().reads_after = vec![Duration::ZERO];
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("hello").unwrap();
        inserter.wait_for_restore();

        assert_eq!(probe.journal().last().unwrap(), "200 restore None");
        assert_eq!(probe.clipboard.content(), None);
    }

    #[test]
    fn a_user_copy_before_the_restore_wins() {
        let rig = rig(Some("previous"));
        rig.clipboard.state().user_copies_after = Some(Duration::from_millis(1000));
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("hello").unwrap();
        inserter.wait_for_restore();

        assert!(!probe.journal().iter().any(|e| e.contains("restore")));
        assert_eq!(probe.clipboard.content().as_deref(), Some("newer"));
    }

    #[test]
    fn waits_for_modifiers_to_be_released_before_touching_anything() {
        let mut rig = rig(Some("previous"));
        rig.desktop.modifiers_until = Duration::from_millis(400);
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("hello").unwrap();

        assert_eq!(probe.journal()[0], "400 save Some(\"previous\")");
    }

    #[test]
    fn modifiers_held_longer_than_1_5_s_do_not_block_insertion() {
        let mut rig = rig(Some("previous"));
        rig.desktop.modifiers_until = Duration::from_secs(60);
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("hello").unwrap();

        assert_eq!(probe.journal()[0], "1500 save Some(\"previous\")");
    }

    #[test]
    fn types_the_text_when_the_clipboard_cannot_be_opened() {
        let rig = rig(Some("previous"));
        rig.clipboard.state().open_fails = true;
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("aż").unwrap();
        inserter.wait_for_restore();

        // 'a' = 0061, 'ż' = 017c; nothing on the clipboard changed.
        assert_eq!(probe.journal(), vec!["0 keys 0061 017c"]);
        assert_eq!(probe.clipboard.content().as_deref(), Some("previous"));
    }

    #[test]
    fn types_the_text_and_restores_at_once_when_the_clipboard_cannot_be_written() {
        let rig = rig(Some("previous"));
        rig.clipboard.state().put_fails = true;
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("a").unwrap();
        inserter.wait_for_restore();

        let journal = probe.journal();
        assert!(journal.contains(&"0 keys 0061".to_owned()));
        assert!(journal.contains(&"0 restore Some(\"previous\")".to_owned()));
        assert!(!journal.iter().any(|e| e.contains("Down(V)")));
    }

    #[test]
    fn types_the_text_and_restores_after_half_a_second_when_the_keystroke_fails() {
        let mut rig = rig(Some("previous"));
        rig.desktop.fail = Some((is_paste, SendError::Failed("no".into())));
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("a").unwrap();
        inserter.wait_for_restore();

        assert_eq!(
            probe.journal(),
            vec![
                "0 save Some(\"previous\")".to_owned(),
                "0 put \"a\"".into(),
                "0 refused Down(Control)".into(),
                format!("0 {CTRL_UP}"),
                "0 keys 0061".into(),
                "500 restore Some(\"previous\")".into(),
            ]
        );
    }

    #[test]
    fn typing_is_not_used_when_the_paste_succeeded() {
        let rig = rig(None);
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("a").unwrap();

        assert!(!probe.journal().iter().any(|e| e.contains("0061")));
    }

    #[test]
    fn typing_sends_surrogate_pairs_line_breaks_and_tabs() {
        let chars: Vec<char> = "a😀\r\nb\tc\n".chars().collect();
        assert_eq!(
            typing_events(&chars),
            vec![
                KeyEvent::Unit(0x61),
                KeyEvent::Unit(0xD83D),
                KeyEvent::Unit(0xDE00),
                KeyEvent::Down(Key::Return),
                KeyEvent::Up(Key::Return),
                KeyEvent::Unit(0x62),
                KeyEvent::Down(Key::Tab),
                KeyEvent::Up(Key::Tab),
                KeyEvent::Unit(0x63),
                KeyEvent::Down(Key::Return),
                KeyEvent::Up(Key::Return),
            ]
        );
    }

    #[test]
    fn long_text_is_typed_in_batches_that_never_split_a_character() {
        let mut rig = rig(None);
        rig.clipboard.state().open_fails = true;
        rig.desktop.fail = None;
        let (mut inserter, probe) = rig.inserter();
        let text = "😀".repeat(TYPING_BATCH_CHARS + 1);

        inserter.insert(&text).unwrap();

        let journal = probe.journal();
        assert_eq!(journal.len(), 2);
        assert_eq!(journal[1], "0 keys d83d de00");
    }

    #[test]
    fn an_elevated_foreground_window_fails_with_blocked_without_touching_anything() {
        let mut rig = rig(Some("previous"));
        rig.desktop.elevated = true;
        let (mut inserter, probe) = rig.inserter();

        assert!(matches!(
            inserter.insert("a"),
            Err(InsertionError::Blocked(_))
        ));
        assert!(probe.journal().is_empty());
    }

    #[test]
    fn refused_keystrokes_fail_with_blocked_and_restore_the_clipboard() {
        let mut rig = rig(Some("previous"));
        rig.desktop.fail = Some((is_paste, SendError::Blocked));
        let (mut inserter, probe) = rig.inserter();

        assert!(matches!(
            inserter.insert("a"),
            Err(InsertionError::Blocked(_))
        ));
        inserter.wait_for_restore();

        assert_eq!(
            probe.journal().last().unwrap(),
            "500 restore Some(\"previous\")"
        );
    }

    #[test]
    fn fails_when_both_methods_fail() {
        let mut rig = rig(Some("previous"));
        rig.clipboard.state().open_fails = true;
        rig.desktop.fail = Some((is_typing, SendError::Failed("no".into())));
        let (mut inserter, _probe) = rig.inserter();

        let error = inserter.insert("a").unwrap_err();

        let InsertionError::Failed(detail) = error else {
            panic!("expected Failed, got {error:?}");
        };
        assert!(detail.contains("held open"), "{detail}");
    }

    #[test]
    fn typing_refused_by_windows_is_blocked() {
        let mut rig = rig(None);
        rig.clipboard.state().open_fails = true;
        rig.desktop.fail = Some((any, SendError::Blocked));
        let (mut inserter, _probe) = rig.inserter();

        assert!(matches!(
            inserter.insert("a"),
            Err(InsertionError::Blocked(_))
        ));
    }

    #[test]
    fn a_new_insertion_restores_the_waiting_clipboard_before_saving() {
        let rig = rig(Some("previous"));
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("one").unwrap();
        inserter.insert("two").unwrap();
        inserter.wait_for_restore();

        let journal: Vec<String> = probe
            .journal()
            .into_iter()
            .filter(|e| e.contains("save") || e.contains("restore"))
            .map(|e| e.split_once(' ').unwrap().1.to_owned())
            .collect();
        assert_eq!(
            journal,
            vec![
                "save Some(\"previous\")",
                "restore Some(\"previous\")",
                "save Some(\"previous\")",
                "restore Some(\"previous\")",
            ]
        );
    }

    #[test]
    fn dropping_the_inserter_restores_a_waiting_clipboard() {
        let rig = rig(Some("previous"));
        let (mut inserter, probe) = rig.inserter();

        inserter.insert("one").unwrap();
        drop(inserter);

        assert_eq!(probe.clipboard.content().as_deref(), Some("previous"));
    }
}
