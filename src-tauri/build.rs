fn main() {
    embed_app_manifest();
    link_vulkan_loader_lazily();
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

/// With the `vulkan` feature the Engine's native library imports the Vulkan loader
/// (`vulkan-1.dll`, installed with GPU drivers). transcribe-cpp-sys names its import library but
/// not where it lives, so point the linker at the Vulkan SDK. The DLL is delay-loaded: on a
/// machine without a GPU driver Echo must still start and run on the CPU
/// (`docs/specs/models.md` rule 29), and the Engine keeps the Vulkan backend from running at all
/// there (see `engine::acceleration`).
fn link_vulkan_loader_lazily() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if std::env::var_os("CARGO_FEATURE_VULKAN").is_none()
        || target_os != "windows"
        || target_env != "msvc"
    {
        return;
    }
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");
    let sdk = std::env::var_os("VULKAN_SDK").expect(
        "the `vulkan` feature needs the Vulkan SDK (VULKAN_SDK is not set); install it from \
         https://vulkan.lunarg.com or build with --no-default-features for a CPU-only Engine",
    );
    let lib = std::path::Path::new(&sdk).join("Lib");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-arg=/DELAYLOAD:vulkan-1.dll");
    println!("cargo:rustc-link-arg=delayimp.lib");
    // Executables that never reach the Engine (e.g. the binary's own unit tests) have no Vulkan
    // imports left after dead-code removal; that is expected, not worth a linker warning.
    println!("cargo:rustc-link-arg=/IGNORE:4199");
}
