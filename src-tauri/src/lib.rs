//! Echo: private, local dictation for Windows.
//!
//! The dictation pipeline depends only on four seams, each in its own module, so it can be
//! tested without real hardware (`docs/plan.md`, "Architecture seams"):
//!
//! - [`audio::AudioSource`] — supplies speech audio (Microphone, or the WAV Audio Source);
//! - [`engine::Engine`] — turns 16 kHz mono audio into a Transcript;
//! - [`shortcut::ShortcutListener`] — reports Record and Cancel Shortcut presses;
//! - [`insertion::Inserter`] — delivers a Transcript into the focused application.

pub mod audio;
pub mod autostart;
pub mod commands;
pub mod data_dir;
pub mod dictation;
pub mod engine;
pub mod history;
pub mod insertion;
pub mod logging;
pub mod models;
pub mod settings;
pub mod shortcut;
pub mod system;
pub mod window;

use std::sync::Arc;

use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{Builder, Event, collect_commands, collect_events};

use data_dir::DataDir;
use history::{History, HistoryChanged, OpenOutcome};
use insertion::SharedInserter;
use models::commands::{AppModelSettings, ModelProblemOccurred, ModelsChanged};
use models::{ModelManager, ModelStorage};
use settings::{LoadOutcome, Settings, SettingsChanged, SettingsStore};
use window::WindowTracker;

/// Where the generated TypeScript bindings live, relative to `src-tauri/`.
pub const BINDINGS_PATH: &str = "../src/bindings.ts";

/// The typed command and event surface shared with the frontend.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::app_info,
            settings::commands::get_settings,
            settings::commands::update_settings,
            settings::commands::reset_setting,
            history::commands::list_history,
            history::commands::delete_history_entry,
            history::commands::restore_history_entry,
            history::commands::clear_history,
            history::commands::reinsert_history_entry,
            system::open_log_folder,
            system::open_microphone_privacy_settings,
            autostart::commands::autostart_status,
            autostart::commands::open_startup_apps_settings,
            audio::microphone::commands::list_microphones,
            audio::microphone::commands::microphone_access,
            models::commands::get_models,
            models::commands::download_model,
            models::commands::cancel_model_download,
            models::commands::activate_model,
            models::commands::delete_model,
            shortcut::app::set_record_shortcut,
            shortcut::app::begin_shortcut_capture,
            shortcut::app::end_shortcut_capture,
            shortcut::app::own_window_key,
            dictation::app::get_dictation_status,
            dictation::app::dictation_window_seen,
            dictation::app::dismiss_dictation_notices,
            dictation::app::get_test_audio,
        ])
        .events(collect_events![
            SettingsChanged,
            ModelsChanged,
            ModelProblemOccurred,
            HistoryChanged,
            shortcut::app::CapturedKeyEvent,
            dictation::app::DictationStatusChanged,
        ])
}

/// The exporter used for `src/bindings.ts`.
pub fn typescript_exporter() -> Typescript {
    Typescript::default()
        .header("// Regenerate with `bun run bindings`; CI fails when this file is out of date.\n")
}

pub fn run() {
    let builder = specta_builder();
    tauri::Builder::default()
        // Must be the first plugin: a second launch hands over to the running Echo and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show_main(app);
        }))
        .plugin(logging::plugin())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            logging::log_panics();
            log::info!("Echo {} starting", env!("CARGO_PKG_VERSION"));
            builder.mount_events(app);

            let data_dir = DataDir::new(app.path().app_local_data_dir()?);
            open_settings(app.handle(), &data_dir);
            open_history(app.handle(), &data_dir);
            // The dictation pipeline registers the real Inserter.
            app.manage(SharedInserter::new());
            app.manage(audio::microphone::Microphones::system());
            open_models(app.handle(), &data_dir);
            dictation::app::install(app.handle());
            shortcut::app::install(app.handle());
            autostart::commands::install(app.handle());

            let tracker = WindowTracker::new(data_dir.window_state_file());
            let autostart = window::launched_by_autostart(std::env::args());
            if let Some(main) = app.get_webview_window(window::MAIN_WINDOW) {
                tracker.restore(&main);
                let completed = app.state::<SettingsStore>().get().onboarding_completed;
                if window::show_at_launch(autostart, completed) {
                    main.show()?;
                    main.set_focus()?;
                }
            }
            app.manage(tracker);
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == window::MAIN_WINDOW {
                window.state::<WindowTracker>().on_event(window, event);
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Echo")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<WindowTracker>().save();
                app.state::<ModelManager>().shutdown();
                log::info!("Echo exiting");
            }
        });
}

/// Loads the settings, makes them available to commands, and forwards every change to the
/// frontend as a [`SettingsChanged`] event.
fn open_settings(app: &tauri::AppHandle, data_dir: &DataDir) {
    let defaults = Settings::defaults(sys_locale::get_locale().as_deref());
    let (store, outcome) = SettingsStore::open(data_dir.settings_file(), defaults);
    match &outcome {
        LoadOutcome::Fresh => log::info!("no settings yet; first run"),
        LoadOutcome::Clean => log::info!("settings loaded"),
        LoadOutcome::Repaired { reset, added } => {
            log::warn!("settings repaired; reset: {reset:?}, added: {added:?}")
        }
        LoadOutcome::Broken { renamed_to } => {
            log::warn!("settings file was unreadable; moved to {renamed_to:?}, using defaults")
        }
    }
    let handle = app.clone();
    store.subscribe(move |_, _| {
        // Send the latest settings rather than this change's, so that when two changes race the
        // last event a window receives is always the current state.
        let latest = handle.state::<SettingsStore>().get();
        if let Err(error) = SettingsChanged(latest).emit(&handle) {
            log::warn!("could not send settings to the windows: {error}");
        }
    });
    app.manage(store);
}

/// Opens History, keeps its limit in step with the settings, and forwards every change to the
/// frontend as a [`HistoryChanged`] event.
fn open_history(app: &tauri::AppHandle, data_dir: &DataDir) {
    let settings = app.state::<SettingsStore>();
    let (history, outcome) =
        History::open(&data_dir.history_file(), settings.get().history_limit.get());
    match outcome {
        OpenOutcome::Opened => log::info!("History opened"),
        OpenOutcome::Replaced { renamed_to, error } => {
            log::warn!("History database was unusable ({error}); moved to {renamed_to:?}")
        }
        OpenOutcome::InMemory { error } => {
            log::error!("History database cannot be used ({error}); History is not saved this time")
        }
    }
    let handle = app.clone();
    history.subscribe(move |entries| {
        if let Err(error) = HistoryChanged(entries.to_vec()).emit(&handle) {
            log::warn!("could not send History to the windows: {error}");
        }
    });
    let handle = app.clone();
    settings.subscribe(move |old, new| {
        if old.history_limit != new.history_limit {
            let limit = new.history_limit.get();
            if let Err(error) = handle.state::<History>().set_limit(limit) {
                log::error!("could not apply the History limit: {error}");
            }
        }
    });
    app.manage(history);
}

/// Starts the Model manager: forwards its state and problems to the windows, follows the
/// "Unload Model after inactivity" setting, and checks the inactivity timer every second.
fn open_models(app: &tauri::AppHandle, data_dir: &DataDir) {
    let manager = ModelManager::new(
        ModelStorage::new(data_dir.models_dir()),
        Arc::new(AppModelSettings(app.clone())),
        Arc::new(models::TranscribeCppEngines),
        Arc::new(models::SystemDiskSpace),
        Arc::new(models::SystemClock::default()),
        models::download::DownloadConfig::default(),
    );
    let handle = app.clone();
    manager.subscribe(move |state| {
        if let Err(error) = ModelsChanged(state.clone()).emit(&handle) {
            log::warn!("could not send the Models to the windows: {error}");
        }
    });
    let handle = app.clone();
    manager.on_problem(move |problem| {
        if let Err(error) = ModelProblemOccurred(problem.clone()).emit(&handle) {
            log::warn!("could not report a Model problem to the windows: {error}");
        }
    });
    let listener = manager.clone();
    app.state::<SettingsStore>().subscribe(move |old, new| {
        if old.unload_model_after != new.unload_model_after {
            listener.unload_setting_changed();
        }
    });
    let ticker = manager.clone();
    std::thread::Builder::new()
        .name("echo-model-idle".into())
        .spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                ticker.tick();
            }
        })
        .expect("the idle timer thread starts");
    app.manage(manager.clone());
    manager.start();
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Fails when `src/bindings.ts` no longer matches the Rust commands. With
    /// `ECHO_UPDATE_BINDINGS=1` it rewrites the file instead (`bun run bindings`).
    #[test]
    fn typescript_bindings_are_up_to_date() {
        let committed_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BINDINGS_PATH);
        let fresh_path = std::env::temp_dir().join(format!(
            "echo-bindings-{}-{:?}.ts",
            std::process::id(),
            std::thread::current().id()
        ));
        specta_builder()
            .export(typescript_exporter(), &fresh_path)
            .expect("bindings export");
        let fresh = std::fs::read_to_string(&fresh_path)
            .unwrap()
            .replace("\r\n", "\n");
        let _ = std::fs::remove_file(&fresh_path);

        if std::env::var_os("ECHO_UPDATE_BINDINGS").is_some() {
            std::fs::write(&committed_path, &fresh).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(&committed_path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            committed == fresh,
            "src/bindings.ts is out of date with the Rust commands; run `bun run bindings`"
        );
    }
}
