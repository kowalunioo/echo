# Echo — rewrite plan

Echo is a private, local dictation app for Windows: press a shortcut, speak, and the transcript is typed into whatever app has focus. Speech recognition runs entirely on the user's machine.

This repository is a from-scratch rewrite. The goals are:

- **Legal clean room** — no code derived from the previous (Handy-based) implementation, so the LICENSE carries only Echo's own copyright. See [ADR 0001](adr/0001-clean-room-rewrite.md).
- **Redesign, not a port** — better architecture and a fresh UI; behaviour is re-specified, not copied.

## Constraints

- **Windows only** for now. Every platform-specific piece (hotkeys, text insertion, autostart, tray quirks) sits behind a trait so macOS/Linux can be added later (backlog #9).
- **No data compatibility** with the old app: settings, history and model storage use fresh formats and locations.
- App identifier `com.enloque.echo`; versioning starts at `0.1.0`.
- **No CUDA-only engines.** GPU acceleration must work on any vendor (Vulkan / DirectML) with a CPU fallback.
- UI is internationalised from day 1; shipped languages: **Polish and English**.

## 0.1.0 scope

| Feature | Notes |
|---|---|
| Record shortcut | Two modes: toggle (press to start, press to stop) and push-to-talk (hold to record). |
| Cancel shortcut | Abort the current dictation; nothing is inserted. |
| Vocabulary | User-defined words/phrases the engine should favour (names, jargon). |
| Dictation language | Chosen language, or automatic detection where the model supports it. |
| Microphone selection | Pick an input device; default device follows the system. |
| History | Past transcripts stored in SQLite; configurable limit, default 5. |
| Models | Selection and download (resumable, SHA-256 verified). Whisper large-v3-turbo (default), Parakeet TDT 0.6B v3, Whisper small. |
| Tray | Tray icon with menu; app lives in the tray. |
| Recording overlay | Small always-on-top indicator while recording/transcribing. |
| Autostart | Start with Windows (opt-in). |
| Updater | Simple in-app updater, signed with a new key. |

Everything else is backlog: GitHub issues #1–#9, milestone "Później", board <https://github.com/users/kowalunioo/projects/1>.

## Stack

- **Shell:** Tauri 2, Rust backend.
- **Frontend:** React + TypeScript + Tailwind + Vite, Bun as package manager/runner, Zustand for state, tauri-specta (pinned version) for typed commands/events.
- **Audio:** cpal for capture, rubato for resampling to 16 kHz mono, earshot for voice-activity detection.
- **Hotkeys:** own low-level keyboard hook (`WH_KEYBOARD_LL` via the `windows` crate). See [ADR 0002](adr/0002-own-keyboard-hook.md).
- **Insertion:** clipboard paste (marked as excluded from clipboard history, previous clipboard restored) with `SendInput` typing as fallback.
- **Storage:** rusqlite for history; settings in a fresh format of our choosing.
- **Downloads:** reqwest with resume and SHA-256 verification.
- **Engine:** transcribe-cpp behind an `Engine` trait. See [ADR 0003](adr/0003-transcribe-cpp-behind-engine-trait.md).
- **Banned dependencies:** `handy-keys`, and any `cjpais` fork (vad-rs, rodio, hf-hub forks).

Visual style: calm, warm, minimal — in the spirit of the Claude desktop app (soft neutral palette, generous spacing, rounded surfaces, restrained accent colour). Described in words, never copied.

## Architecture seams

The backend is organised around a few traits so that the dictation pipeline can be tested without real hardware:

- **`AudioSource`** — yields PCM frames. Implementations: microphone (cpal) and **WAV file** (fake microphone for tests and dev mode).
- **`Engine`** — turns 16 kHz mono audio (+ language, vocabulary) into a transcript.
- **`ShortcutListener`** — reports shortcut press/release events.
- **`Inserter`** — puts text into the focused app.

The pipeline (shortcut → record → transcribe → insert → history) depends only on these traits. Exact trait shapes are designed in the skeleton slice.

## How work is done

**Roles.**
- The **orchestrator** (a Claude session) splits work into GitHub issues, dispatches implementer agents, verifies their results itself (build, tests, running the app), requests fixes, merges PRs to `main` when CI is green, and closes issues.
- A **spec-writer** agent is the only one allowed to read the legacy code; it writes behaviour-only specs into `docs/specs/`.
- **Implementer** agents each take one issue, in their own git worktree and branch, and open a PR. They never read legacy code.
- **The user** approves every behaviour spec before implementation, reviews UI via screenshots on PRs, and does a real-microphone smoke test before each release.

**Work unit = vertical slice.** One feature = backend + UI + tests in one PR, not layers.

**Definition of done for a slice:**
1. `cargo test`, `cargo clippy` with no warnings, `cargo fmt --check`, ESLint, `tsc`, frontend tests — green locally and in GitHub CI.
2. End-to-end check in **fake-microphone mode**: a WAV fixture is fed instead of the mic, and the full chain shortcut → record → transcribe → insert into Notepad produces the expected text (within tolerance).
3. Screenshots of any UI/overlay change attached to the PR.
4. Before a release: the user's real-microphone test.

**Build settings.** The native transcribe-cpp/Vulkan build breaks on long Windows paths, and parallel builds have exhausted the page file, so every local cargo/bun build sets:
- `CARGO_TARGET_DIR=D:\ECHO\.toolchain\target` — one short, warm target directory for `bun run check` and releases. An implementer working in parallel with others may use its own short per-issue directory (`D:\ECHO\.toolchain\t<issue>`) instead, so builds don't wait on each other's cargo lock.
- `CARGO_BUILD_JOBS=4`.

sccache is not used: measured on a clean Vulkan `cargo test --no-run` into a fresh target directory, a warm sccache gave 0 hits out of 346 Rust compilations (also with `SCCACHE_BASEDIRS`), most likely because its Rust cache keys still carry paths into the target directory; and it never covers the native transcribe-cpp/Vulkan part, which cmake builds. A warm shared target directory is what saves the time.

**Commits:** conventional prefixes (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `ci:`); the message explains *why*.

## Test audio

Fixtures live in `tests/fixtures/audio/` and are **gitignored** — they contain the user's voice and the repo is public. Implementers and CI must treat them as optional: tests that need them skip cleanly when absent.

All files are WAV PCM 16-bit, 48 kHz stereo (kept as-is to exercise downmix and resampling), 4–10 s, fairly quiet (peaks around −17 to −21 dBFS).

| File | Expected transcript |
|---|---|
| `pl-proste.wav` | Dzień dobry, to jest test dyktowania w aplikacji Echo. |
| `pl-interpunkcja.wav` | Czy jutro o piętnastej możemy się spotkać? Jeśli nie, napisz mi wiadomość. |
| `pl-slownik.wav` | Wrzuciłem poprawkę do Echo na GitHuba, a Claude Code przetestował ją w Tauri z modelem Parakeet na Vulkanie. |
| `pl-liczby.wav` | Zamówienie numer 4521 kosztuje 129 złotych i 99 groszy. |
| `en-proste.wav` | This is a quick dictation test in English. |
| `cisza.wav` | *(empty — 10 s of room tone with a noise peak at −16.7 dB; no hallucinated text allowed)* |

`pl-slownik.wav` is the vocabulary test: Echo, GitHub, Claude Code, Tauri, Parakeet, Vulkan (including Polish inflected forms).

Comparison is loose: case, punctuation and number formatting may differ (e.g. "129,99 zł"). The exact tolerance (normalised word error rate threshold) is defined in the transcription spec.

## Roadmap to 0.1.0

1. **Foundation docs** — this plan, `AGENTS.md`, `CONTEXT.md`, ADRs. *(first commit)*
2. **Specs** — spec-writer produces one behaviour spec per 0.1.0 feature in `docs/specs/`; the user approves each.
3. **Skeleton** — Tauri 2 app (`com.enloque.echo`, `0.1.0`), the trait seams with a fake WAV audio source, i18n (PL + EN), Windows CI workflow, new updater signing key (private key stored as a GitHub secret by the user).
4. **Slices** — 0.1.0 issues (milestone `0.1.0`) on the board, dispatched to implementers in parallel where independent.
5. **Release** — new release workflow; update `.claude/agents/release.md` to match it.
