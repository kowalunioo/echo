# Third-party notices

Echo is MIT-licensed (see [`LICENSE`](LICENSE)). It is built on the open-source components below, which keep their own licenses.

## How this file is maintained

- **Now (skeleton):** the tables list Echo's *direct* dependencies — the ones named in `package.json` (`dependencies`, i.e. code shipped in the app) and `src-tauri/Cargo.toml` (`[dependencies]`). Whoever adds, removes or replaces a direct dependency updates the matching table in the same pull request. Build- and test-only tools (Vite, ESLint, Vitest, Tauri CLI, …) are not shipped and are not listed.
- **Before the first release:** the release slice replaces the tables with a generated, complete list covering every transitive dependency and the full license texts — `cargo about generate` for the Rust crates and a license report from the installed `node_modules` for the frontend bundle — and adds a CI step that fails when the generated file is stale. The generated file is what ships with the installer.
- Models downloaded at runtime (`docs/specs/models.md`) are not bundled; their licenses are shown where they are downloaded and listed in a section here once the Models slice lands.
- Native code compiled into the app by a crate's build script (transcribe-cpp's C/C++ library and what it vendors) is listed in its own table below, because `cargo about` sees only the crate, not the vendored sources.

## Frontend (bundled into the app)

| Package | Version | License |
|---|---|---|
| `@tauri-apps/api` | 2.12.1 | Apache-2.0 OR MIT |
| `i18next` | 26.4.2 | MIT |
| `react` | 19.3.0 | MIT |
| `react-dom` | 19.3.0 | MIT |
| `react-i18next` | 17.0.15 | MIT |
| `zustand` | 5.0.15 | MIT |

## Backend (Rust crates linked into the app)

| Crate | Version | License |
|---|---|---|
| `hound` | 3.5.1 | Apache-2.0 |
| `rubato` | 5.0.1 | MIT OR Apache-2.0 |
| `serde` | 1.0 | MIT OR Apache-2.0 |
| `specta` | 2.0.0-rc.25 | MIT |
| `specta-typescript` | 0.0.12 | MIT |
| `sys-locale` | 0.3.2 | MIT OR Apache-2.0 |
| `tauri` | 2.12.1 | Apache-2.0 OR MIT |
| `tauri-specta` | 2.0.0-rc.25 | MIT |
| `thiserror` | 2.0 | MIT OR Apache-2.0 |
| `transcribe-cpp` (and `transcribe-cpp-sys`) | 0.3.1 | MIT — Copyright (c) 2026 The transcribe.cpp authors |

Their transitive dependencies (about 280 crates on Windows) are under MIT, Apache-2.0, BSD, Zlib, Unlicense, 0BSD, CC0 and Unicode-3.0 licenses, plus MPL-2.0 for `cssparser`, `cssparser-macros`, `selectors`, `dtoa-short` (used by Tauri) and `option-ext`; the generated list will enumerate them with their license texts.

## Native code statically linked into the app

Built from source by `transcribe-cpp-sys` (the Engine, [ADR 0003](docs/adr/0003-transcribe-cpp-behind-engine-trait.md)) and linked into `echo.exe`.

| Component | Where it comes from | License |
|---|---|---|
| transcribe.cpp | `transcribe-cpp-sys` 0.3.1, <https://github.com/handy-computer/transcribe.cpp> | MIT — Copyright (c) 2026 The transcribe.cpp authors |
| ggml (CPU and Vulkan backends) | vendored in `transcribe-cpp-sys` | MIT — Copyright (c) 2023-2026 The ggml authors |
| miniz | vendored in `transcribe-cpp-sys` (`src/third_party/miniz`) | MIT — Copyright 2013-2014 RAD Game Tools and Valve Software; Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC |
| Vulkan headers (Vulkan-Headers, Vulkan-Hpp) | Vulkan SDK, compiled into the Vulkan backend at build time | Apache-2.0 OR MIT — Copyright 2015-2026 The Khronos Group Inc. |

The Vulkan loader (`vulkan-1.dll`) is part of the GPU driver installed on the user's machine and is not shipped with Echo.
