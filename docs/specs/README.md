# Behaviour specs — Echo 0.1.0

One spec per 0.1.0 feature. Each spec is the source of truth for behaviour; it states what the user observes and what the system guarantees, using the vocabulary in [`CONTEXT.md`](../../CONTEXT.md). Each has Behaviour, Settings, UI, Acceptance tests and Decisions; the Decisions section records the user's answers to the questions raised while writing the spec, with a short reason.

| Spec | Summary |
|---|---|
| [dictation-pipeline.md](dictation-pipeline.md) | Dictation states, audio conversion to 16 kHz mono, voice-activity detection, short/silent Recordings, Engine errors, presses while busy, Insertion (clipboard paste with restore, typing fallback), fake-microphone mode and the normalised-WER acceptance tolerance for the fixtures. |
| [record-shortcut.md](record-shortcut.md) | Push-to-Talk Mode and Toggle Mode, default Ctrl+Space, exact-match and debounce rules, swallowed vs passed-through keys, allowed combinations, capture UI. |
| [cancel-shortcut.md](cancel-shortcut.md) | Escape by default; Cancellation during Recording or Transcribing, active only during a Dictation; equivalent Overlay and tray actions. |
| [vocabulary.md](vocabulary.md) | The Vocabulary list, normalisation and duplicates, hint for Whisper Models with a budget indicator, spelling correction for Parakeet. |
| [dictation-language.md](dictation-language.md) | Automatic or specific Dictation Language, resolution against each Model's capabilities, picker behaviour. |
| [microphone.md](microphone.md) | Default device following Windows, specific device selection, missing-device fallback, device lost mid-Recording, privacy block. |
| [history.md](history.md) | Text-only History in SQLite, stored fields, limit 0–100 (default 5), copy / delete with undo / re-insert / clear all. |
| [models.md](models.md) | The three 0.1.0 Models, storage location, resumable downloads with size and SHA-256 checks, progress, pause/resume, activation, deletion, first-Model handling, optional unload after inactivity. |
| [tray.md](tray.md) | Tray icon states (incl. red error state) and theme, left/right click, menu items in idle and busy states, close-to-tray, single instance, quit. |
| [overlay.md](overlay.md) | Getting ready / listening / transcribing / message states, never takes focus, cancel button, placement on the pointer's monitor, scaling, visibility and position settings. |
| [autostart.md](autostart.md) | Opt-in start at Windows sign-in, self-healing registration, starts hidden in the tray, respects Windows Startup apps. |
| [updater.md](updater.md) | Signed updates from the GitHub release feed, automatic checks every 4 h, install only when Idle, manual check, environment kill switch. |
| [settings-and-first-run.md](settings-and-first-run.md) | Onboarding steps, UI Language (PL/EN), settings persistence and salvage, window/process behaviour, privacy of logs, settings navigation structure. |

## Out of scope for 0.1.0

Behaviour that existed in the previous app but is not part of 0.1.0 (candidates for the backlog):

- Automatic hybrid shortcut mode (short tap toggles, long hold is push-to-talk).
- Second Record Shortcut that sends the Transcript through an external AI text clean-up service (with provider, API key and prompt settings).
- Live text preview while speaking for streaming-capable Models, and the larger overlay style for it.
- Filler-word removal ("uh", "um", …) with language-dependent and custom lists.
- Lowercase-everything output option.
- Chinese simplified/traditional script conversion.
- Translate-to-English option.
- Global shortcut to add the selected text to the Vocabulary.
- Alternative insertion methods (Ctrl+Shift+V, Shift+Insert, typing only, none, external script), copy-to-clipboard-after-insertion, auto-press Enter after insertion, adjustable paste delays.
- Start/stop sound cues with themes, volume and output-device choice; mute system audio while recording.
- Input channel selection; laptop-lid-closed microphone; keep-microphone-always-open and delayed-close options; extra recording time after release.
- Voice-activity detection on/off switch and detector choice.
- Tray "Unload Model" item; manual CPU/GPU device selection; large catalogue of extra Models, alternative quantisations and user-supplied model files.
- History starring, time-based retention, retranscription from stored audio.
- Theme override (light/dark), hide-tray-icon option, start-hidden option.
- "What's new" release-notes dialog after updates.
- Debug section (log viewer, keyboard diagnostics, timing knobs), command-line remote control (toggle/cancel from another process), headless file transcription.
- macOS and Linux specifics (permissions, secure-input warnings, typing tools).
- UI languages other than Polish and English.
