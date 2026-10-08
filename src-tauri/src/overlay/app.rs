//! The Overlay inside the Tauri app: runs the [`OverlayController`] on a thread of its own, drives
//! the `overlay` window, sends the frontend the view and ~30 level/timer frames per second while
//! recording, and offers the commands behind the cancel button and clickable messages.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_specta::Event;

use super::controller::{OverlayController, OverlaySettings, OverlaySurface};
#[cfg(windows)]
use super::placement::pill_region;
use super::placement::{self, Monitor, Rect};
use super::{MessageAction, OverlayPosition, OverlayView};
use crate::dictation::{Dictation, DictationStatus};
use crate::settings::{Settings, SettingsStore};
use crate::window::{MAIN_WINDOW, show_main};

/// The Overlay window's label (declared in `tauri.conf.json`).
pub const OVERLAY_WINDOW: &str = "overlay";
/// About 30 level-meter frames per second (rule 3).
const FRAME: Duration = Duration::from_millis(33);
/// How often a visible Overlay re-asserts that it is topmost (rule 12).
const KEEP_ON_TOP: Duration = Duration::from_secs(1);

/// What the Overlay shows changed.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct OverlayViewChanged(pub OverlayView);

/// One level-meter and timer frame while recording.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct OverlayFrame {
    /// The loudest input since the last frame, `0..=1` on a decibel scale.
    pub level: f32,
    /// Time since the Recording was requested.
    pub elapsed_ms: u32,
}

/// The main-window sections, for requests to show one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MainPage {
    Dictation,
    Model,
    Vocabulary,
    History,
    App,
}

/// Asks the main window to switch to a page (e.g. Models, after a "No Model" message is clicked).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct MainPageRequested(pub MainPage);

enum Msg {
    Status(DictationStatus),
    MicrophoneFallback,
    Settings(OverlaySettings),
    MessageClicked,
}

/// Handle to the running Overlay. Clones share it.
#[derive(Clone)]
pub struct Overlay {
    tx: Sender<Msg>,
    view: Arc<Mutex<OverlayView>>,
    pill: PillSize,
}

/// The pill's last reported size in CSS pixels, to clip the window again whenever it is shown at
/// another size or scaling (rule 11).
type PillSize = Arc<Mutex<Option<(f64, f64)>>>;

impl Overlay {
    /// The dictation status changed.
    pub fn status(&self, status: &DictationStatus) {
        let _ = self.tx.send(Msg::Status(status.clone()));
    }

    /// The selected Microphone was missing; the starting Recording uses the default one.
    pub fn microphone_fallback(&self) {
        let _ = self.tx.send(Msg::MicrophoneFallback);
    }

    fn view(&self) -> OverlayView {
        self.view
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

fn overlay_settings(settings: &Settings) -> OverlaySettings {
    OverlaySettings {
        show: settings.show_overlay,
        position: settings.overlay_position,
    }
}

/// Starts the Overlay. Needs the settings managed already; install it before the Dictation so
/// the Dictation can feed it.
pub fn install(app: &AppHandle) {
    let window = app.get_webview_window(OVERLAY_WINDOW);
    match &window {
        Some(window) => prepare(window),
        None => log::error!("the Overlay window is missing; the Overlay will not show"),
    }

    let (tx, rx) = mpsc::channel();
    let view = Arc::new(Mutex::new(OverlayView::Hidden));
    let store = app.state::<SettingsStore>();
    let settings = overlay_settings(&store.get());
    let listener = tx.clone();
    store.subscribe(move |old, new| {
        let (old, new) = (overlay_settings(old), overlay_settings(new));
        if old != new {
            let _ = listener.send(Msg::Settings(new));
        }
    });

    let pill = PillSize::default();
    let surface = WindowSurface {
        app: app.clone(),
        window,
        view: Arc::clone(&view),
        pill: Arc::clone(&pill),
    };
    let controller = OverlayController::new(surface, settings, &DictationStatus::default());
    let thread_app = app.clone();
    let preview = cfg!(debug_assertions) && std::env::var_os("ECHO_OVERLAY_PREVIEW").is_some();
    std::thread::Builder::new()
        .name("echo-overlay".into())
        .spawn(move || run(controller, rx, thread_app, preview))
        .expect("spawn the Overlay thread");
    if preview {
        spawn_preview(tx.clone());
    }
    app.manage(Overlay { tx, view, pill });
}

/// Developer aid, debug builds only: with `ECHO_OVERLAY_PREVIEW` set, the Overlay cycles through
/// its states with a synthetic level, to check its look without speaking a Dictation.
fn spawn_preview(tx: Sender<Msg>) {
    use crate::dictation::{DictationProblem, DictationState, ProblemKind};
    log::warn!("ECHO_OVERLAY_PREVIEW is set: the Overlay cycles through its states");
    std::thread::spawn(move || {
        let mut status = DictationStatus::default();
        let send = |status: &DictationStatus, seconds: f32| {
            let _ = tx.send(Msg::Status(status.clone()));
            std::thread::sleep(Duration::from_secs_f32(seconds));
        };
        for id in 1..=u32::MAX {
            status.state = DictationState::Recording;
            send(&status, 4.0);
            status.listening = true;
            send(&status, 5.0);
            status.listening = false;
            status.state = DictationState::Transcribing;
            send(&status, 4.0);
            status.state = DictationState::Inserting;
            send(&status, 1.0);
            status.state = DictationState::Idle;
            send(&status, 2.0);
            let problem = DictationProblem {
                id,
                kind: ProblemKind::NoModel,
                detail: String::new(),
            };
            status.error = Some(problem.clone());
            status.notices = vec![problem];
            send(&status, 5.0);
        }
    });
}

fn run(
    mut controller: OverlayController<WindowSurface>,
    rx: Receiver<Msg>,
    app: AppHandle,
    preview: bool,
) {
    let mut last_frame = Instant::now();
    let mut last_top = Instant::now();
    loop {
        let mut wake = controller.next_deadline();
        if controller.view().is_recording() {
            wake = Some(wake.map_or(last_frame + FRAME, |at| at.min(last_frame + FRAME)));
        }
        if controller.window_visible() {
            wake = Some(wake.map_or(last_top + KEEP_ON_TOP, |at| at.min(last_top + KEEP_ON_TOP)));
        }
        let msg = match wake {
            Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let now = Instant::now();
        match msg {
            Ok(Msg::Status(status)) => controller.status(&status, now),
            Ok(Msg::MicrophoneFallback) => controller.microphone_fallback(now),
            Ok(Msg::Settings(settings)) => controller.settings(settings, now),
            Ok(Msg::MessageClicked) => {
                if let Some(action) = controller.message_clicked(now) {
                    perform(&app, action);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        controller.tick(now);

        if controller.view().is_recording() && now >= last_frame + FRAME {
            last_frame = now;
            let elapsed = controller.recording_elapsed(now).unwrap_or_default();
            let level = if preview {
                let t = elapsed.as_secs_f32();
                (0.5 + 0.45 * (t * 5.3).sin() * (t * 1.7).cos()).clamp(0.0, 1.0)
            } else {
                app.try_state::<Dictation>()
                    .map_or(0.0, |d| d.take_input_level())
            };
            let frame = OverlayFrame {
                level,
                elapsed_ms: u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX),
            };
            let _ = frame.emit_to(&app, OVERLAY_WINDOW);
        }
        if controller.window_visible() && now >= last_top + KEEP_ON_TOP {
            last_top = now;
            controller.surface().keep_on_top();
        }
    }
}

fn perform(app: &AppHandle, action: MessageAction) {
    match action {
        MessageAction::OpenModels => {
            show_main(app);
            if let Err(error) = MainPageRequested(MainPage::Model).emit_to(app, MAIN_WINDOW) {
                log::warn!("could not open the Models page: {error}");
            }
        }
        MessageAction::OpenMicrophonePrivacy => {
            if let Err(error) = crate::system::open_microphone_privacy_settings(app.clone()) {
                log::warn!("could not open the microphone privacy settings: {error}");
            }
        }
        MessageAction::ShowNotices => show_main(app),
    }
}

/// The real Overlay window.
struct WindowSurface {
    app: AppHandle,
    window: Option<WebviewWindow>,
    view: Arc<Mutex<OverlayView>>,
    pill: PillSize,
}

impl WindowSurface {
    fn keep_on_top(&self) {
        self.on_window(|_window| {
            #[cfg(windows)]
            if let Ok(hwnd) = _window.hwnd() {
                super::native::keep_on_top(hwnd);
            }
        });
    }

    /// Runs `f` with the window on the UI thread, which owns it.
    fn on_window(&self, f: impl FnOnce(&WebviewWindow) + Send + 'static) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if let Err(error) = self.app.run_on_main_thread(move || f(&window)) {
            log::warn!("Overlay window update failed: {error}");
        }
    }

    /// Where the window goes now: the monitor under the pointer (rule 13); and the factor its
    /// content is drawn at there (monitor scaling × "Text size").
    fn placement(&self, window: &WebviewWindow, position: OverlayPosition) -> Option<(Rect, f64)> {
        let pointer = window.cursor_position().ok()?;
        let monitors: Vec<Monitor> = window
            .available_monitors()
            .ok()?
            .iter()
            .map(|m| Monitor {
                bounds: Rect {
                    x: m.position().x,
                    y: m.position().y,
                    width: m.size().width,
                    height: m.size().height,
                },
                work_area: Rect {
                    x: m.work_area().position.x,
                    y: m.work_area().position.y,
                    width: m.work_area().size.width,
                    height: m.work_area().size.height,
                },
                scale: m.scale_factor(),
            })
            .collect();
        let monitor = placement::monitor_at(&monitors, (pointer.x as i32, pointer.y as i32))?;
        let text_scale = text_scale();
        Some((
            placement::place(&monitor, position, text_scale),
            monitor.scale * text_scale,
        ))
    }
}

impl OverlaySurface for WindowSurface {
    fn render(&mut self, view: &OverlayView) {
        *self.view.lock().unwrap_or_else(PoisonError::into_inner) = view.clone();
        if let Err(error) = OverlayViewChanged(view.clone()).emit_to(&self.app, OVERLAY_WINDOW) {
            log::warn!("could not update the Overlay: {error}");
        }
    }

    fn show(&mut self, position: OverlayPosition) {
        let Some(window) = &self.window else {
            return;
        };
        let Some((rect, factor)) = self.placement(window, position) else {
            log::warn!("no monitor to show the Overlay on");
            return;
        };
        // Rule 15: the Windows "Text size" setting enlarges the Overlay's content too.
        let _ = window.set_zoom(text_scale());
        let pill = Arc::clone(&self.pill);
        self.on_window(move |_window| {
            #[cfg(windows)]
            if let Ok(hwnd) = _window.hwnd() {
                // The pill was last measured at another size or scaling, perhaps: clip the
                // window for the size it is shown at now (rule 11).
                let region = pill
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .map(|size| pill_region(size, factor, (rect.width, rect.height)));
                super::native::show(hwnd, rect, region);
            }
            #[cfg(not(windows))]
            let _ = (rect, factor, pill);
        });
    }

    fn hide(&mut self) {
        self.on_window(|_window| {
            #[cfg(windows)]
            if let Ok(hwnd) = _window.hwnd() {
                super::native::hide(hwnd);
            }
        });
    }
}

/// Makes the window an Overlay: never activated, no taskbar button, not in Alt+Tab, topmost.
fn prepare(window: &WebviewWindow) {
    #[cfg(windows)]
    match window.hwnd() {
        Ok(hwnd) => super::native::prepare(hwnd),
        Err(error) => log::error!("cannot prepare the Overlay window: {error}"),
    }
    let _ = window.set_zoom(text_scale());
}

/// The Windows "Text size" factor (1.0 at 100 %).
fn text_scale() -> f64 {
    #[cfg(windows)]
    return super::native::text_scale();
    #[cfg(not(windows))]
    1.0
}

/// What the Overlay shows now, for its window when it loads.
#[tauri::command]
#[specta::specta]
pub fn get_overlay_view(overlay: State<'_, Overlay>) -> OverlayView {
    overlay.view()
}

/// The Overlay's cancel button: the same Cancellation as the Cancel Shortcut (rule 10).
#[tauri::command]
#[specta::specta]
pub fn overlay_cancel(dictation: State<'_, Dictation>) {
    dictation.cancel();
}

/// The user clicked the Overlay's message (rule 5).
#[tauri::command]
#[specta::specta]
pub fn overlay_message_clicked(overlay: State<'_, Overlay>) {
    let _ = overlay.tx.send(Msg::MessageClicked);
}

/// The pill's size in CSS pixels changed: only the pill takes clicks; beside it they go to the
/// windows beneath (rule 11).
#[tauri::command]
#[specta::specta]
pub fn overlay_shape(overlay: State<'_, Overlay>, window: WebviewWindow, width: f64, height: f64) {
    *overlay.pill.lock().unwrap_or_else(PoisonError::into_inner) = Some((width, height));
    #[cfg(windows)]
    if let (Ok(hwnd), Ok(scale), Ok(size)) =
        (window.hwnd(), window.scale_factor(), window.outer_size())
    {
        let region = pill_region(
            (width, height),
            scale * text_scale(),
            (size.width, size.height),
        );
        super::native::set_shape(hwnd, region);
    }
    #[cfg(not(windows))]
    let _ = window;
}
