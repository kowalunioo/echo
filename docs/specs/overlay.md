# Overlay

A small always-on-top indicator that shows a Dictation is recording or transcribing, gives a cancel button, and briefly shows short messages. It must never take keyboard focus from the application the user is dictating into.

## Behaviour

### States

1. The Overlay is hidden when Echo is Idle and no message is pending.
2. **Getting ready:** shown as soon as a Recording is requested, until the first audio arrives from the Audio Source. It shows the visible label "Getting ready…" next to a muted, pulsing Echo mark, and the cancel button; there is no level meter and no timer yet, so the user can see the Microphone is not live.
3. **Listening:** shown while Recording once audio flows. The label gives way to a live level meter (about 9 bars) reacting to the input level, updated about 30 times per second with smoothing, the Echo mark turns to full colour, and an elapsed-time counter (m:ss) and the cancel button are shown. The pill keeps its width, so the change from label to meter is the cue that audio is live; the meter fades in (a plain fade, also with reduced motion). "Listening" is announced to assistive technology.
4. **Transcribing:** shown from the moment the Recording stops until Insertion completes. It displays a spinner, the label "Transcribing…", and the cancel button (active while Transcribing; hidden once Inserting starts).
5. **Message:** a single short line of text with no controls (it must fit on one line in the Overlay window: at most 60 characters in every UI Language, leading with what to do when the message has an action and dropping details the main window notice already gives), used for notices and errors defined in other specs (errors are also indicated on the tray icon and in the main window, see `dictation-pipeline.md` rules 39a–39d) (e.g. "Selected microphone not found — using the default microphone", "Transcription failed", "No Model — open Echo to download one", "Allow microphone access in privacy settings"). A message stays for 2.5 s then hides, unless a new Recording takes over the Overlay earlier. Clicking a message that has an associated action (e.g. "open Models page", or "show the main window" for an Insertion failure, `dictation-pipeline.md` rule 36) performs it. A clickable message stays a button for assistive technology; its text line is what is announced.
6. When a Dictation ends (inserted, empty, cancelled or failed), the Overlay fades out over about 300 ms. If a new Recording starts during the fade, the Overlay immediately shows "Getting ready" again; a stale hide never hides a newer Dictation's Overlay.
7. The Overlay appears with a short fade/slide-in (about 150–200 ms); motion is reduced to a plain fade when Windows "Show animations" is off.

### Focus and input

8. The Overlay never takes keyboard focus: showing, updating, moving or hiding it leaves the focused application and its caret unchanged, so Insertion lands where the user was typing.
9. The Overlay does not appear in the taskbar or in Alt+Tab.
10. Clicking the cancel button triggers Cancellation (same as `cancel-shortcut.md`) without moving keyboard focus away from the application the user was typing in.
11. Apart from its buttons, the Overlay does not block mouse clicks to the windows beneath it outside its visible shape.
12. The Overlay stays above other windows, including after other always-on-top windows appear, and above full-screen windows that are not exclusive full-screen games.

### Position and size

13. The Overlay appears horizontally centred on the monitor that contains the mouse pointer at the moment it is shown.
14. Position setting **Bottom** (default): the Overlay's bottom edge sits 12 logical pixels above the bottom edge of that monitor's work area (so it never overlaps the taskbar). Position **Top**: it sits a few pixels below the top edge of that monitor.
15. The compact Overlay is about 256 × 50 logical pixels (the working state may be narrower); it scales with the monitor's display scaling and with the Windows accessibility "Text size" setting, so it is crisp and correctly sized on every monitor, including when moving between monitors with different scaling.
16. Changing the position setting moves the Overlay immediately if visible.

### Visibility setting

17. The user can turn the Overlay off. When off, the getting ready, listening and transcribing states and non-error notices are not shown, but error messages (`dictation-pipeline.md` rule 39a) are still shown. Errors are additionally indicated on the tray icon and in the main window regardless of this setting.
18. The Overlay's text follows the UI Language and its colours follow the app's light/dark appearance.

## Settings

| Setting | Values | Default |
|---|---|---|
| Show Overlay | on, off | on |
| Overlay position | Bottom, Top | Bottom |

## UI

- The look is calm and minimal: a rounded pill with a soft neutral background, the level meter or label in the centre, and the timer and a small "×" cancel button on the right. On the left, the recording states show Echo's mark (the logo's three bars, middle one lavender), muted while getting ready; Transcribing shows a spinner; messages show a small status dot (red for errors). One row at a time; the pill animates its width between states.
- The pill's edge is a ring in the control colour (`--echo-control`, 3:1 or more against a white or a dark editor in both appearances) rather than a hairline, so it stays visible over any window. There is no outer shadow: the window is clipped to the pill's shape (rule 11).
- Settings: a toggle "Show the Overlay" and a choice "Position: bottom / top".
- PR screenshots must show each state: getting ready, listening, transcribing, message, on light and dark.

## Acceptance tests

1. *State sequence.* With WAV + FSL + slow fake Engine, the Overlay goes hidden → getting ready → listening (after first audio) → transcribing → hidden. (Overlay controller unit test with a fake window; plus screenshot in e2e.)
2. *Getting ready until first audio.* A fake Audio Source that delays its first audio by 1 s keeps the Overlay in "getting ready" for that second. (Fake Audio Source.)
3. *Focus never stolen.* With Notepad focused and the caret mid-text, a full Dictation shows/hides the Overlay; afterwards Notepad is still the foreground window and the text is inserted at the caret. (HW.)
4. *Cancel button keeps focus.* Clicking "×" during Recording cancels and the previously focused window is still foreground. (HW.)
5. *Not in taskbar/Alt+Tab.* The Overlay window has no taskbar button and is not listed in Alt+Tab. (HW.)
6. *Monitor choice.* With two monitors and the mouse on the second, the Overlay is centred on the second. (HW, or unit test of the placement function with fake monitor geometry.)
7. *Placement maths.* For a fake monitor at (−2560, 0) 2560×1440 at 150% scaling, the Bottom position is horizontally centred and fully above the taskbar's work-area edge. (Unit.)
8. *Stale hide.* Dictation ends, a new Recording starts 100 ms later → the Overlay remains visible. (Overlay controller unit test with controllable clock.)
9. *Message timing.* A message shows for 2.5 s then hides; a Recording starting at 1 s replaces it. (Unit with clock.)
10. *Disabled.* With "Show Overlay" off, no Overlay window is shown for Recording/Transcribing or non-error notices, but an Engine-failure message is still shown. (Fake window.)

## Decisions

- **Elapsed timer:** shown in the compact Overlay — useful in Toggle Mode.
- **Bottom placement:** 12 px above the bottom edge of the monitor's work area — legacy's fixed offset could overlap the taskbar.
- **Errors when the Overlay is off:** error messages are still shown; only the recording/transcribing states and non-error notices are suppressed — errors must stay visible.
- **Getting ready vs Listening:** a visible label while getting ready, replaced by the meter when audio flows — colour and pulse alone were too subtle, and users started talking before the Microphone was live.
- **Overlay message length:** one line, at most 60 characters; measured in Segoe UI Variable Text 13 px medium, the longest message is about 380 px of the ~436 px a clickable message has in the 520 px window.
- **Live text / extra styles:** compact style only; live text goes to the backlog — no 0.1.0 Model streams.
