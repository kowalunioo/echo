# Third-party notices

Echo is MIT-licensed (see [`LICENSE`](LICENSE)). It is built on the open-source components below, which keep their own licenses.

## How this file is maintained

- **Now (skeleton):** the tables list Echo's *direct* dependencies — the ones named in `package.json` (`dependencies`, i.e. code shipped in the app) and `src-tauri/Cargo.toml` (`[dependencies]`). Whoever adds, removes or replaces a direct dependency updates the matching table in the same pull request. Build- and test-only tools (Vite, ESLint, Vitest, Tauri CLI, …) are not shipped and are not listed.
- **Before the first release:** the release slice replaces the tables with a generated, complete list covering every transitive dependency and the full license texts — `cargo about generate` for the Rust crates and a license report from the installed `node_modules` for the frontend bundle — and adds a CI step that fails when the generated file is stale. The generated file is what ships with the installer.
- Models downloaded at runtime (`docs/specs/models.md`) are not bundled; their licenses are shown where they are downloaded and listed in a section here once the Models slice lands.
- transcribe-cpp (ADR 0003) is credited here when the Engine slice adds it.

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

Their transitive dependencies (about 280 crates on Windows) are under MIT, Apache-2.0, BSD, Zlib, Unlicense, 0BSD, CC0 and Unicode-3.0 licenses, plus MPL-2.0 for `cssparser`, `cssparser-macros`, `selectors`, `dtoa-short` (used by Tauri) and `option-ext`; the generated list will enumerate them with their license texts.
