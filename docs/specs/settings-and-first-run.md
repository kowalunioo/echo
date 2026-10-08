# Settings and first run

The structure of Echo's main window (settings), the first-run onboarding, the UI Language, and cross-cutting behaviour not owned by another spec: settings persistence, privacy of logs, single instance and window behaviour.

## Behaviour

### First run (onboarding)

1. First run is when no Model has ever been made active (fresh settings). Onboarding is shown in the main window, which opens regardless of how Echo was started.
2. Onboarding steps:
   1. **Welcome** — one screen: what Echo does, that everything runs locally, that a speech Model will be downloaded once and is chosen in the next step (no sizes: each Model shows its own on Choose a Model, and the catalogue will grow), and how a Dictation is started in the current Record Shortcut mode (Push-to-Talk Mode: "hold a shortcut, speak, and let go"; Toggle Mode: "press a shortcut, speak, and press it again"). UI Language switch (Polski / English) available here.
   2. **Microphone access** — shown only if Windows privacy settings block microphone access for desktop apps. Explains the problem, offers "Open Windows privacy settings", and re-checks automatically every 2 s and when the window regains focus; continues automatically once access is allowed. A "Skip" link continues anyway (Recordings will fail until fixed).
   3. **Choose a Model** — the three Models from `models.md` with sizes and one-line descriptions. Echo says in plain words what it found on this computer and marks the Model recommended for it as Recommended (`models.md` rule 30): with a graphics card the Engine can use, "Graphics card found — recommended: Whisper large-v3-turbo"; without one, "No graphics card found — Parakeet TDT 0.6B v3 is faster on this PC". Every Model has its own Download (labelled with its size), so the user can fetch several to try them; one downloads at a time and the rest queue (`models.md` rule 7). A download shows progress, speed and Cancel in its Model's row. Models already present (downloaded earlier) appear as "Use this Model" without downloading. "Finish later" leaves onboarding without a Model (rule 3a).
   4. **Try it** — appears when the Model is downloaded and active: shows the Record Shortcut and mode ("Hold Ctrl+Space and say:"), then the step's hero: a large borderless test field whose placeholder is the sample phrase to say ("Hello Echo, can you hear me?", in the UI Language), and "Finish". A fixed-height status line under the field follows the Dictation in the Overlay's words: "Getting ready…" while recording before audio flows, "Listening" with a pulsing dot once it flows, "Transcribing…" while the phrase shimmers softly; the phrase itself stays muted throughout. Until onboarding is complete, a Dictation's Transcript is put straight into this field instead of being inserted into the application with keyboard focus (`dictation-pipeline.md` rule 31a), so the test works wherever focus is; it is still added to History. Any text arriving in the field (no matching against the phrase) replaces the phrase and the status line acknowledges it ("Echo heard you."). 2.5 s after text first arrives, with "Opening Echo…" shown next to "Finish" meanwhile, the step fades out and onboarding completes as with "Finish"; starting a new Dictation, or clicking, focusing or typing in the field before then, cancels that, and the user leaves with "Finish". With Windows animations off nothing shimmers, blurs or pulses; states only fade. A "Nothing appeared?" disclosure lists the likely causes, each with its action: the Microphone ("Microphone settings" opens the Microphone section of the Dictation page; "Windows privacy settings"), the Model ("Model settings" opens the Model & language page), and anything else ("Open log folder"). Following an action that opens a page completes onboarding and shows that page.
2a. Every step after the first offers "Back" to the previous shown step. Going back undoes nothing: Welcome stays done, a running download continues and an active Model stays active. On a step reached with Back, the forward action moves to the next step ("Continue" on Choose a Model once a Model is active).
2b. When the step changes, keyboard focus moves to the new step's heading, so screen readers announce it. Try it instead focuses its test field (also when it is the first step shown), so a Dictation has somewhere to land; the field's description names the step and the instruction. The progress indicator marks completed steps with a check mark and the current step with a filled marker, not by colour alone.
3. Onboarding can be left at the "Try it" step via "Finish"; the Record Shortcut becomes active as soon as a Model is active (step 3 complete), even before "Finish".
3a. Onboarding can also be left at the "Choose a Model" step via "Finish later": onboarding is complete and the main window opens on the Model & language page with the "Download a Model to start" panel (rule 5). A download started before "Finish later" continues. Until a Model is active, Dictation stays unavailable as in `dictation-pipeline.md` rule 5.
4. If the user closes the window during onboarding, Echo stays in the tray; the next time the window opens, onboarding resumes at the first incomplete step. A running download continues in the background.
5. After onboarding is complete it is never shown again, unless all Models are deleted — then the main window shows a prominent "Download a Model to start" panel (not the full onboarding).

### UI Language

6. The UI Language is Polish or English.
7. On first run it is chosen from the Windows display language: Polish if the Windows display language is Polish, otherwise English.
8. Changing the UI Language applies immediately to the main window, the Overlay and the tray menu, without restart.
9. The UI Language is independent of the Dictation Language.
10. All user-visible text exists in both languages; dates and numbers are formatted per the UI Language.

### Settings persistence

11. Every setting is saved as soon as it is changed; there is no "Save" button.
12. Settings are stored in a fresh format in Echo's data directory under `com.enloque.echo`; nothing is imported from any previous application.
13. If the settings file is unreadable or partly invalid, every individually valid setting is kept and only the invalid ones fall back to their defaults; the file is then rewritten. A completely unreadable file is renamed with a `.broken` suffix and defaults are used.
14. Settings added in a newer version get their default values when an older settings file is loaded.
15. Each setting that has a default offers a "reset to default" control where the setting UI shows one (shortcuts, Microphone, Dictation Language).

### Window and process

16. Only one Echo process runs per Windows user. Starting it again brings the existing main window to the front (see `tray.md` rule 14).
17. Closing the main window hides it to the tray (see `tray.md`).
18. A manual launch opens the main window; an autostart launch starts hidden (see `autostart.md`).
19. The main window remembers its size and position between launches; if the remembered position is off-screen (monitor removed), it opens centred on the primary monitor.
20. The window follows the Windows light/dark app mode.

### Privacy

21. Echo never sends audio, Transcripts, Vocabulary or settings over the network. The only network traffic is Model downloads (`models.md`) and update checks/downloads (`updater.md`).
22. Echo writes a diagnostic log file in its data directory. In release builds the log never contains Transcript text, Vocabulary entries or clipboard contents; it records only events, durations and error messages. Logs are capped (e.g. 5 files × 5 MB) and rotated.

## Settings

Owned here:

| Setting | Values | Default |
|---|---|---|
| UI Language | Polski, English | from Windows display language (Polish → Polski, else English) |

All other settings are owned by their feature specs; the settings UI groups them as below.

## UI

The main window has a left navigation with these sections (names in English / Polish):

1. **Dictation / Dyktowanie** — Record Shortcut and mode, Cancel Shortcut, Microphone. (`record-shortcut.md`, `cancel-shortcut.md`, `microphone.md`)
2. **Model & language / Model i język** — the Models list with download/activate/delete, the active Model indicator, and the Dictation Language picker for the active Model. (`models.md`, `dictation-language.md`)
3. **Vocabulary / Słownik** — the Vocabulary list and budget. (`vocabulary.md`)
4. **History / Historia** — the History list, limit and clear-all. (`history.md`)
5. **App / Aplikacja** — UI Language, start when signing in, show recording indicator and its position, automatic updates, check for updates, version, an "Open log folder" link, and a short note that the voice and Transcripts stay on this computer (Echo goes online only to download Models and check for updates). (`autostart.md`, `overlay.md`, `updater.md`)

Visual direction (from `docs/plan.md`): calm, warm, minimal — soft neutral palette, generous spacing, rounded surfaces, one restrained accent colour. Every control has a short plain-language description. Keyboard navigation and visible focus rings throughout; all controls have accessible names.

A status area at the top or bottom of the window shows: active Model state (ready / loading / unloaded / downloading x% / none), an update notice when one is available, and error notices for errors that happened since the window was last opened (`dictation-pipeline.md` rules 39b–39d). Opening the window clears the tray's red error state; each notice can be dismissed. Notices follow `dictation-pipeline.md` rule 39e: a plain message, the technical detail below it in smaller selectable text, and the action that leads to the fix.

## Acceptance tests

1. *Fresh start shows onboarding.* With empty settings, the main window opens on Welcome. (e2e.)
2. *Microphone step skipped when allowed.* With the permission fake reporting "allowed", step 2 is skipped. With "denied", it is shown and advances automatically when the fake switches to "allowed". (Frontend with mocked commands.)
3. *Model step.* Choosing Download on the recommended Model starts its download; when the fake download completes, the Model is active, the Record Shortcut is active, and "Try it" is shown. (Frontend + fakes.)
4. *Resume after close.* Closing during the Model step and reopening returns to the Model step with the download progress still shown. (e2e with test server.)
5. *Completed onboarding not repeated.* After finishing, restarts open the main settings. (e2e.)
6. *UI Language default.* With a fake Windows language "pl-PL" → Polski; "de-DE" → English. (Unit.)
7. *Live language switch.* Switching to English updates window text, tray labels and Overlay text without restart. (Frontend + menu model.)
8. *Translation completeness.* Every UI string key exists in both Polish and English (CI check). (Unit.)
9. *Settings salvage.* A settings file with one invalid value keeps the other values and resets only that one. (Unit.)
10. *Unreadable settings.* A garbage settings file is renamed `.broken` and defaults are used. (Unit.)
11. *No transcript in logs.* After a Dictation in a release-configured build, the log contains no part of the Transcript text. (WAV, fake Engine returning a unique marker string; grep log.)
12. *Window position.* A remembered position on a removed monitor opens centred on the primary monitor. (Unit of the placement check.)
13. *Recommendation follows the hardware.* With the compute-hardware fake reporting a graphics card, Whisper large-v3-turbo is marked Recommended and "Graphics card found" is shown; reporting none, Parakeet TDT 0.6B v3 is marked Recommended and "No graphics card found" is shown. (Frontend with mocked commands; unit of the hardware mapping.)
14. *Back.* From Choose a Model, "Back" shows Welcome; from Try it, "Back" shows Choose a Model with "Continue", which returns to Try it. (Frontend.)
15. *Finish later.* "Finish later" on Choose a Model completes onboarding without a Model and shows the Model & language page with "Download a Model to start". (Frontend.)
16. *Try it feedback.* Try it focuses its test field; the status line shows "Getting ready…", "Listening" and "Transcribing…" as the fake Dictation moves through them; text arriving in the field shows "Echo heard you." and 2.5 s later onboarding is completed and the main window shows, unless a new Dictation started or the user typed in the field first; "Nothing appeared?" reveals the Microphone, Model and log-folder actions. (Frontend.)
17. *Focus and progress.* After each step change the new step's heading has focus; completed steps carry a check mark. (Frontend.)
18. *Welcome follows the mode.* With Toggle Mode, Welcome says "press a shortcut … press it again"; with Push-to-Talk Mode, "hold a shortcut … let go". (Frontend.)

## Decisions

- **Onboarding:** the four steps Welcome, Microphone access, Choose a Model, Try it; the Record Shortcut becomes active as soon as a Model is active — legacy had only permission and Model steps.
- **Onboarding navigation:** Back on every step after the first and "Finish later" on Choose a Model — a first-timer is never trapped behind a large download; Dictation stays honestly unavailable until a Model is active.
- **Model recommendation:** Echo states what it found (graphics card or not) instead of asking the user to judge their own hardware.
- **Theme:** follow Windows light/dark only; manual override goes to the backlog.
- **Debug section:** none; only an "Open log folder" link in the App section.
- **UI languages:** Polish and English only, as in `docs/plan.md`.
