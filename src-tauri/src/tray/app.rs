//! The tray inside the Tauri app: the real notification-area icon, its menu events,
//! close-to-tray with the one-time hint (rules 12–13), and the feeds that keep it current.
//!
//! Every input reaches one tray thread as a [`TrayUpdate`], so updates apply in order and no
//! caller waits on the tray: the Dictation worker, the settings, History and the Model manager
//! only send a message.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Window, WindowEvent, Wry};
use tauri_specta::Event;

use super::actions::{ActionTarget, perform};
use super::theme::{SystemTheme, ThemeWatcher};
use super::{
    MenuEntry, MenuModels, TaskbarTheme, TrayAction, TrayController, TrayIconState, TrayInputs,
    TrayUpdate, TrayView, icons,
};
use crate::dictation::{Dictation, DictationStatus};
use crate::history::History;
use crate::models::{ModelId, ModelManager};
use crate::settings::SettingsStore;
use crate::window::{MAIN_WINDOW, show_main};

/// The main window was closed for the first time: show the "still running in the tray" hint,
/// then call [`close_to_tray`] (rule 13).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct TrayHintRequested;

/// Sends updates to the tray thread.
struct TrayHandle(Mutex<Sender<TrayUpdate>>);

impl TrayHandle {
    fn send(&self, update: TrayUpdate) {
        let _ = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(update);
    }
}

fn send(app: &AppHandle, update: TrayUpdate) {
    if let Some(handle) = app.try_state::<TrayHandle>() {
        handle.send(update);
    }
}

/// The Dictation published a new status (called by the Dictation's publisher).
pub fn dictation_status_changed(app: &AppHandle, status: &DictationStatus) {
    send(app, TrayUpdate::Status(status.clone()));
}

/// Shows the tray icon. Needs the settings, History, the Model manager and the Dictation
/// managed already.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let (updates, inbox) = mpsc::channel();
    // Managed before the first state is read, so no change in between is lost.
    app.manage(TrayHandle(Mutex::new(updates)));

    let settings = app.state::<SettingsStore>();
    let watcher = ThemeWatcher::new(SystemTheme);
    let inputs = TrayInputs {
        status: app.state::<Dictation>().status(),
        theme: watcher.current(),
        language: settings.get().ui_language,
        models: MenuModels::from_state(&app.state::<ModelManager>().state()),
        history_empty: app
            .state::<History>()
            .latest()
            .map_or(true, |entry| entry.is_none()),
    };
    let size = Arc::new(AtomicU32::new(icon_size(app)));
    let tray = TrayIconBuilder::with_id("echo")
        .icon(glyph(
            TrayIconState::Idle,
            inputs.theme,
            size.load(Ordering::Relaxed),
        ))
        .tooltip(format!("Echo {}", version(app)))
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if let Some(action) = TrayAction::from_id(event.id().as_ref()) {
                if let TrayAction::ActivateModel(_) = action {
                    // The check mark toggled on click; the menu shows the real state again.
                    send(app, TrayUpdate::Refresh);
                }
                perform(action, &AppTarget(app));
            }
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => show_main(tray.app_handle()),
            _ => {}
        })
        .build(app)?;

    let view = TauriTray {
        app: app.clone(),
        tray,
        size: Arc::clone(&size),
    };
    let version = version(app);
    std::thread::Builder::new()
        .name("echo-tray".into())
        .spawn(move || {
            let mut controller = TrayController::new(view, version, inputs);
            for update in inbox {
                controller.apply(update);
            }
        })?;

    follow_inputs(app);
    watch_theme_and_scale(app, watcher, size);
    Ok(())
}

fn version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Forwards changes of the UI Language, History and the Models to the tray thread.
fn follow_inputs(app: &AppHandle) {
    let handle = app.clone();
    app.state::<SettingsStore>().subscribe(move |old, new| {
        if old.ui_language != new.ui_language {
            send(&handle, TrayUpdate::Language(new.ui_language));
        }
    });
    let handle = app.clone();
    app.state::<History>().subscribe(move |entries| {
        send(&handle, TrayUpdate::HistoryEmpty(entries.is_empty()));
    });
    let handle = app.clone();
    app.state::<ModelManager>().subscribe(move |state| {
        send(&handle, TrayUpdate::Models(MenuModels::from_state(state)));
    });
}

/// Polls the taskbar theme and the display scale once a second (rule 3).
fn watch_theme_and_scale(
    app: &AppHandle,
    mut watcher: ThemeWatcher<SystemTheme>,
    size: Arc<AtomicU32>,
) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("echo-tray-theme".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                if let Some(theme) = watcher.poll() {
                    send(&app, TrayUpdate::Theme(theme));
                }
                let now = icon_size(&app);
                if size.swap(now, Ordering::Relaxed) != now {
                    send(&app, TrayUpdate::Refresh);
                }
            }
        });
    if let Err(error) = spawned {
        log::error!("the tray will not follow theme changes: {error}");
    }
}

/// The glyph size for the primary display, where the taskbar's notification area is.
fn icon_size(app: &AppHandle) -> u32 {
    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map_or(1.0, |monitor| monitor.scale_factor());
    icons::size_for_scale(scale)
}

fn glyph(state: TrayIconState, theme: TaskbarTheme, size: u32) -> Image<'static> {
    Image::from_bytes(icons::png(state, theme, size)).expect("the tray glyphs are valid PNGs")
}

/// The notification-area icon.
struct TauriTray {
    app: AppHandle,
    tray: TrayIcon,
    size: Arc<AtomicU32>,
}

impl TrayView for TauriTray {
    fn show_icon(&mut self, state: TrayIconState, theme: TaskbarTheme) {
        let image = glyph(state, theme, self.size.load(Ordering::Relaxed));
        if let Err(error) = self.tray.set_icon(Some(image)) {
            log::warn!("could not change the tray icon: {error}");
        }
    }

    fn show_tooltip(&mut self, text: &str) {
        if let Err(error) = self.tray.set_tooltip(Some(text)) {
            log::warn!("could not change the tray tooltip: {error}");
        }
    }

    fn show_menu(&mut self, entries: &[MenuEntry]) {
        match build_menu(&self.app, entries) {
            Ok(menu) => {
                if let Err(error) = self.tray.set_menu(Some(menu)) {
                    log::warn!("could not change the tray menu: {error}");
                }
            }
            Err(error) => log::warn!("could not build the tray menu: {error}"),
        }
    }
}

fn build_menu(app: &AppHandle, entries: &[MenuEntry]) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    for entry in entries {
        match entry {
            MenuEntry::Info(text) => {
                menu.append(&MenuItem::with_id(app, "info", text, false, None::<&str>)?)?
            }
            MenuEntry::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
            MenuEntry::Item {
                action,
                label,
                enabled,
            } => menu.append(&MenuItem::with_id(
                app,
                action.id(),
                label,
                *enabled,
                None::<&str>,
            )?)?,
            MenuEntry::Models {
                label,
                enabled,
                choices,
            } => {
                let submenu = Submenu::with_id(app, "models", label, *enabled)?;
                for choice in choices {
                    submenu.append(&CheckMenuItem::with_id(
                        app,
                        TrayAction::ActivateModel(choice.model).id(),
                        &choice.name,
                        true,
                        choice.active,
                        None::<&str>,
                    )?)?;
                }
                menu.append(&submenu)?;
            }
        }
    }
    Ok(menu)
}

/// The app as the tray menu's [`ActionTarget`].
struct AppTarget<'a>(&'a AppHandle);

impl ActionTarget for AppTarget<'_> {
    fn cancel_dictation(&self) {
        self.0.state::<Dictation>().cancel();
    }

    fn latest_transcript(&self) -> Option<String> {
        match self.0.state::<History>().latest() {
            Ok(entry) => entry.map(|entry| entry.text),
            Err(error) => {
                log::warn!("could not read History: {error}");
                None
            }
        }
    }

    fn copy_to_clipboard(&self, text: &str) -> Result<(), String> {
        #[cfg(windows)]
        return super::clipboard::copy_text(text);
        #[cfg(not(windows))]
        {
            let _ = text;
            Err("no clipboard on this platform".into())
        }
    }

    fn activate_model(&self, model: ModelId) {
        let manager = self.0.state::<ModelManager>().inner().clone();
        // Loading the Model takes a while; the menu thread must not wait for it.
        std::thread::spawn(move || {
            if let Err(error) = manager.activate(model) {
                log::warn!("could not switch to {model:?} from the tray: {error}");
            }
        });
    }

    fn show_main_window(&self) {
        show_main(self.0);
    }

    fn exit(&self) {
        log::info!("Quit Echo chosen in the tray");
        self.0.exit(0);
    }
}

/// Main-window events the tray cares about: closing hides it to the tray (rules 12–13), and
/// bringing it to the front clears the tray error (rule 4a).
pub fn on_main_window_event(window: &Window, event: &WindowEvent) {
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let app = window.app_handle();
            let hint_shown = app.state::<SettingsStore>().get().tray_hint_shown;
            if hint_shown || TrayHintRequested.emit_to(app, MAIN_WINDOW).is_err() {
                let _ = window.hide();
            }
        }
        WindowEvent::Focused(true) => {
            if let Some(dictation) = window.app_handle().try_state::<Dictation>() {
                dictation.window_seen();
            }
        }
        _ => {}
    }
}

/// The user read the close-to-tray hint: it is not shown again, and the window hides.
#[tauri::command]
#[specta::specta]
pub fn close_to_tray(app: AppHandle) {
    if let Err(error) = app
        .state::<SettingsStore>()
        .update(|settings| settings.tray_hint_shown = true)
    {
        log::warn!("could not remember that the tray hint was shown: {error}");
    }
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.hide();
    }
}
