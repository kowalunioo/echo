//! The updater inside the Tauri app: the driver thread, the commands and the
//! event the main window follows, and the real [`Host`].
//!
//! One thread ("echo-updater") owns the [`UpdaterCore`]. It ticks once a second
//! and runs manual checks and confirmed installs sent by the commands, so no
//! command waits on the network.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use semver::Version;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use super::feed::{PluginFeed, PluginInstaller};
use super::marker::{self, RestartMarker};
use super::{Host, UpdateStatus, UpdaterCore};
use crate::dictation::{Dictation, DictationState, DictationStatus};
use crate::models::{DownloadState, ModelManager, ModelsState};
use crate::settings::SettingsStore;
use crate::window::{MAIN_WINDOW, WindowTracker};

/// What the main window shows about updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterView {
    /// The running version.
    pub current_version: String,
    /// The updater is disabled by the environment: "Updates are managed by your system" (rule 11).
    pub managed: bool,
    pub status: UpdateStatus,
    /// "Echo was updated to <version>" after an update's restart (rule 6), until dismissed.
    pub updated_to: Option<String>,
}

/// The updater's view changed.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct UpdaterChanged(pub UpdaterView);

enum Command {
    Check,
    Install,
}

/// Managed state: the current view and the way to the driver thread (none when disabled).
pub struct Updater {
    view: Mutex<UpdaterView>,
    commands: Option<Mutex<Sender<Command>>>,
}

impl Updater {
    fn view(&self) -> UpdaterView {
        self.view
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn change(&self, app: &AppHandle, change: impl FnOnce(&mut UpdaterView)) {
        let view = {
            let mut view = self.view.lock().unwrap_or_else(PoisonError::into_inner);
            change(&mut view);
            view.clone()
        };
        if let Err(error) = UpdaterChanged(view).emit(app) {
            log::warn!("Couldn't send the updater status: {error}");
        }
    }

    fn send(&self, command: Command) {
        if let Some(commands) = &self.commands {
            let sent = commands
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .send(command);
            if sent.is_err() {
                log::warn!("The updater thread has stopped");
            }
        }
    }

    /// Whether the updater runs at all (rule 11).
    pub fn enabled(&self) -> bool {
        self.commands.is_some()
    }
}

/// Starts the updater. `disabled` comes from `ECHO_DISABLE_UPDATES` (rule 11);
/// `updated_to` from the restart marker (rule 6).
pub fn install(app: &AppHandle, marker_file: PathBuf, disabled: bool, updated_to: Option<String>) {
    let current = env!("CARGO_PKG_VERSION");
    let commands = if disabled {
        log::info!("Updates are disabled by {}", super::DISABLE_ENV);
        None
    } else {
        let (sender, receiver) = mpsc::channel();
        let handle = app.clone();
        let started = Instant::now();
        let spawned = thread::Builder::new()
            .name("echo-updater".into())
            .spawn(move || {
                let installing = Arc::new(Mutex::new(None));
                let host = AppHost {
                    app: handle.clone(),
                    marker_file: marker_file.clone(),
                    installing: Arc::clone(&installing),
                };
                let exiting = handle.clone();
                let feed = PluginFeed::new(handle, move || {
                    before_exit(&exiting, &marker_file, &installing);
                });
                let version = Version::parse(current).expect("Cargo version is semver");
                let mut core = UpdaterCore::new(feed, PluginInstaller, version, Duration::ZERO);
                loop {
                    match receiver.recv_timeout(Duration::from_secs(1)) {
                        Ok(Command::Check) => core.check_manually(&host),
                        Ok(Command::Install) => core.install_confirmed(&host),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    core.tick(started.elapsed(), &host);
                }
            });
        match spawned {
            Ok(_) => Some(Mutex::new(sender)),
            Err(error) => {
                log::error!("Couldn't start the updater: {error}");
                None
            }
        }
    };
    app.manage(Updater {
        view: Mutex::new(UpdaterView {
            current_version: current.to_owned(),
            managed: disabled,
            status: UpdateStatus::Idle,
            updated_to,
        }),
        commands,
    });
}

/// Runs a manual check (tray "Check for updates…" and the settings button).
pub fn check_now(app: &AppHandle) {
    if let Some(updater) = app.try_state::<Updater>() {
        updater.send(Command::Check);
    }
}

/// Whether an update is installing: Echo is about to restart, so no Dictation may start.
pub fn installing(app: &AppHandle) -> bool {
    app.try_state::<Updater>()
        .is_some_and(|updater| matches!(updater.view().status, UpdateStatus::Installing { .. }))
}

/// Whether the tray shows "Check for updates…" (rule 11).
pub fn enabled(app: &AppHandle) -> bool {
    app.try_state::<Updater>()
        .is_some_and(|updater| updater.enabled())
}

/// Idle for the updater (rule 5): no Recording, Transcribing or Inserting and no
/// Model download running.
pub fn is_idle(dictation: &DictationStatus, models: &ModelsState) -> bool {
    let downloading = models.models.iter().any(|model| {
        matches!(
            model.download,
            DownloadState::Queued | DownloadState::Downloading { .. } | DownloadState::Verifying
        )
    });
    dictation.state == DictationState::Idle && !downloading
}

struct AppHost {
    app: AppHandle,
    marker_file: PathBuf,
    /// The version being installed, for [`before_exit`].
    installing: Arc<Mutex<Option<Version>>>,
}

/// Runs right before the installer ends Echo (never in development builds, which do not
/// install): writes the restart marker (rule 6) and saves what the usual exit would.
fn before_exit(app: &AppHandle, marker_file: &Path, installing: &Mutex<Option<Version>>) {
    let Some(version) = installing
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    else {
        return;
    };
    let window_visible = app.get_webview_window(MAIN_WINDOW).is_some_and(|window| {
        marker::window_shown(
            window.is_visible().unwrap_or(false),
            window.is_minimized().unwrap_or(false),
        )
    });
    let marker = RestartMarker {
        version: version.to_string(),
        window_visible,
    };
    if let Err(error) = marker::write_for_restart(marker_file, &marker, cfg!(debug_assertions)) {
        log::warn!("Couldn't write the update restart marker: {error}");
    }
    if let Some(tracker) = app.try_state::<WindowTracker>() {
        tracker.save();
    }
}

impl Host for AppHost {
    fn is_idle(&self) -> bool {
        let dictation = self.app.state::<Dictation>().status();
        let models = self.app.state::<ModelManager>().state();
        is_idle(&dictation, &models)
    }

    fn automatic(&self) -> bool {
        self.app
            .state::<SettingsStore>()
            .get()
            .check_updates_automatically
    }

    fn before_install(&self, version: &Version) {
        // The marker itself is written by `before_exit`, once the installer really starts.
        *self
            .installing
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(version.clone());
    }

    fn install_failed(&self) {
        self.installing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        marker::remove(&self.marker_file);
    }

    fn publish(&self, status: &UpdateStatus) {
        let updater = self.app.state::<Updater>();
        updater.change(&self.app, |view| view.status = status.clone());
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_updater_view(updater: State<'_, Updater>) -> UpdaterView {
    updater.view()
}

#[tauri::command]
#[specta::specta]
pub fn check_for_updates(updater: State<'_, Updater>) {
    updater.send(Command::Check);
}

/// The user confirmed "Install and restart".
#[tauri::command]
#[specta::specta]
pub fn install_update(updater: State<'_, Updater>) {
    updater.send(Command::Install);
}

#[tauri::command]
#[specta::specta]
pub fn dismiss_update_notice(app: AppHandle, updater: State<'_, Updater>) {
    updater.change(&app, |view| view.updated_to = None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ActiveModelState, ModelEntry, ModelId};

    fn models_with(download: DownloadState) -> ModelsState {
        ModelsState {
            models: vec![ModelEntry {
                id: ModelId::WhisperSmall,
                name: "Whisper small".into(),
                size_bytes: 1,
                languages: 1,
                recommended: false,
                downloaded: false,
                download,
            }],
            active: None,
            active_state: ActiveModelState::None,
            activating: None,
            load_failure: None,
            dictation_in_progress: false,
        }
    }

    #[test]
    fn rule_5_idle_means_no_dictation_and_no_running_model_download() {
        let idle = DictationStatus::default();
        assert!(is_idle(&idle, &models_with(DownloadState::Idle)));

        for state in [
            DictationState::Recording,
            DictationState::Transcribing,
            DictationState::Inserting,
        ] {
            let busy = DictationStatus {
                state,
                ..DictationStatus::default()
            };
            assert!(
                !is_idle(&busy, &models_with(DownloadState::Idle)),
                "{state:?}"
            );
        }

        let running = [
            DownloadState::Queued,
            DownloadState::Verifying,
            DownloadState::Downloading {
                downloaded: 1,
                total: 2,
                bytes_per_second: 1,
            },
        ];
        for download in running {
            assert!(
                !is_idle(&idle, &models_with(download.clone())),
                "{download:?}"
            );
        }
        let paused = DownloadState::Paused {
            downloaded: 1,
            total: 2,
        };
        assert!(is_idle(&idle, &models_with(paused)));
    }
}
