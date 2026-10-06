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
pub mod commands;
pub mod engine;
pub mod insertion;
pub mod shortcut;

use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands, collect_events};

/// Where the generated TypeScript bindings live, relative to `src-tauri/`.
pub const BINDINGS_PATH: &str = "../src/bindings.ts";

/// The typed command and event surface shared with the frontend.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::app_info,
            shortcut::app::record_shortcut,
            shortcut::app::set_record_shortcut,
            shortcut::app::reset_record_shortcut,
            shortcut::app::set_shortcut_mode,
            shortcut::app::begin_shortcut_capture,
            shortcut::app::end_shortcut_capture,
            shortcut::app::own_window_key,
        ])
        .events(collect_events![
            shortcut::app::CapturedKeyEvent,
            shortcut::app::RecordIntentEvent,
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
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            shortcut::app::install(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Echo");
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
