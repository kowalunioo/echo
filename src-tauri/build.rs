fn main() {
    embed_app_manifest();
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("tauri-build failed");
}

/// Tauri imports Common Controls v6 functions, which Windows only resolves for executables that
/// declare that dependency in a manifest. tauri-build would embed one into the app binary only,
/// so test executables would die with STATUS_ENTRYPOINT_NOT_FOUND before running a single test.
/// Instead we embed the manifest into every linked artifact (app, unit and integration tests).
fn embed_app_manifest() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
