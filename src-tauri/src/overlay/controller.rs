//! What the Overlay shows and when its window is visible (`overlay.md` rules 1–7, 16, 17), as pure
//! logic. Inputs are the dictation status, Microphone notices, the two Overlay settings and the
//! passage of time (every method takes `now`, so tests drive the clock); outputs go to an
//! [`OverlaySurface`].

use std::time::{Duration, Instant};

use super::{MessageAction, OverlayMessage, OverlayPosition, OverlayView};
use crate::dictation::{DictationState, DictationStatus};

/// How long a message stays (rule 5).
pub const MESSAGE_DURATION: Duration = Duration::from_millis(2500);
/// How long the fade-out lasts before the window is really hidden (rule 6).
pub const FADE_OUT: Duration = Duration::from_millis(300);

/// The window the controller drives: the real Overlay window in the app, a recorder in tests.
pub trait OverlaySurface {
    /// The Overlay should now draw `view`. A [`OverlayView::Hidden`] starts the fade-out; the
    /// window itself stays up until [`hide`](Self::hide).
    fn render(&mut self, view: &OverlayView);
    /// Places the window on the monitor under the mouse pointer and shows it without taking
    /// keyboard focus. Also called while visible when the position setting changes (rule 16).
    fn show(&mut self, position: OverlayPosition);
    /// Hides the window once the fade-out has finished.
    fn hide(&mut self);
}

/// The two Overlay settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlaySettings {
    /// "Show Overlay" (rule 17).
    pub show: bool,
    pub position: OverlayPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Window {
    Hidden,
    Shown,
    /// Fading out; hidden at this instant unless something is shown again first.
    Fading(Instant),
}

#[derive(Debug, Clone)]
struct Message {
    message: OverlayMessage,
    until: Instant,
    /// A Microphone notice arrives while its own Recording is starting; that start must not
    /// replace it the way a later Recording does.
    survives_next_start: bool,
}

pub struct OverlayController<S> {
    surface: S,
    settings: OverlaySettings,
    state: DictationState,
    listening: bool,
    /// The newest error already shown (or present at start-up).
    last_problem: u32,
    message: Option<Message>,
    recording_since: Option<Instant>,
    view: OverlayView,
    window: Window,
}

impl<S: OverlaySurface> OverlayController<S> {
    /// Starts hidden. Errors already in `status` are not shown again.
    pub fn new(surface: S, settings: OverlaySettings, status: &DictationStatus) -> Self {
        Self {
            surface,
            settings,
            state: DictationState::Idle,
            listening: false,
            last_problem: newest_problem(status).map_or(0, |(id, _)| id),
            message: None,
            recording_since: None,
            view: OverlayView::Hidden,
            window: Window::Hidden,
        }
    }

    /// The dictation status changed.
    pub fn status(&mut self, status: &DictationStatus, now: Instant) {
        let recording = status.state == DictationState::Recording;
        if recording && self.state != DictationState::Recording {
            self.recording_since = Some(now);
            // A new Recording takes over the Overlay from a message (rule 5).
            match &mut self.message {
                Some(m) if m.survives_next_start => m.survives_next_start = false,
                _ => self.message = None,
            }
        }
        if !recording {
            self.recording_since = None;
        }
        self.state = status.state;
        self.listening = status.listening;

        if let Some((id, problem)) = newest_problem(status)
            && id > self.last_problem
        {
            self.last_problem = id;
            self.message = Some(Message {
                message: OverlayMessage::Problem { problem },
                until: now + MESSAGE_DURATION,
                survives_next_start: false,
            });
        }
        self.apply(now);
    }

    /// The selected Microphone was missing, so the Recording that is starting uses the default
    /// one (`microphone.md` rule 5).
    pub fn microphone_fallback(&mut self, now: Instant) {
        self.message = Some(Message {
            message: OverlayMessage::MicrophoneFallback,
            until: now + MESSAGE_DURATION,
            survives_next_start: self.state != DictationState::Recording,
        });
        self.apply(now);
    }

    /// The Overlay settings changed.
    pub fn settings(&mut self, settings: OverlaySettings, now: Instant) {
        let moved = settings.position != self.settings.position;
        self.settings = settings;
        if moved && self.window != Window::Hidden {
            self.surface.show(settings.position);
        }
        self.apply(now);
    }

    /// Time passed: messages expire and finished fade-outs hide the window.
    pub fn tick(&mut self, now: Instant) {
        if self.message.as_ref().is_some_and(|m| m.until <= now) {
            self.message = None;
        }
        self.apply(now);
        if let Window::Fading(at) = self.window
            && at <= now
        {
            self.window = Window::Hidden;
            self.surface.hide();
        }
    }

    /// The user clicked the message: it goes away, and its action (if any) is returned.
    pub fn message_clicked(&mut self, now: Instant) -> Option<MessageAction> {
        let OverlayView::Message { message, .. } = &self.view else {
            return None;
        };
        let action = message.action();
        self.message = None;
        self.apply(now);
        action
    }

    /// When [`tick`](Self::tick) next has something to do.
    pub fn next_deadline(&self) -> Option<Instant> {
        let fade = match self.window {
            Window::Fading(at) => Some(at),
            _ => None,
        };
        let message = self.message.as_ref().map(|m| m.until);
        fade.into_iter().chain(message).min()
    }

    /// How long the current Recording has run, for the timer (rule 3).
    pub fn recording_elapsed(&self, now: Instant) -> Option<Duration> {
        self.recording_since
            .map(|since| now.saturating_duration_since(since))
    }

    pub fn view(&self) -> &OverlayView {
        &self.view
    }

    /// Whether the window is up (including while it fades out).
    pub fn window_visible(&self) -> bool {
        self.window != Window::Hidden
    }

    pub fn surface(&self) -> &S {
        &self.surface
    }

    fn desired(&self, now: Instant) -> OverlayView {
        if let Some(m) = &self.message
            && m.until > now
            && (self.settings.show || m.message.is_error())
        {
            return OverlayView::Message {
                actionable: m.message.action().is_some(),
                message: m.message.clone(),
            };
        }
        if !self.settings.show {
            return OverlayView::Hidden;
        }
        match self.state {
            DictationState::Idle => OverlayView::Hidden,
            DictationState::Recording if self.listening => OverlayView::Listening,
            DictationState::Recording => OverlayView::GettingReady,
            DictationState::Transcribing => OverlayView::Transcribing { cancellable: true },
            DictationState::Inserting => OverlayView::Transcribing { cancellable: false },
        }
    }

    fn apply(&mut self, now: Instant) {
        let desired = self.desired(now);
        if desired != self.view {
            self.view = desired;
            self.surface.render(&self.view);
        }
        match (self.view.is_hidden(), self.window) {
            (false, Window::Hidden) => {
                self.window = Window::Shown;
                self.surface.show(self.settings.position);
            }
            // Shown again during the fade-out: the pending hide is stale (rule 6).
            (false, Window::Fading(_)) => self.window = Window::Shown,
            (true, Window::Shown) => self.window = Window::Fading(now + FADE_OUT),
            _ => {}
        }
    }
}

/// The newest error in the status: its id and kind.
fn newest_problem(status: &DictationStatus) -> Option<(u32, crate::dictation::ProblemKind)> {
    status
        .error
        .iter()
        .chain(&status.notices)
        .max_by_key(|p| p.id)
        .map(|p| (p.id, p.kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictation::{DictationProblem, ProblemKind};

    /// Everything the controller did to the window, in order.
    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Render(OverlayView),
        Show(OverlayPosition),
        Hide,
    }

    #[derive(Default)]
    struct FakeWindow {
        calls: Vec<Call>,
        visible: bool,
    }

    impl OverlaySurface for FakeWindow {
        fn render(&mut self, view: &OverlayView) {
            self.calls.push(Call::Render(view.clone()));
        }
        fn show(&mut self, position: OverlayPosition) {
            self.visible = true;
            self.calls.push(Call::Show(position));
        }
        fn hide(&mut self) {
            self.visible = false;
            self.calls.push(Call::Hide);
        }
    }

    const ON: OverlaySettings = OverlaySettings {
        show: true,
        position: OverlayPosition::Bottom,
    };
    const OFF: OverlaySettings = OverlaySettings {
        show: false,
        position: OverlayPosition::Bottom,
    };

    struct Rig {
        c: OverlayController<FakeWindow>,
        t0: Instant,
        status: DictationStatus,
        next_id: u32,
    }

    impl Rig {
        fn new(settings: OverlaySettings) -> Self {
            Self {
                c: OverlayController::new(
                    FakeWindow::default(),
                    settings,
                    &DictationStatus::default(),
                ),
                t0: Instant::now(),
                status: DictationStatus::default(),
                next_id: 1,
            }
        }
        fn at(&self, ms: u64) -> Instant {
            self.t0 + Duration::from_millis(ms)
        }
        fn state(&mut self, ms: u64, state: DictationState, listening: bool) {
            self.status.state = state;
            self.status.listening = listening;
            let now = self.at(ms);
            self.c.status(&self.status.clone(), now);
        }
        fn error(&mut self, ms: u64, kind: ProblemKind) {
            let problem = DictationProblem {
                id: self.next_id,
                kind,
                detail: String::new(),
            };
            self.next_id += 1;
            self.status.error = Some(problem.clone());
            self.status.notices.push(problem);
            let now = self.at(ms);
            self.c.status(&self.status.clone(), now);
        }
        fn tick(&mut self, ms: u64) {
            let now = self.at(ms);
            self.c.tick(now);
        }
        fn view(&self) -> &OverlayView {
            self.c.view()
        }
        fn visible(&self) -> bool {
            self.c.surface().visible
        }
        fn calls(&self) -> &[Call] {
            &self.c.surface().calls
        }
    }

    fn message(problem: ProblemKind, actionable: bool) -> OverlayView {
        OverlayView::Message {
            message: OverlayMessage::Problem { problem },
            actionable,
        }
    }

    // Acceptance test 1 and rules 1-4, 6.
    #[test]
    fn a_dictation_goes_hidden_getting_ready_listening_transcribing_hidden() {
        use DictationState::*;
        let mut r = Rig::new(ON);
        assert!(!r.visible());
        r.state(0, Recording, false);
        r.state(1000, Recording, true);
        r.state(3000, Transcribing, false);
        r.state(4000, Inserting, false);
        r.state(4100, Idle, false);
        assert!(r.visible(), "the window stays up while it fades out");
        r.tick(4399);
        assert!(r.visible());
        r.tick(4400);
        assert!(!r.visible());
        assert_eq!(
            r.calls(),
            &[
                Call::Render(OverlayView::GettingReady),
                Call::Show(OverlayPosition::Bottom),
                Call::Render(OverlayView::Listening),
                Call::Render(OverlayView::Transcribing { cancellable: true }),
                Call::Render(OverlayView::Transcribing { cancellable: false }),
                Call::Render(OverlayView::Hidden),
                Call::Hide,
            ]
        );
    }

    // Acceptance test 2: getting ready lasts until the first audio, however long that takes.
    #[test]
    fn getting_ready_lasts_until_the_first_audio() {
        let mut r = Rig::new(ON);
        r.state(0, DictationState::Recording, false);
        r.tick(1000);
        assert_eq!(r.view(), &OverlayView::GettingReady);
        r.state(1000, DictationState::Recording, true);
        assert_eq!(r.view(), &OverlayView::Listening);
    }

    // Acceptance test 8 and rule 6.
    #[test]
    fn a_stale_hide_never_hides_a_newer_dictation() {
        let mut r = Rig::new(ON);
        r.state(0, DictationState::Recording, true);
        r.state(500, DictationState::Idle, false);
        r.state(600, DictationState::Recording, false);
        assert_eq!(r.view(), &OverlayView::GettingReady);
        r.tick(800);
        r.tick(5000);
        assert!(r.visible());
        assert!(!r.calls().contains(&Call::Hide));
        assert_eq!(
            r.calls()
                .iter()
                .filter(|c| matches!(c, Call::Show(_)))
                .count(),
            1,
            "the window is not placed again while it is still up"
        );
    }

    // Acceptance test 9 and rule 5.
    #[test]
    fn a_message_shows_for_two_and_a_half_seconds() {
        let mut r = Rig::new(ON);
        r.error(0, ProblemKind::TranscriptionFailed);
        assert_eq!(r.view(), &message(ProblemKind::TranscriptionFailed, false));
        assert_eq!(r.c.next_deadline(), Some(r.at(2500)));
        r.tick(2499);
        assert!(!r.view().is_hidden());
        r.tick(2500);
        assert!(r.view().is_hidden());
        r.tick(2800);
        assert!(!r.visible());
    }

    // Acceptance test 9: a Recording starting at 1 s replaces the message.
    #[test]
    fn a_new_recording_replaces_a_message() {
        let mut r = Rig::new(ON);
        r.error(0, ProblemKind::NoModel);
        r.state(1000, DictationState::Recording, false);
        assert_eq!(r.view(), &OverlayView::GettingReady);
        r.tick(2600);
        assert_eq!(r.view(), &OverlayView::GettingReady);
    }

    #[test]
    fn an_error_that_ends_a_dictation_is_shown_then_the_overlay_hides() {
        let mut r = Rig::new(ON);
        r.state(0, DictationState::Recording, true);
        r.state(1000, DictationState::Transcribing, false);
        // Coalesced: the failure and the return to Idle arrive together.
        r.status.state = DictationState::Idle;
        r.error(2000, ProblemKind::TranscriptionFailed);
        assert_eq!(r.view(), &message(ProblemKind::TranscriptionFailed, false));
        r.tick(4500);
        assert!(r.view().is_hidden());
        r.tick(4800);
        assert!(!r.visible());
    }

    #[test]
    fn an_error_during_a_dictation_shows_then_the_dictation_view_returns() {
        let mut r = Rig::new(ON);
        r.state(0, DictationState::Recording, true);
        r.status.state = DictationState::Transcribing;
        r.error(1000, ProblemKind::MicrophoneDisconnected);
        assert_eq!(
            r.view(),
            &message(ProblemKind::MicrophoneDisconnected, false)
        );
        r.tick(3500);
        assert_eq!(r.view(), &OverlayView::Transcribing { cancellable: true });
    }

    #[test]
    fn errors_present_at_start_up_are_not_shown_again() {
        let mut status = DictationStatus::default();
        status.notices.push(DictationProblem {
            id: 7,
            kind: ProblemKind::NoModel,
            detail: String::new(),
        });
        let mut c = OverlayController::new(FakeWindow::default(), ON, &status);
        c.status(&status, Instant::now());
        assert!(c.view().is_hidden());
        assert!(c.surface().calls.is_empty());
    }

    #[test]
    fn dismissing_notices_does_not_replay_or_hide_errors() {
        let mut r = Rig::new(ON);
        r.error(0, ProblemKind::NoModel);
        r.status.notices.clear();
        r.status.error = None;
        let status = r.status.clone();
        r.c.status(&status, r.at(100));
        assert_eq!(r.view(), &message(ProblemKind::NoModel, true));
        r.error(200, ProblemKind::InsertionFailed);
        assert_eq!(r.view(), &message(ProblemKind::InsertionFailed, true));
    }

    // microphone.md rule 5: the notice arrives while its own Recording starts.
    #[test]
    fn the_microphone_fallback_notice_survives_its_own_recording_start() {
        let mut r = Rig::new(ON);
        let now = r.at(0);
        r.c.microphone_fallback(now);
        r.state(10, DictationState::Recording, false);
        assert_eq!(
            r.view(),
            &OverlayView::Message {
                message: OverlayMessage::MicrophoneFallback,
                actionable: false
            }
        );
        r.state(500, DictationState::Recording, true);
        r.tick(2500);
        assert_eq!(r.view(), &OverlayView::Listening);
        // A later Recording would replace it.
        r.state(3000, DictationState::Idle, false);
        let now = r.at(4000);
        r.c.microphone_fallback(now);
        r.state(4010, DictationState::Recording, false);
        r.state(4500, DictationState::Idle, false);
        r.state(5000, DictationState::Recording, false);
        assert_eq!(r.view(), &OverlayView::GettingReady);
    }

    // Acceptance test 10 and rule 17.
    #[test]
    fn with_the_overlay_off_only_errors_are_shown() {
        let mut r = Rig::new(OFF);
        r.state(0, DictationState::Recording, false);
        r.state(500, DictationState::Recording, true);
        let now = r.at(600);
        r.c.microphone_fallback(now);
        r.state(1000, DictationState::Transcribing, false);
        r.state(2000, DictationState::Idle, false);
        assert!(r.calls().is_empty(), "nothing shown: {:?}", r.calls());
        r.error(3000, ProblemKind::TranscriptionFailed);
        assert_eq!(r.view(), &message(ProblemKind::TranscriptionFailed, false));
        assert!(r.visible());
    }

    #[test]
    fn turning_the_overlay_off_hides_it_and_on_shows_it() {
        let mut r = Rig::new(ON);
        r.state(0, DictationState::Recording, true);
        let now = r.at(100);
        r.c.settings(OFF, now);
        assert!(r.view().is_hidden());
        r.tick(400);
        assert!(!r.visible());
        let now = r.at(500);
        r.c.settings(ON, now);
        assert_eq!(r.view(), &OverlayView::Listening);
        assert!(r.visible());
    }

    // Rule 16.
    #[test]
    fn changing_the_position_moves_a_visible_overlay_at_once() {
        let mut r = Rig::new(ON);
        let top = OverlaySettings {
            position: OverlayPosition::Top,
            ..ON
        };
        let now = r.at(0);
        r.c.settings(top, now);
        assert!(r.calls().is_empty(), "a hidden Overlay is not shown");
        r.state(100, DictationState::Recording, true);
        let now = r.at(200);
        r.c.settings(ON, now);
        assert_eq!(
            r.calls()
                .iter()
                .filter(|c| matches!(c, Call::Show(_)))
                .collect::<Vec<_>>(),
            [
                &Call::Show(OverlayPosition::Top),
                &Call::Show(OverlayPosition::Bottom)
            ]
        );
    }

    #[test]
    fn clicking_a_message_returns_its_action_and_dismisses_it() {
        let mut r = Rig::new(ON);
        r.error(0, ProblemKind::NoModel);
        assert_eq!(
            r.c.message_clicked(r.at(100)),
            Some(MessageAction::OpenModels)
        );
        assert!(r.view().is_hidden());
        r.error(200, ProblemKind::MicrophoneAccessDenied);
        assert_eq!(
            r.c.message_clicked(r.at(300)),
            Some(MessageAction::OpenMicrophonePrivacy)
        );
        r.error(400, ProblemKind::TranscriptionFailed);
        assert_eq!(r.c.message_clicked(r.at(500)), None);
        r.error(520, ProblemKind::InsertionFailedNotInHistory);
        assert_eq!(
            r.c.message_clicked(r.at(540)),
            Some(MessageAction::ShowNotices)
        );
        r.state(600, DictationState::Recording, true);
        assert_eq!(r.c.message_clicked(r.at(700)), None);
        assert_eq!(r.view(), &OverlayView::Listening);
    }

    #[test]
    fn the_timer_counts_from_the_recording_request() {
        let mut r = Rig::new(ON);
        assert_eq!(r.c.recording_elapsed(r.at(0)), None);
        r.state(1000, DictationState::Recording, false);
        r.state(1800, DictationState::Recording, true);
        assert_eq!(
            r.c.recording_elapsed(r.at(4000)),
            Some(Duration::from_millis(3000))
        );
        r.state(5000, DictationState::Transcribing, false);
        assert_eq!(r.c.recording_elapsed(r.at(5000)), None);
    }
}
