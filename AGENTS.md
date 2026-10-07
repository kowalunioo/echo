# Echo

Private, local dictation app for Windows (Tauri 2 + Rust + React/TS). Read [`docs/plan.md`](docs/plan.md) for scope, stack, architecture seams and the definition of done; read [`CONTEXT.md`](CONTEXT.md) for domain terms and use them exactly in code, UI strings, tests and PRs. Decisions with their reasons live in [`docs/adr/`](docs/adr/); behaviour specs for each feature live in [`docs/specs/`](docs/specs/) — the spec for your feature is the source of truth for behaviour.

## Clean room — hard rule

This is a clean-room rewrite ([ADR 0001](docs/adr/0001-clean-room-rewrite.md)). Work only from this repository, its specs and public documentation of the libraries you use. Never open `D:\ECHO-LEGACY`, the `kowalunioo/echo-legacy` repo, its git history, or the Handy project's source (`cjpais/Handy`, `handy-computer/*` apart from using `transcribe-cpp` as a dependency). Downloading the Model files named in `docs/specs/models.md` from their Hugging Face repositories is allowed (weights, not code). Banned dependencies: `handy-keys` and any `cjpais` fork. If a spec is unclear, ask the orchestrator instead of looking elsewhere.

## Working on an issue

1. One issue per branch and worktree; branch name `<type>/<issue-number>-<slug>` (e.g. `feat/12-history`).
2. Work test-first: a failing test for each behaviour in the spec, then the code.
3. A slice is vertical — backend, UI and tests together. UI strings go through i18n with both Polish and English text.
4. A feature's settings go into the shared settings model; [`docs/settings.md`](docs/settings.md) shows how to add one.
5. Hardware-dependent code (microphone, keyboard hook, insertion, autostart) stays behind its trait; tests use the fakes, including the WAV **Audio Source**.
6. Before opening the PR, run every check from the definition of done in `docs/plan.md` and make them green. Include in the PR: `Closes #N`, what you verified and how, and screenshots for any UI or Overlay change.

## Build settings

Set `CARGO_TARGET_DIR=D:\ECHO\.toolchain\target` and `CARGO_BUILD_JOBS=4` for every cargo/bun build (`bun run check`, releases): the Vulkan build breaks on long worktree paths and parallel builds exhaust the page file. Parallel implementers may use their own short per-issue dir (`D:\ECHO\.toolchain\t<issue>`) to avoid waiting on each other's cargo lock. Details in [`docs/plan.md`](docs/plan.md#how-work-is-done).

## Conventions

- Commits: conventional prefixes (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `ci:`); the message says why.
- Docs, code, comments and commit messages are in English.
- `tests/fixtures/audio/` holds the user's voice recordings and is gitignored; tests that need them skip cleanly when a file is absent, so CI stays green without them. Never commit audio of a person's voice.
- Never commit secrets (signing keys, tokens) or `.claude/settings.local.json`.
- Shell on this machine is PowerShell (Git Bash also available); give the user PowerShell commands.
