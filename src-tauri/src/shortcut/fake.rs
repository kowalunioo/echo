use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use super::{
    KeyAction, KeyCombination, Shortcut, ShortcutError, ShortcutEvent, ShortcutListener,
    ShortcutSink,
};

/// A [`ShortcutListener`] whose presses and releases are injected by the test.
///
/// Like the real listener it only reports shortcuts that are currently bound, and a failed
/// [`bind`](ShortcutListener::bind) leaves the previous binding in place. Clones share state, so
/// a test keeps one clone to inject events while the code under test owns another.
#[derive(Clone, Default)]
pub struct FakeShortcutListener {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    sink: Option<ShortcutSink>,
    bindings: HashMap<Shortcut, KeyCombination>,
    reject_binds: Option<String>,
}

impl FakeShortcutListener {
    pub fn new() -> Self {
        Self::default()
    }

    /// Injects a press of `shortcut`. Returns whether it was delivered (the listener is started
    /// and the shortcut is bound).
    pub fn press(&self, shortcut: Shortcut) -> bool {
        self.inject(shortcut, KeyAction::Pressed)
    }

    /// Injects a release of `shortcut`. Returns whether it was delivered.
    pub fn release(&self, shortcut: Shortcut) -> bool {
        self.inject(shortcut, KeyAction::Released)
    }

    /// The combination currently bound to `shortcut`.
    pub fn binding(&self, shortcut: Shortcut) -> Option<KeyCombination> {
        self.state().bindings.get(&shortcut).cloned()
    }

    /// Makes later binds fail with `reason` (or succeed again with `None`), as when a
    /// combination cannot be activated.
    pub fn reject_binds(&self, reason: Option<&str>) {
        self.state().reject_binds = reason.map(str::to_owned);
    }

    fn inject(&self, shortcut: Shortcut, action: KeyAction) -> bool {
        // Take the sink out so it is not called with the lock held (it may call back into us).
        let mut sink = {
            let mut state = self.state();
            if !state.bindings.contains_key(&shortcut) {
                return false;
            }
            match state.sink.take() {
                Some(sink) => sink,
                None => return false,
            }
        };
        sink(ShortcutEvent { shortcut, action });
        self.state().sink.get_or_insert(sink);
        true
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl ShortcutListener for FakeShortcutListener {
    fn start(&mut self, sink: ShortcutSink) -> Result<(), ShortcutError> {
        self.state().sink = Some(sink);
        Ok(())
    }

    fn bind(
        &mut self,
        shortcut: Shortcut,
        combination: Option<KeyCombination>,
    ) -> Result<(), ShortcutError> {
        let mut state = self.state();
        if let Some(reason) = &state.reject_binds {
            return Err(ShortcutError::Rejected {
                combination: combination.map(|c| c.to_string()).unwrap_or_default(),
                reason: reason.clone(),
            });
        }
        match combination {
            Some(combination) => state.bindings.insert(shortcut, combination),
            None => state.bindings.remove(&shortcut),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::super::{Key, Modifiers};
    use super::*;

    fn ctrl_space() -> KeyCombination {
        KeyCombination {
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            key: Some(Key("Space".into())),
        }
    }

    fn started() -> (FakeShortcutListener, mpsc::Receiver<ShortcutEvent>) {
        let probe = FakeShortcutListener::new();
        let mut listener: Box<dyn ShortcutListener> = Box::new(probe.clone());
        let (tx, rx) = mpsc::channel();
        listener
            .start(Box::new(move |event| tx.send(event).unwrap()))
            .unwrap();
        listener.bind(Shortcut::Record, Some(ctrl_space())).unwrap();
        (probe, rx)
    }

    #[test]
    fn delivers_injected_presses_and_releases_of_bound_shortcuts() {
        let (probe, rx) = started();

        assert!(probe.press(Shortcut::Record));
        assert!(probe.release(Shortcut::Record));

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                ShortcutEvent {
                    shortcut: Shortcut::Record,
                    action: KeyAction::Pressed
                },
                ShortcutEvent {
                    shortcut: Shortcut::Record,
                    action: KeyAction::Released
                },
            ]
        );
    }

    #[test]
    fn unbound_shortcuts_are_not_reported() {
        let (probe, rx) = started();

        assert!(!probe.press(Shortcut::Cancel));
        assert_eq!(rx.try_iter().count(), 0);
    }

    #[test]
    fn a_rejected_bind_keeps_the_previous_binding() {
        let (probe, _rx) = started();
        let mut listener = probe.clone();
        probe.reject_binds(Some("reserved by Windows"));

        let result = listener.bind(Shortcut::Record, None);

        assert!(matches!(result, Err(ShortcutError::Rejected { .. })));
        assert_eq!(probe.binding(Shortcut::Record), Some(ctrl_space()));
    }

    #[test]
    fn clearing_a_binding_stops_its_events() {
        let (probe, rx) = started();
        let mut listener = probe.clone();

        listener.bind(Shortcut::Record, None).unwrap();

        assert!(!probe.press(Shortcut::Record));
        assert_eq!(rx.try_iter().count(), 0);
    }
}
