# Cancel Shortcut

A global key combination that triggers Cancellation of the current Dictation: nothing is inserted and nothing is stored in History. Cancellation is also available from the Overlay and the tray menu.

## Behaviour

1. Pressing the Cancel Shortcut while a Dictation is Recording triggers Cancellation: the Recording stops, the captured audio is discarded, the Microphone is released, the Overlay hides, the tray icon returns to idle, and the Dictation returns to Idle.
2. Pressing the Cancel Shortcut while a Dictation is Transcribing triggers Cancellation: the Overlay hides and the tray icon returns to idle at once; whatever the Engine produces afterwards is discarded.
3. After a Cancellation, nothing is inserted, nothing is added to History, and no error message is shown.
4. Once a Dictation has reached Inserting, Cancellation has no effect.
5. Cancellation also forgets a Record Shortcut press that was remembered while busy (see `dictation-pipeline.md` rule 30).
6. The Cancel Shortcut acts on key press; its release does nothing.
7. The Cancel Shortcut is only active while a Dictation is Recording or Transcribing. When Echo is Idle, the combination is not intercepted at all and reaches the focused application normally (Escape keeps working everywhere).
8. While the Cancel Shortcut is active and pressed, its keystrokes are swallowed (they do not reach the focused application).
9. After a Cancellation, the next press of the Record Shortcut starts a fresh Dictation normally, in both modes. In Push-to-Talk Mode, releasing the Record Shortcut after a Cancellation does nothing.
10. The default Cancel Shortcut is **Escape**.
11. Allowed combinations: the same rules as the Record Shortcut (`record-shortcut.md` rule 19), plus Escape alone. It must differ from the Record Shortcut.
12. Changes take effect for the next Dictation; a change made during a Dictation applies from the next one.
13. Other ways to trigger the same Cancellation: the cancel button on the Overlay (`overlay.md`) and "Cancel" in the tray menu (`tray.md`). All three behave identically.

## Settings

| Setting | Values | Default |
|---|---|---|
| Cancel Shortcut | any allowed combination (rule 11) | Escape |

## UI

- Shown in the shortcut settings next to the Record Shortcut, with the same capture field and reset button (`record-shortcut.md` UI). During capture of the Cancel Shortcut, Escape alone is accepted as a value (and therefore does not cancel the capture; clicking outside does).
- A short description: "Cancels the current Dictation. Active only while recording or transcribing."

## Acceptance tests

1. *Cancel while Recording.* FSL press Record (Toggle Mode), then Cancel → state Idle, Audio Source closed, FI received nothing, History unchanged. (WAV, FSL, FI.)
2. *Cancel while Transcribing.* Slow fake Engine; Cancel during Transcribing → Overlay hidden immediately; Engine later returns text; FI receives nothing; History unchanged. (FSL, fake Engine, FI.)
3. *Too late.* Cancel arriving after Inserting started → Insertion completes and History has the entry. (FSL, fake Inserter that blocks.)
4. *Inactive when Idle.* With Echo Idle, Escape pressed in Notepad reaches Notepad (e.g. closes an open dialog). (HW.)
5. *Swallowed while active.* During a Recording with a browser focused, Escape does not reach the browser. (HW.)
6. *Push-to-Talk release after Cancel.* Hold Record, press Cancel, release Record → state stays Idle, no Transcribing. (FSL.)
7. *Fresh start after Cancel.* After test 1, pressing Record starts a new Recording. (FSL.)
8. *Validation.* Cancel Shortcut equal to the Record Shortcut is rejected; Escape is accepted. (Unit test.)
9. *Remembered press is forgotten.* During Transcribing, Record pressed (remembered), then Cancel → no new Recording starts afterwards. (FSL, fake Engine.)
10. *Overlay and tray cancel* produce the same results as tests 1 and 2. (FSL plus UI command.)

## Decisions

- **Active during Transcribing:** the Cancel Shortcut works during Recording and Transcribing — users expect Escape to stop a slow transcription.
- **Feedback:** Cancellation is silent; the Overlay simply disappears — the user asked for it, no message needed.
- **Swallowing Escape:** while a Dictation runs, Escape does not reach the focused application — accepted trade-off of a shortcut active only during a Dictation.
