# Echo

Echo turns the user's speech into text and places that text where they are typing, entirely on their own machine.

## Language

### Dictation

**Dictation**:
One complete cycle from the moment the user starts speaking to Echo until the resulting text is inserted or discarded.
_Avoid_: Session, job, request

**Recording**:
The audio-capturing phase of a Dictation, between start and stop.
_Avoid_: Capture, take

**Transcript**:
The text produced from one Dictation's audio.
_Avoid_: Transcription (for the result), output, result

**Cancellation**:
Ending a Dictation on purpose so that nothing is inserted and nothing is kept in History.
_Avoid_: Abort, discard

**Insertion**:
Delivering a Transcript into the application that has keyboard focus.
_Avoid_: Paste, typing, output

### Shortcuts

**Record Shortcut**:
The key combination that starts and stops a Recording.
_Avoid_: Hotkey, keybind, trigger

**Toggle Mode**:
Record Shortcut behaviour where one press starts the Recording and the next press stops it.

**Push-to-Talk Mode**:
Record Shortcut behaviour where the Recording lasts exactly as long as the keys are held.
_Avoid_: PTT, hold mode

**Cancel Shortcut**:
The key combination that triggers Cancellation of the current Dictation.

### Recognition

**Engine**:
The component that converts speech audio into a Transcript using a Model.
_Avoid_: Backend, recognizer, ASR

**Model**:
A downloadable set of speech-recognition weights that an Engine runs.
_Avoid_: Weights, checkpoint

**Dictation Language**:
The spoken language the user tells Echo to expect, or automatic detection.
_Avoid_: Locale (that is the UI language)

**UI Language**:
The language of Echo's own interface (Polish or English), independent of the Dictation Language.

**Vocabulary**:
The user's list of words and phrases the Engine should favour when they are spoken.
_Avoid_: Dictionary, custom words, glossary

### Devices and surfaces

**Microphone**:
The audio input device a Recording listens to.
_Avoid_: Input, mic device

**Audio Source**:
Anything that supplies speech audio to a Dictation — a Microphone or, for testing, an audio file.

**Overlay**:
The small always-on-top indicator showing that a Dictation is recording or transcribing.
_Avoid_: HUD, popup, indicator window

**History**:
The stored list of recent Transcripts, capped at a user-chosen number of entries.
_Avoid_: Log, archive
