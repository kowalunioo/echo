# Dictation pipeline

The core cycle of Echo: the user starts a Dictation, speaks, stops it, and the Transcript is inserted into the focused application and stored in History. This spec owns the Dictation states, audio handling, voice-activity detection, Insertion, and the fake-microphone test mode with its acceptance tolerance.

## Behaviour

### States

1. A Dictation is always in exactly one of these states: **Idle**, **Recording**, **Transcribing**, **Inserting**. Echo as a whole is Idle when no Dictation is in progress.
2. Allowed transitions:
   - Idle → Recording: the Record Shortcut starts a Recording (see `record-shortcut.md`).
   - Recording → Transcribing: the Record Shortcut stops the Recording.
   - Recording → Idle: Cancellation, a failure to open the Microphone, or a Recording that captured no audio.
   - Transcribing → Inserting: the Engine returned a non-empty Transcript.
   - Transcribing → Idle: Cancellation, an empty Transcript, or an Engine error.
   - Inserting → Idle: Insertion finished (successfully or not).
3. Only one Dictation exists at a time. Echo never records two Dictations in parallel.
4. Every transition is reflected within 100 ms in the Overlay (see `overlay.md`) and in the tray icon (see `tray.md`).

### Starting

5. When a Recording is requested and no Model is active (none downloaded or none selected), the Recording does not start, the Microphone is not opened, and the user is told that a Model is needed, with a way to open the Models page.
6. When a Recording is requested and the active Model is not yet loaded into memory, Echo starts the Recording without waiting and starts loading the Model in the background 250 ms later, once the Overlay has slid in: loading a Model onto the GPU stalls the Overlay's rendering for about a second, which would freeze its entrance. Transcribing waits for the load to finish.
7. When a Recording starts, Echo opens the Microphone chosen in `microphone.md`. The Overlay shows a "getting ready" look until the first audio actually arrives from the device, then switches to the "listening" look. Audio is captured from the moment the device delivers it; nothing said before the first audio arrives can be captured.
8. If the Microphone cannot be opened (no input device, access denied by Windows privacy settings, device busy), the Dictation returns to Idle, the Overlay and tray return to idle, and the user sees an error naming the cause: "no microphone found", "microphone access is blocked in Windows privacy settings" (with a way to open those settings), or a generic failure with the detail.
9. The Microphone is opened only for the duration of a Recording and is released when the Recording ends, so the Windows "microphone in use" indicator is shown only while recording.

### Audio handling

10. Echo accepts any Microphone sample rate and channel count. Multi-channel input is mixed down by averaging all channels; the result is resampled to 16 kHz mono before voice-activity detection and before the Engine.
11. The same conversion applies to the WAV Audio Source (see "Fake-microphone test mode").
12. A Recording stops automatically after 10 minutes. This is a normal stop, not a Cancellation: the audio captured so far is transcribed and inserted as usual.
13. Echo never writes the audio of a Dictation to disk and never keeps it after the Dictation ends. History stores text only.

### Voice-activity detection

14. Voice-activity detection is always on in 0.1.0. During a Recording, only audio judged to be speech is kept for the Engine; silence between and around speech is dropped.
15. Speech start needs at least 60 ms of consecutive speech-like audio. When speech starts, the preceding 450 ms of audio is kept as well, so the first syllable is not clipped.
16. After speech stops, a further 450 ms of audio is kept before audio is dropped again, so word endings are not clipped.
17. If the detector fails on a piece of audio, that piece is treated as speech (fail open), so a detector fault never loses words.

### Short and silent Recordings

18. If a Recording ends with no kept speech audio at all (silence only, or a Recording so short that nothing was captured), the Engine is not run, nothing is inserted, nothing is added to History, and the Dictation returns to Idle without an error message.
19. If the kept speech audio is shorter than 1.0 s, it is padded with trailing silence to 1.25 s before it is given to the Engine. There is no minimum Recording duration otherwise.
20. A Transcript that is empty or whitespace-only after clean-up (rule 25) counts as "no speech": nothing is inserted, nothing is added to History, no error is shown.

### Transcribing

21. When the Recording stops, the Overlay switches to the "transcribing" look immediately, before the Engine starts.
22. The Engine receives the kept 16 kHz mono audio, the Dictation Language (resolved per `dictation-language.md`) and the Vocabulary (per `vocabulary.md`).
23. If the Engine fails (returns an error or crashes), nothing is inserted, nothing is added to History, the Dictation returns to Idle, and the user sees "Transcription failed" with the error detail. If the Engine crashed, the Model is unloaded and loaded again on the next Dictation.
24. A failed Dictation cannot be retried, because the audio is not kept.
25. Transcript clean-up, applied in this order after the Engine returns:
    1. Vocabulary correction where the Model needs it (see `vocabulary.md`).
    2. Any word repeated three or more times in a row (case-insensitive, letters only) collapses to one occurrence ("I I I I think" → "I think").
    3. Runs of whitespace collapse to a single space.
    4. Leading and trailing whitespace is removed.
26. Clean-up must never lose a successful Transcript: if any clean-up step fails, the uncleaned Engine text is used.

### New Record Shortcut press while Transcribing or Inserting

27. If the user starts a new Dictation while the previous one is Transcribing or Inserting, the request is remembered (not dropped) and the new Recording starts as soon as the previous Dictation returns to Idle. Only one such request is remembered.
28. Toggle Mode: a second press during the same busy period cancels the remembered request (two presses cancel each other out). Releases are ignored.
29. Push-to-Talk Mode: a press while busy is remembered as "held". If the key is released while still busy, the remembered request is forgotten (the user's hold ended before Echo could record). If the key is still held when the previous Dictation finishes, the Recording starts then and stops on release as usual.
30. Cancellation (any source) during the busy period also forgets the remembered request.

### Insertion

31. Insertion delivers the Transcript to whichever application has keyboard focus at the moment of Insertion (not necessarily the one focused when the Recording started).
31a. Until onboarding is complete, Insertion instead puts the Transcript into the onboarding's Try it field in the main window, wherever keyboard focus is (`settings-and-first-run.md` rule 2.4). If the main window cannot receive it, the Insertion fails as in rule 36.
32. The inserted text is the cleaned Transcript exactly. Echo adds no leading or trailing space and no newline.
33. Before inserting, Echo waits until the user has released all modifier keys (Ctrl, Alt, Shift, Win), for at most 1.5 s, so that held modifiers do not combine with the paste keystroke.
34. **Clipboard paste (primary method):**
    1. Echo saves the current clipboard contents in all formats it can read.
    2. Echo puts the Transcript on the clipboard as plain Unicode text, marked so that Windows excludes it from clipboard history, from cloud clipboard sync, and from clipboard-monitoring tools.
    3. Echo sends Ctrl+V to the focused application, holding Ctrl for about 100 ms so slow applications register the combination.
    4. Echo restores the saved clipboard only after the target application has read the Transcript from the clipboard (plus a quiet period of 200 ms after its last read, because some applications read several times), or after at most 8 s if no read is observed, or after 0.5 s if the keystroke could not be sent at all.
    5. If the clipboard changed by any other means in the meantime (for example the user copied something), Echo does not restore; the user's newer clipboard wins.
    6. If the clipboard was empty before, it is emptied again on restore.
    7. The restored content is not added to clipboard history as a new entry.
35. **Typing fallback:** if the clipboard cannot be opened or written, or the paste keystroke cannot be sent, Echo instead types the Transcript into the focused application as simulated Unicode keystrokes. Typing is not used when the paste succeeded.
36. If Insertion fails entirely (both methods), the text is never lost silently. This includes the case where Windows blocks simulated input into a window running as administrator; Echo detects that failure where Windows reports it and never runs elevated itself.
    1. Echo keeps that Transcript in memory, whatever the History limit, until the next Recording starts (then it is dropped; it is never written to disk except as the History entry of rule 38).
    2. The main-window notice offers **Copy text**, which puts the kept Transcript on the clipboard as plain text (an ordinary copy, like History's Copy). Once the next Recording has started, the notice no longer offers it.
    3. If History stored the Transcript (History limit above 0), the notice reads "Couldn't insert the text — it is in History." and also offers **Open History**.
    4. If History keeps nothing (History limit 0, `history.md` rule 9), the wording must not claim History holds it: "Couldn't insert the text. Copy it before your next Dictation." The Overlay message is "Couldn't insert the text — open Echo to copy it".
    5. Clicking the Overlay message for either case shows the main window, where the notice and its actions are.
37. After Insertion (successful or not), the Overlay hides and the tray icon returns to idle.

### History

38. A Transcript is added to History exactly when it is non-empty, the Dictation was not cancelled, and the History limit is above 0. It is added before Insertion starts, so a failed Insertion still leaves it in History when History is on (with History off, rule 36.1 keeps it for copying). Details in `history.md`.

### Cancellation

39. Cancellation is possible during Recording and Transcribing (see `cancel-shortcut.md`). After a Cancellation nothing is inserted and nothing is stored, even if the Engine finishes afterwards. Once Inserting has begun, Cancellation has no effect.

### Error indication

39a. A **Dictation error** is any of: no Model active at start (rule 5), Microphone could not be opened or was lost (rule 8, `microphone.md`), Engine failure (rule 23), Insertion failure (rule 36). Model download and Model load failures (`models.md`) are indicated the same way. Cancellation, silence and empty Transcripts are not errors.
39b. On an error, three things happen: a short message in the Overlay for 2.5 s (`overlay.md`); an error notice in the main window (shown immediately if it is open, otherwise the next time it is opened); and the tray icon switches to its red error variant with a tooltip naming the error (`tray.md`).
39c. The red tray icon stays until the user opens the main window (where the notice is shown) or the next Dictation completes successfully (inserted without error), whichever comes first. During a later Recording or Transcribing, the tray shows the normal recording/transcribing icon; if that Dictation fails, the error state returns with the new error.
39d. If several errors happen before the user looks, the tooltip and notice show the most recent one; the main window lists all errors since it was last opened.
39e. Each main-window notice is one plain line in the UI Language, with the technical detail (English, from the failing part) below it in smaller, selectable text; the message line alone is announced to screen readers, not the buttons beside it. Every notice offers the next step where one exists:
    - no Model, Model download failed, Model load failed: **Open Model settings** (the Model page);
    - Microphone access blocked: **Open privacy settings** (the Windows microphone privacy page);
    - no Microphone found, Microphone failed, Microphone disconnected: **Open Microphone settings** (the Dictation page, with focus on its Microphone section);
    - Transcription failed: **Open log folder**;
    - Insertion failed: **Copy text** and, when History stored it, **Open History** (rule 36).

## Fake-microphone test mode

40. Echo can run with a **WAV Audio Source** in place of the Microphone. This mode is selected at launch by a developer option (environment variable or command-line argument naming a WAV file); it is never offered in the normal UI.
41. While the WAV Audio Source is active, Echo shows a persistent marker (in the Overlay and the window title) so a build in this mode is never mistaken for normal operation.
42. The WAV Audio Source accepts PCM WAV files at any sample rate (at least 16, 44.1, 48 kHz), 16-bit integer or 32-bit float, mono or stereo. It goes through the same downmix, resampling and voice-activity detection as the Microphone.
43. When a Recording starts, the WAV Audio Source starts from the beginning of the file and delivers audio in real time (one second of audio per second). When the file ends, it delivers silence until the Recording stops.
44. A test option makes the Recording stop automatically when the file ends, so tests do not need to time the stop.
45. A second test option delivers the file as fast as possible instead of in real time, for unit tests that do not involve timing.
46. All other parts (Engine, Insertion, History) run unchanged. Insertion can additionally be replaced by a fake Inserter that records the text it received, and the Record Shortcut by a fake ShortcutListener that injects press/release events.

### Acceptance tolerance for fixture recordings

47. Fixtures live in `tests/fixtures/audio/` (see `docs/plan.md`). A test that needs a fixture skips with a clear "fixture missing" message when the file is absent.
48. Comparison uses a **normalised word error rate** (WER): word-level edit distance (substitutions + deletions + insertions) between the normalised expected and normalised actual text, divided by the number of words in the normalised expected text.
49. Normalisation, applied to both texts in this order:
    1. Unicode NFC; lowercase.
    2. Currency: "zł", "złoty", "złote", "złotych" → `zl`; "gr", "grosz", "grosze", "groszy" → `gr`. A decimal amount followed by a currency word, such as "129,99 zł" or "129.99 zł", becomes `129 zl 99 gr`.
    3. The word "i" (Polish "and") is removed when it directly follows `zl` and directly precedes a number.
    4. Abbreviation "nr" → "numer".
    5. Thousands separators inside digit groups are removed ("4 521", "4,521", "4.521" → "4521") when the group is exactly three digits.
    6. Times "15:00" or "15.00" → "15".
    7. Every character that is not a letter, a digit or whitespace is replaced by a space (this strips punctuation, including Polish quotation marks).
    8. Whitespace runs collapse to one space; leading and trailing whitespace is removed.
    9. Spelled-out numbers are not converted; the thresholds below absorb one such difference.
50. Thresholds, mandatory for the default Model (Whisper large-v3-turbo) with Dictation Language set as listed:

    | File | Dictation Language | Normalised expected words | Max WER | Extra condition |
    |---|---|---|---|---|
    | `pl-proste.wav` | Polish | 9 | 0.12 (1 error) | — |
    | `pl-interpunkcja.wav` | Polish | 12 | 0.17 (2 errors) | — |
    | `pl-slownik.wav` | Polish | 18 | 0.17 (3 errors) | Vocabulary set to Echo, GitHub, Claude Code, Tauri, Parakeet, Vulkan; every term must appear in the normalised output as a word that starts with the lowercased term (so "githuba", "vulkanie" count; "claude code" must appear as two consecutive words) |
    | `pl-liczby.wav` | Polish | 8 | 0.13 (1 error) | — |
    | `en-proste.wav` | English | 8 | 0.13 (1 error) | — |
    | `cisza.wav` | Polish | 0 | — | The Transcript must be empty: no Insertion, no History entry |

51. The same fixtures also run with Dictation Language set to automatic detection; there the thresholds are reported but not mandatory.
52. For the other two Models, `cisza.wav` (empty output) is mandatory; the WER rows are reported only.
53. The normalisation function itself has unit tests covering each rule in 49, including "129,99 zł" and "129 złotych i 99 groszy" both normalising to `129 zl 99 gr`.

## Settings

None owned by this spec in 0.1.0. Inputs come from: Record Shortcut and mode (`record-shortcut.md`), Microphone (`microphone.md`), Model (`models.md`), Dictation Language (`dictation-language.md`), Vocabulary (`vocabulary.md`), History limit (`history.md`).

## UI

- While a Dictation is in progress, the user sees the Overlay (`overlay.md`) and the tray icon state (`tray.md`); the main window is not required to be open.
- Errors (rule 39a) must reach the user even when the main window is hidden: a short Overlay message, the red tray icon with the error in its tooltip, and an error notice in the main window (rules 39b–39d). No Windows toast notifications.
- In fake-microphone mode, a visible "test audio" marker is shown.

## Acceptance tests

Fakes: WAV Audio Source (WAV), fake ShortcutListener (FSL), fake Inserter (FI). "HW" = needs real hardware or a real desktop session.

1. *Happy path, Push-to-Talk.* Given WAV `en-proste.wav`, FSL, FI, default Model; when FSL presses, waits for the file to end, then releases; then FI receives one text whose normalised WER ≤ 0.13, and History gains one entry with the same text. (WAV, FSL, FI; needs fixture and Model.)
2. *Silence.* Given WAV `cisza.wav`; when a full Recording of the file is made; then the Engine is not called or returns empty, FI receives nothing, History is unchanged, no error is shown. (WAV, FSL, FI.)
3. *Silence, synthetic.* Given a generated 3 s WAV of digital silence; then the Engine is not called at all. (WAV, FSL, FI; no fixture needed.)
4. *Short speech padding.* Given a generated WAV with 0.5 s of a speech-like signal accepted by the detector; then the audio passed to the Engine is exactly 1.25 s long. (WAV, fake Engine.)
5. *Downmix and resample.* Given a 48 kHz stereo WAV with a 1 kHz tone on the left channel only; then the audio delivered to voice-activity detection is 16 kHz mono with the tone at half amplitude and the same duration ±10 ms. (WAV; unit test.)
6. *Odd rates.* Same as 5 for 44.1 kHz mono, 8 kHz mono and 96 kHz 4-channel input. (WAV; unit test.)
7. *Engine error.* Given a fake Engine that fails; then FI receives nothing, History is unchanged, the state returns to Idle, and an error "Transcription failed" with the detail is emitted. (fake Engine, FI.)
8. *No Model.* Given no active Model; when FSL presses; then the Microphone/Audio Source is never opened, the state stays Idle, and a "Model needed" message is emitted. (WAV, FSL.)
9. *Microphone failure.* Given an Audio Source that fails to open; then the state returns to Idle and an error naming the cause is emitted. (fake Audio Source.)
10. *Busy press, Toggle Mode.* Given a slow fake Engine (2 s); when FSL presses, presses (stop), then presses once more during Transcribing; then a second Recording starts automatically after the first Insertion. When instead it presses twice during Transcribing, no second Recording starts. (FSL, fake Engine.)
11. *Busy press, Push-to-Talk.* Given a slow fake Engine; when the key is pressed during Transcribing and released before it finishes, no second Recording starts; when it is still held at the end, a Recording starts and stops on release. (FSL, fake Engine.)
12. *Cancellation during Transcribing.* Given a slow fake Engine; when Cancellation happens during Transcribing; then even though the Engine later returns text, FI receives nothing and History is unchanged. (FSL, fake Engine, FI.)
13. *Clean-up.* Engine returns "  so so so so we   go  "; FI receives "so we go". (fake Engine, FI.)
14. *Clipboard restore.* Given the clipboard holds "previous"; when a Dictation inserts "hello" into Notepad; then Notepad contains "hello", the clipboard holds "previous" afterwards, and Windows clipboard history (if enabled) does not contain "hello". (HW: real desktop session.)
15. *User copy wins.* Given an Insertion in progress; when something else is copied before restore; then the clipboard keeps the newer content. (HW.)
16. *Typing fallback.* Given the clipboard is held open by another process; when a Dictation inserts "zażółć gęślą jaźń"; then Notepad contains exactly that text. (HW.)
17. *Fixture suite.* For each row of the table in rule 50, an end-to-end run WAV → Engine → Insertion into Notepad meets the threshold. (WAV, FSL, HW for Notepad; needs fixtures and Model; skips cleanly otherwise.)
18. *Normaliser unit tests* for every rule in 49. (pure unit test.)
19. *Audio never persisted.* After test 1, no audio file was created in Echo's data directories or temp directory. (WAV.)
20. *Error indication.* Given a fake Engine that fails; then an Overlay message is emitted, the tray state becomes "error" with a tooltip containing "Transcription failed", and a notice is queued for the main window. When the main window is then opened, the tray returns to idle. (fake Engine, fake tray.)
20a. *Insertion failure keeps the text.* Given FI fails; then the error is "Insertion failed", the status names that error as the one whose Transcript is kept, and the kept text equals the Transcript; with History off the error is the "not in History" variant and History stays empty; when the next Recording starts the kept text is gone; a successful Insertion keeps nothing. (fakes.)
21. *Error cleared by success.* After test 20 without opening the window, a successful Dictation returns the tray to idle; a cancelled or empty Dictation does not. (fakes, fake tray.)
22. *Recording during error state.* With the tray in error state, starting a Recording shows the recording icon; when that Dictation succeeds the tray ends idle, when it fails the tray ends in error with the new message. (fakes, fake tray.)
23. *Auto-stop.* With a controllable clock, a Toggle Mode Recording reaches 10 minutes and moves to Transcribing by itself; the Transcript is inserted. (WAV, FSL, fake Engine, FI.)

## Decisions

- **No Model at shortcut press:** Overlay message "No Model — open Echo to download one" that opens the Models page when clicked, plus the error indication of rules 39a–39d — legacy refused silently.
- **Maximum Recording length:** auto-stop (not cancel) after 10 minutes and transcribe what was captured — a forgotten Toggle Mode Recording must not run forever.
- **Voice-activity detection:** always on, no setting — fewer knobs; revisit only if users report clipped words.
- **Trailing space:** none in 0.1.0; a trailing-space option goes to the backlog — keeps Insertion predictable.
- **Waiting for modifiers:** wait up to 1.5 s for modifiers to be released before pasting — held Ctrl/Alt otherwise corrupt the paste keystroke.
- **Clipboard:** restore all readable formats, restore only after the target app has read the Transcript (with the timeouts in rule 34), always exclude the Transcript from clipboard history — fixed delays could paste the old clipboard, and text-only restore lost data.
- **Where errors appear:** Overlay message, red tray icon with the error in its tooltip until the window is opened or the next Dictation succeeds, and a notice in the main window; no Windows toast notifications — the main window is usually hidden, so errors must be visible from the tray.
- **Tolerance scope:** thresholds are mandatory only for the default Model with a fixed Dictation Language; other Models and automatic detection are report-only — tighten after first real measurements.
- **Elevated target windows:** detect the failure where possible and use the rule 36 message; Echo never runs elevated — Windows blocks simulated input into administrator windows.
- **Failed Insertion with History off:** keep the Transcript in memory until the next Recording starts and offer "Copy text"; never claim History holds it — with a History limit of 0 the old "it is in History" message was false and the text was lost.
