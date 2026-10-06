//! The diagnostic log (`settings-and-first-run.md` rule 22).
//!
//! Log files live in `%LOCALAPPDATA%\com.enloque.echo\logs\` (Tauri's app log folder, inside
//! Echo's data directory), at most [`MAX_FILE_BYTES`] each, keeping [`KEEP_FILES`] files.
//!
//! **Privacy rule for every module:** Transcript text, Vocabulary entries and clipboard contents
//! may only be logged at `debug!` or `trace!` level. Release builds record [`release_level`]
//! (`Info`) and above, so such text never reaches a release log; log events, durations and error
//! messages at `info!`/`warn!`/`error!`.

use log::LevelFilter;
use tauri::Runtime;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

/// Size at which a log file is rotated.
pub const MAX_FILE_BYTES: u128 = 5 * 1024 * 1024;
/// Number of log files kept.
pub const KEEP_FILES: usize = 5;

/// The most detailed level written in a release build.
pub const fn release_level() -> LevelFilter {
    LevelFilter::Info
}

/// The level for this build: `Debug` in development, [`release_level`] in release builds.
pub const fn level() -> LevelFilter {
    if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        release_level()
    }
}

/// The logging plugin: rotated files in the log folder, plus the console in development.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let mut builder = tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::LogDir {
            file_name: Some("echo".into()),
        }))
        .level(level())
        // Window-system and webview internals are noisy and say nothing about Echo.
        .level_for("tao", LevelFilter::Warn)
        .level_for("wry", LevelFilter::Warn)
        .level_for("tauri", LevelFilter::Info)
        .max_file_size(MAX_FILE_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(KEEP_FILES))
        .timezone_strategy(TimezoneStrategy::UseLocal);
    if cfg!(debug_assertions) {
        builder = builder.target(Target::new(TargetKind::Stdout));
    }
    builder.build()
}

/// Records panics in the log as well as on stderr, so crash reports have something to show.
pub fn log_panics() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    // Part of settings-and-first-run.md rule 22: Transcript text is only ever logged at debug
    // level, which a release build does not record.
    #[test]
    fn release_builds_do_not_record_debug_messages() {
        assert!(log::Level::Debug > release_level());
        assert!(log::Level::Info <= release_level());
    }
}
