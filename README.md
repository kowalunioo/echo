<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/hero-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="docs/assets/hero-light.svg">
    <img src="docs/assets/hero-light.svg" width="880" alt="Echo: private, local dictation for Windows. The Echo logo, three rounded bars with a shorter lavender one in the middle, above the recording Overlay, a small pill whose level meter ripples out from the centre while you speak.">
  </picture>
</p>

# Echo

<p align="center">
  <a href="https://github.com/kowalunioo/echo/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/kowalunioo/echo/ci.yml?branch=main&style=flat-square&label=ci" alt="CI status"></a>
  <a href="https://github.com/kowalunioo/echo/releases/latest"><img src="https://img.shields.io/github/v/release/kowalunioo/echo?style=flat-square&label=release&color=7c6be0" alt="Latest release"></a>
  <a href="https://github.com/kowalunioo/echo/releases"><img src="https://img.shields.io/github/downloads/kowalunioo/echo/total?style=flat-square&label=downloads" alt="Total downloads"></a>
  <img src="https://img.shields.io/badge/platform-Windows-0078d4?style=flat-square" alt="Platform: Windows">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License: MIT"></a>
</p>

Echo is a private, local dictation app for Windows: press a shortcut, speak, and the Transcript is inserted into whatever app has focus. Speech recognition runs entirely on your machine. Audio and text never leave it; the network is used only to download Models and updates.

<p align="center">
  <a href="https://github.com/kowalunioo/echo/releases/latest">Download</a> ·
  <a href="#privacy">Privacy</a> ·
  <a href="#build-from-source">Build from source</a> ·
  <a href="https://github.com/kowalunioo/echo/releases">Changelog</a> ·
  <a href="https://github.com/kowalunioo/echo/issues/new/choose">Report a bug</a>
</p>

<p align="center">
  <a href="docs/assets/echo-reel.mp4">
    <img src="docs/assets/echo-reel.webp" width="880" alt="Echo in 22 seconds: the logo's three bars move as Echo listens; the Overlay pill ripples while you speak and, when you stop, puts the Transcript into a Notes window; Dictation in 99 languages; runs on your PC, no account, nothing uploaded.">
  </a>
</p>

- **Record Shortcut** in Toggle Mode (press to start, press to stop) or Push-to-Talk Mode (hold to record), plus a **Cancel Shortcut** that inserts nothing.
- **99 Dictation languages**: pick a Dictation Language or let the Model detect it. The UI is in Polish and English.
- **Vocabulary** for the names and jargon the Model should favour (GitHub, Tauri, Vulkan…).
- **Models**: Whisper large-v3-turbo (default), Parakeet TDT 0.6B v3 and Whisper small, with resumable, SHA-256 verified downloads. GPU acceleration on any vendor through Vulkan, with a CPU fallback.
- **Overlay** that shows when Echo is getting ready, listening and transcribing, a short **History** of recent Transcripts, a tray icon, opt-in autostart and a signed in-app updater.

## Privacy

**Nothing you say leaves your PC.** Echo never sends audio, Transcripts, Vocabulary or settings over the network. There is no account, no telemetry and no cloud service. Echo makes exactly two kinds of request:

- **Model downloads**, from the Model publisher's Hugging Face repository, when you choose a Model. Each file is checked against its published SHA-256.
- **Update checks**, against this repository's [latest release](https://github.com/kowalunioo/echo/releases/latest). Automatic checks run 30 s after start and then every 4 hours, and can be turned off; an update installs only if its signature matches the key built into Echo.

## Install

Download the latest installer from [Releases](https://github.com/kowalunioo/echo/releases). Echo runs on Windows only for now.

## Build from source

You need [Bun](https://bun.sh), the Rust toolchain and, for the default Vulkan build, the Vulkan SDK.

```powershell
bun install
bun run tauri dev
```

Run every check before opening a pull request. The Vulkan build breaks on long paths and parallel builds can exhaust the page file, so set a short target directory and limit the jobs:

```powershell
$env:CARGO_TARGET_DIR = "D:\ECHO\.toolchain\target"
$env:CARGO_BUILD_JOBS = "4"
bun run check
```

The hero image above is generated from the real level meter; after changing it, run `bun run readme-hero`.

## Contributing

Read [AGENTS.md](AGENTS.md) first. Scope and architecture are in [docs/plan.md](docs/plan.md), domain terms in [CONTEXT.md](CONTEXT.md), decisions in [docs/adr/](docs/adr/) and behaviour specs in [docs/specs/](docs/specs/).

## License

[MIT](LICENSE). Third-party licences are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
