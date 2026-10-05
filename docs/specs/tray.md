# Tray

Echo lives in the Windows notification area (system tray). The tray icon shows whether a Dictation is in progress and gives quick access to common actions; closing the main window keeps Echo running in the tray.

## Behaviour

### Icon

1. Echo always shows a tray icon while it runs.
2. The icon has four states: **idle**, **recording**, **transcribing** (Transcribing and Inserting both show "transcribing") and **error** (a red variant). It changes within 100 ms of the Dictation state changing.
3. The icon has light and dark variants and follows the Windows taskbar theme (light taskbar → dark glyph, dark taskbar → light glyph), switching live when the user changes the Windows theme.
4. Hovering the icon shows the tooltip "Echo <version>", plus the current state when not idle ("Echo 0.1.0 — recording"), or the most recent error in the error state ("Echo 0.1.0 — Transcription failed").
4a. **Error state:** when an error from `dictation-pipeline.md` rule 39a happens (no Model, Microphone problem, Engine failure, Insertion failure, Model download or load failure), the icon switches to the red error variant. It stays red until the user opens the main window (where the error notice is shown) or the next Dictation completes successfully. While a later Dictation is Recording or Transcribing, the recording/transcribing icon is shown instead; when that Dictation ends, the icon returns to idle if it succeeded or to error if it failed.
4b. The red error variant is clearly distinguishable from the recording state on both light and dark taskbars (different colour and an exclamation mark glyph, not colour alone).
5. Left-click (single or double) opens the main window, restoring it if minimised and bringing it to the front.
6. Right-click opens the tray menu.

### Menu

7. When Idle, the menu contains, in order:
   1. "Echo <version>" (disabled, informational)
   2. separator
   3. "Copy last Transcript"
   4. separator
   5. "Model" submenu, labelled with the active Model's name, listing all downloaded Models with a check mark on the active one; choosing another makes it active (see `models.md`). Disabled with "No Model downloaded" when there are none.
   6. separator
   7. "Settings…" — opens the main window.
   8. "Check for updates…" — runs a manual update check (see `updater.md`).
   9. separator
   10. "Quit Echo"
8. While a Dictation is Recording or Transcribing, the menu additionally shows "Cancel" right after the version line (with separators), and the Model submenu is disabled (`models.md` rule 20).
9. "Copy last Transcript" copies the newest History entry's text to the clipboard. It is disabled when History is empty.
10. "Quit Echo" exits the app: a Recording in progress is cancelled (nothing inserted or stored), an in-progress Transcribing is abandoned, downloads stop (partial files kept), the Model is unloaded, the tray icon is removed.
11. Menu labels follow the UI Language and update when it changes.

### Main window and tray

12. Closing the main window (title-bar X or Alt+F4) hides it; Echo keeps running in the tray, shortcuts keep working. It does not quit.
13. The first time the window is closed to the tray, Echo shows a one-time hint: "Echo is still running in the tray. Use Quit Echo from the tray menu to exit."
14. Launching Echo again while it is already running (from the Start menu or a shortcut) does not start a second copy; it opens the existing main window.

## Settings

None in 0.1.0.

## UI

Described in Behaviour; no extra screens.

## Acceptance tests

1. *State icons.* Driving the Dictation state Idle → Recording → Transcribing → Idle (FSL + fake Engine) changes the tray icon state accordingly. (Tray abstraction with a fake tray, or HW.)
2. *Theme.* With a fake theme source switching light ↔ dark, the icon variant switches. (Fake.)
2a. *Error state.* A fake Engine failure turns the icon to error with a tooltip containing the error; opening the main window returns it to idle. A second scenario: without opening the window, a later successful Dictation returns it to idle; a cancelled Dictation leaves it in error. (Fakes, fake tray.)
2b. *Download failure.* A failed Model download (test server) turns the icon to error with a tooltip naming the Model. (Test server, fake tray.)
3. *Busy menu.* While Recording, the menu contains "Cancel" and the Model submenu is disabled; when Idle, no "Cancel". (Menu model unit test.)
4. *Cancel item.* Activating "Cancel" during Recording performs a Cancellation (same results as `cancel-shortcut.md` test 1). (FSL, fakes.)
5. *Copy last Transcript.* With History ["older", "newest"], the item copies "newest"; with empty History it is disabled. (Fake clipboard.)
6. *Model switch.* Choosing another downloaded Model makes it active. (Fakes.)
7. *Close hides.* Closing the window hides it; the process keeps running and the Record Shortcut still works. (HW/e2e.)
8. *Single instance.* Launching a second instance focuses the first and the second exits. (HW/e2e.)
9. *Quit during Recording.* Quit while Recording → no Insertion, no History entry, process exits. (FSL, fakes.)
10. *Localised labels.* Switching UI Language to Polish changes the menu labels (e.g. "Zakończ Echo"). (Menu model unit test.)

## Decisions

- **Tray icon always shown:** no option to hide it — a hidden Echo without an icon would be unreachable.
- **Close-to-tray hint:** shown once on first close — users otherwise think Echo quit.
- **Unload Model item:** dropped (the idle-unload setting lives in `models.md`).
- **Menu accelerators:** no Ctrl+, / Ctrl+Q labels in the tray menu — they were not real global shortcuts.
- **Error state:** the tray icon turns red with the error in its tooltip until the main window is opened or the next Dictation succeeds — the main window is usually hidden, so the tray must show that something went wrong.
