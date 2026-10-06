# Record Shortcut

The global key combination that starts and stops a Recording from any application, in Toggle Mode or Push-to-Talk Mode. This spec covers the two modes, the default, how the user changes the combination, which combinations are allowed, and which keystrokes reach other applications.

## Behaviour

### Modes

1. **Push-to-Talk Mode:** pressing the Record Shortcut starts a Recording; releasing it stops the Recording and the Dictation moves to Transcribing.
2. **Toggle Mode:** pressing the Record Shortcut starts a Recording; the next press stops it. Releases are ignored.
3. In both modes the Recording starts on key-down, not on release.
4. The mode can be changed at any time. If it is changed while a Recording is in progress, the Recording keeps going and the next press of the Record Shortcut stops it regardless of the mode it started in (so a Recording can never become unstoppable except by Cancellation).
5. The default mode is Push-to-Talk Mode.

### Key handling

6. The Record Shortcut works globally: in every application, whether or not Echo's window is open or focused.
7. The combination "is pressed" when exactly its keys are down: all of its modifiers and, if it has one, its main key. Extra modifiers held at the same time mean it is not the Record Shortcut (Ctrl+Shift+Space does not trigger Ctrl+Space).
8. Left and right variants of a modifier are equivalent (left Ctrl and right Ctrl both count as Ctrl).
9. Key auto-repeat while the shortcut is held never counts as a new press.
10. A second press arriving within 30 ms of the previous press is ignored (debounce).
11. In Push-to-Talk Mode, a release is acted on 50 ms after it happens; if a press of the same shortcut arrives within those 50 ms, both are ignored and the Recording continues (protection against keyboards that send a spurious release/press pair).
12. While the Record Shortcut is pressed, its keystrokes (press and release of its main key and of the modifiers that make up the combination) are **swallowed**: they do not reach the focused application. All other keystrokes pass through unchanged.
13. For modifier-only combinations that include Win or Alt, Echo makes sure releasing them does not open the Start menu or activate an application's menu bar.
14. While the shortcut-capture UI is active (see UI), the Record Shortcut does not trigger Dictations and is not swallowed.
15. If a different shortcut is pressed while a Recording made by the Record Shortcut is running, it does not affect the Recording (the Cancel Shortcut is the exception, see `cancel-shortcut.md`).
16. While Echo is busy Transcribing or Inserting, presses are remembered as specified in `dictation-pipeline.md` (rules 27–30).
17. The Record Shortcut only becomes active once first-run setup is finished (a Model is selected), see `settings-and-first-run.md`.

### Default and allowed combinations

18. The default Record Shortcut is **Ctrl+Space**.
19. Allowed combinations:
    - one or more modifiers (Ctrl, Alt, Shift, Win) plus one main key, e.g. Ctrl+Space, Ctrl+Alt+D;
    - modifier-only combinations of two or more modifiers (e.g. Ctrl+Win, Ctrl+Shift);
    - a single function key F1–F24 or another non-character key (Pause, Scroll Lock, Insert) without modifiers.
20. Not allowed (rejected with a message explaining why):
    - an empty combination;
    - a single character key or Space without a modifier (it would make that key unusable for typing);
    - a single modifier alone, either side (e.g. Ctrl, Shift, Win, right Alt, right Ctrl);
    - Escape alone (reserved as the default Cancel Shortcut);
    - combinations Windows reserves and does not deliver to applications (Ctrl+Alt+Delete, Win+L);
    - a combination identical to the Cancel Shortcut.
21. A combination is stored and shown in a canonical order: Ctrl, Alt, Shift, Win, then the main key.

### Changing the shortcut

22. A new combination takes effect immediately after it is accepted; no restart is needed.
23. If a new combination cannot be activated, the previous one stays active and unchanged, and the user sees why.
24. "Reset to default" restores Ctrl+Space (subject to the same validation).

## Settings

| Setting | Values | Default |
|---|---|---|
| Record Shortcut | any allowed combination (rule 19) | Ctrl+Space |
| Shortcut mode | Push-to-Talk Mode, Toggle Mode | Push-to-Talk Mode |

## UI

- The settings show the current Record Shortcut as readable key caps (e.g. "Ctrl + Space"), with the UI Language's key names where they differ.
- To change it, the user clicks the shortcut field. The field switches to a capture state saying "Press the new shortcut…" and shows the keys as they are held.
- Capture finishes when:
  - the user releases the main key of a combination that has one → that combination is proposed; or
  - the user releases all modifiers of a modifier-only combination without having pressed a main key → that modifier-only combination is proposed.
- The proposal is validated; if valid it is saved and shown; if invalid, the previous shortcut is kept and the reason is shown next to the field.
- Clicking anywhere outside the field, or the window losing focus, ends capture without changes. Escape pressed alone during capture also ends capture without changes.
- A reset button next to the field restores the default.
- A two-option control picks Push-to-Talk Mode ("Hold to record") or Toggle Mode ("Press to start, press again to stop"), each with a one-line explanation.

## Acceptance tests

1. *Push-to-Talk start/stop.* FSL press → state Recording; FSL release → after 50 ms state Transcribing. (FSL.)
2. *Release grace.* FSL press, release, press again 20 ms later → state stays Recording; release → Transcribing. (FSL, controllable clock.)
3. *Toggle.* In Toggle Mode, press → Recording; release → still Recording; press → Transcribing. (FSL.)
4. *Debounce.* Two presses 10 ms apart in Toggle Mode → one start, not start+stop. (FSL, controllable clock.)
5. *Auto-repeat.* Push-to-Talk, 20 repeated key-downs while held → one Recording, no restart. (FSL or hook unit test with synthetic events.)
6. *Exact match.* With Ctrl+Space, pressing Ctrl+Shift+Space does not start a Recording. (Hook unit test with synthetic key events.)
7. *Left/right.* Right Ctrl+Space triggers. (Hook unit test.)
8. *Mode change mid-Recording.* Start in Push-to-Talk, switch to Toggle while held, release → still Recording; press → stops. (FSL.)
9. *Swallowing.* With Notepad focused and Ctrl+Space as shortcut, a Push-to-Talk Dictation whose Transcript is empty leaves Notepad's text unchanged (no space typed). (HW.)
10. *Pass-through.* Typing "abc" while no shortcut is held reaches Notepad unchanged while Echo runs. (HW.)
11. *Win modifier-only.* With Ctrl+Win as shortcut, a full press/release does not open the Start menu. (HW.)
12. *Validation.* Proposals "Space", "A", "Escape", empty, left Ctrl alone, Shift alone, Win alone, right Alt alone, right Ctrl alone, and the current Cancel Shortcut are rejected with a message; "Ctrl+Alt+D", "F9" and "Ctrl+Win" are accepted. (Unit test of validation.)
13. *Failed change keeps old.* When activation of a new combination fails (fake ShortcutListener reports failure), the old combination still triggers. (FSL.)
14. *Capture UI.* Simulated key events Ctrl down, Space down, Space up → proposal "Ctrl+Space"; Ctrl down, Win down, Win up, Ctrl up → proposal "Ctrl+Win"; click outside → no change. (Frontend test with mocked events.)
15. *Inactive during capture.* While capture is active, pressing the current Record Shortcut starts no Recording. (FSL.)

## Decisions

- **Default mode:** Push-to-Talk Mode; the automatic hybrid mode (short tap toggles, long hold talks) goes to the backlog — 0.1.0 scope lists only the two modes.
- **Conflicts:** a Record Shortcut identical to the Cancel Shortcut is rejected; no detection of other applications' shortcuts in 0.1.0 — legacy did not check at all.
- **Escape during capture:** Escape alone cancels capture — Escape alone is not an allowed Record Shortcut anyway.
- **Swallowing:** the shortcut's keystrokes are swallowed in both modes — otherwise Ctrl+Space would also reach the focused application.
- **Modifier-only combinations:** allowed only as two or more modifiers — reduces accidental triggers during normal typing. A single right Alt or right Ctrl was allowed at first and dropped (issue #33): on a Polish keyboard right Alt is AltGr, needed for Polish characters, so it must stay free for typing; right Ctrl went with it to keep one rule. A stored single right Alt or right Ctrl falls back to the default like any other value that is not allowed.
