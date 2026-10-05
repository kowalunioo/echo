# Microphone

Choosing which audio input device a Recording listens to. By default Echo follows the Windows default recording device.

## Behaviour

1. The Microphone setting is either **Default** (follow the Windows default recording device) or one specific device identified by its name as Windows reports it.
2. The default is Default.
3. With Default, every Recording uses whatever device Windows considers the default recording device at the moment the Recording starts. Changing the Windows default between Dictations is picked up by the next Recording without restarting Echo.
4. With a specific device, every Recording uses that device.
5. If the selected specific device is not present when a Recording starts (unplugged, renamed, disabled), Echo records from the Windows default device instead, shows a short notice "Selected microphone not found — using the default microphone" in the Overlay, and keeps the user's selection unchanged so it is used again once the device returns.
6. If no input device exists at all, the Recording does not start and the error from `dictation-pipeline.md` rule 8 is shown.
7. If Windows privacy settings block microphone access for desktop apps, the Recording does not start and the user is told, with a button that opens the Windows microphone privacy page.
8. The device list shown to the user is refreshed every time the picker is opened, so newly plugged devices appear without restarting Echo.
9. Changing the Microphone takes effect from the next Recording; a Recording in progress continues on its current device.
10. **Device lost during a Recording:** if the device stops delivering audio or reports an error during a Recording (unplugged, USB/Bluetooth dropout), the Recording stops automatically; the audio captured up to that moment is transcribed and inserted as usual, and the user sees "Microphone disconnected — recording stopped" (error indication per `dictation-pipeline.md` rules 39a–39d).
11. After a device loss, the next Recording opens the device afresh using rules 3–5.
12. Echo opens the device only while recording (see `dictation-pipeline.md` rule 9).
13. Audio from multi-channel devices is averaged into mono (no per-channel choice in 0.1.0).

## Settings

| Setting | Values | Default |
|---|---|---|
| Microphone | Default, or any input device name present when the picker is opened | Default |

## UI

- A drop-down labelled "Microphone" with "Default (<current Windows default device name>)" first, then all input devices by name.
- If the stored specific device is currently missing, it still shows as the selected value, marked "(not connected)".
- A reset control returns to Default.
- During first run, if microphone access is blocked by Windows, a dedicated step explains it and offers "Open Windows privacy settings"; it re-checks automatically when the user returns (see `settings-and-first-run.md`).

## Acceptance tests

1. *Default follows system.* With a fake device enumerator whose default changes from A to B between two Recordings, the first Recording opens A and the second B. (Fake Audio Source factory.)
2. *Specific device.* With "B" selected and default "A", the Recording opens B. (Fake.)
3. *Missing device fallback.* With "C" selected but absent, the Recording opens the default, a notice is emitted, and the setting still says "C". When "C" reappears, the next Recording opens "C". (Fake.)
4. *No device.* With no devices, the Recording does not start and the "no microphone found" error is emitted. (Fake.)
5. *Mid-Recording loss.* A fake Audio Source delivers 2 s of speech then reports a device error; the Dictation proceeds to Transcribing with the 2 s, the notice is emitted, and the next Recording opens a fresh source. (Fake Audio Source, fake Engine, FI.)
6. *Real unplug.* With a USB microphone selected, unplugging it mid-Recording stops the Recording and inserts what was said. (HW, manual smoke test.)
7. *Picker refresh.* Opening the picker requests a fresh device list each time. (Frontend with mocked command.)
8. *Privacy block.* When the permission check reports "denied", the Recording does not start and the privacy message is emitted. (Fake permission check.)

## Decisions

- **Missing selected device:** fall back to the default for that Recording only and keep the selection — legacy erased the selection permanently.
- **Device lost mid-Recording:** stop, transcribe what was captured, notify the user (an error per `dictation-pipeline.md` rule 39a) — legacy gave no notice.
- **Channel selection, mute-while-recording, always-open microphone, laptop-lid microphone:** backlog — not in 0.1.0 scope.
