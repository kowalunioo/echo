# Vocabulary Shortcut

A global key combination that adds the text selected in any application to the Vocabulary, without opening Echo. Typical use: Echo misspelled a name, the user corrects it in place, selects it and presses the Vocabulary Shortcut, and every later Dictation gets it right.

## Behaviour

### Triggering

1. The Vocabulary Shortcut works globally, in every application, whether or not Echo's window is open or focused.
2. Key handling follows the Record Shortcut: exact match, left/right modifiers equivalent, auto-repeat ignored, 30 ms debounce (`record-shortcut.md` rules 7–10). AltGr is not Ctrl+Alt: on layouts with AltGr, AltGr+D does not trigger Ctrl+Alt+D, whichever window has focus (including Echo's own).
3. The Vocabulary Shortcut acts on key press. Its keystrokes are swallowed (they do not reach the focused application), as in `record-shortcut.md` rules 12–13.
4. The copy (rule 6) starts only once every key of the shortcut has been released, so that held modifiers cannot turn the copy keystroke into a different combination. If the keys are still held 2 s after the press, the attempt is dropped silently.
5. The Vocabulary Shortcut does nothing while a Dictation is not Idle (Recording, Transcribing or Inserting), while another Vocabulary Shortcut attempt is still in progress, or while a shortcut-capture field is active. Its keystrokes are still swallowed when it does nothing because of a Dictation or an attempt in progress; nothing is remembered for later.

### Reading the selection

6. Echo reads the selection by copying it:
    1. It saves the current clipboard contents in all formats it can read and notes the clipboard's change counter.
    2. It sends Ctrl+Insert to the focused application and waits up to 250 ms for the clipboard to change.
    3. If the clipboard did not change and the focused window is not a terminal (a console window, Windows Terminal, or another window known to treat Ctrl+C as an interrupt), Echo sends Ctrl+C and waits up to a further 250 ms. In a terminal, Ctrl+C is never sent.
7. If the clipboard changed and holds text, that text is the selection. If it did not change, or holds no text (e.g. an image or files), nothing is selected.
7a. Code editors copy the whole current line when nothing is selected. So a copy marked by the editor as coming from an empty selection (VS Code and editors built on it put `isFromEmptySelection: true` in their `vscode-editor-data` clipboard format), or text ending in a line break, also counts as nothing selected.
8. Afterwards Echo restores the saved clipboard as in `dictation-pipeline.md` rules 34.5–34.7: not if something else changed the clipboard after the copy, emptied again if it was empty, and the restored content is not added to clipboard history as a new entry.
9. Echo never runs elevated; if the focused application runs as administrator and the copy keystrokes cannot reach it, nothing is selected.

### Adding the entry

10. The selection is normalised and checked exactly as when the user adds an entry in the Vocabulary section (`vocabulary.md` rules 2–5). Line breaks and tabs count as whitespace, so they collapse to single spaces. The text is added as selected: Echo does not strip Polish endings or change capitalisation.
11. The Vocabulary is unchanged, and the matching message (rule 13) is shown, when the normalised text is empty, longer than 50 characters, made of more than 3 words (words are separated by spaces after normalisation), or a duplicate of an existing entry (ignoring case).
12. Otherwise the entry is appended to the end of the Vocabulary, saved immediately, and takes effect from the next Dictation (`vocabulary.md` rules 6–7). If the main window is open, its Vocabulary list shows the new entry at once.

### Feedback

13. Every attempt that reaches rule 6 ends with exactly one Overlay message (`overlay.md` rule 5; shown for 2.5 s):
    - added: **Added "<entry>" to Vocabulary** / **Dodano „<entry>” do Słownika** — clicking it shows the main window on the Vocabulary page;
    - already present: **"<entry>" is already in your Vocabulary** / **„<entry>” jest już w Słowniku**, naming the existing entry with its stored spelling;
    - too long (over 50 characters or over 3 words): **Select one word or a short name** / **Zaznacz jedno słowo lub krótką nazwę**;
    - nothing selected: **Select a word first, then press <shortcut>** / **Najpierw zaznacz słowo, potem naciśnij <shortcut>**.
14. Where the entry would make a message longer than 60 characters, the entry is shortened with "…" so the message fits on one line.
15. These messages are shown even when the Overlay is turned off (`overlay.md` rule 17), because they are the only response to the user's action. They are not errors: neutral status dot, no tray error state, no main-window notice.
16. A Recording that starts while such a message is shown replaces it, as for any Overlay message.

### Settings

17. The default Vocabulary Shortcut is **Ctrl+Alt+D**, enabled.
18. Allowed combinations are those of the Record Shortcut (`record-shortcut.md` rules 19–21), including the ban on text-editing combinations (rule 20).
19. The Record, Cancel and Vocabulary Shortcuts must all differ from one another. A combination already used by one of the others is rejected; the previous combination stays and the message names the shortcut that uses it ("Already used by the Record Shortcut" / "Już używany przez skrót nagrywania"). A turned-off Vocabulary Shortcut still holds its combination for this check.
20. The user can turn the Vocabulary Shortcut off. While off, its combination is not intercepted and reaches the focused application normally.
21. A new combination, or turning it on or off, takes effect immediately; if a new combination cannot be activated, the previous one stays as in `record-shortcut.md` rule 23.

## Settings

| Setting | Values | Default |
|---|---|---|
| Vocabulary Shortcut | any allowed combination (rules 18–19) | Ctrl+Alt+D |
| Vocabulary Shortcut enabled | on / off | on |

## UI

- On the Dictation page, in the shortcuts section below the Cancel Shortcut: a row "Add selected text to Vocabulary" / "Dodaj zaznaczony tekst do Słownika" with a switch, the same capture field and reset button as the Record Shortcut (`record-shortcut.md` UI), and the description "Select a name in any app and press the shortcut. Select just the name, without a Polish ending." / "Zaznacz nazwę w dowolnej aplikacji i naciśnij skrót. Zaznacz samą nazwę, bez końcówki."
- While the switch is off, the capture field is disabled.
- Overlay messages as in rule 13, on light and dark. PR screenshots show the settings row (on and off), a rejected combination, and each of the four messages.

## Acceptance tests

1. *Adds the selection.* Fake selection reader returns "  GitHub\t"; FSL presses and releases the Vocabulary Shortcut → Vocabulary ends with "GitHub", settings saved, Overlay message "Added "GitHub" to Vocabulary" with the open-Vocabulary action. (FSL, fake selection reader, fake Overlay.)
2. *Waits for release.* FSL presses and holds the shortcut → the selection reader is not called; on release it is called once. (FSL.)
3. *Held too long.* FSL holds for 2.1 s → the selection reader is never called, no message. (FSL, controllable clock.)
4. *Duplicate.* With "GitHub" present, selection "github" → Vocabulary unchanged; message names "GitHub". (fakes.)
5. *Too long / too many words / empty.* A 51-character selection, a 4-word selection and an empty selection leave the Vocabulary unchanged and show the too-long, too-long and nothing-selected messages; a 50-character 3-word selection is added. (fakes.)
6. *Busy.* During a Recording, pressing the Vocabulary Shortcut does not call the selection reader and does not affect the Recording. (FSL, WAV.)
7. *Overlay off.* With "Show Overlay" off, the added message is still shown and is not an error. (Fake window.)
8. *Message length.* A 50-character entry yields messages of at most 60 characters ending the entry with "…", in both UI Languages. (Unit.)
9. *Copy order.* Fake copier: Ctrl+Insert changes the clipboard → Ctrl+C is not sent. Ctrl+Insert does nothing and the window is not a terminal → Ctrl+C is sent. Ctrl+Insert does nothing and the window is a terminal → Ctrl+C is not sent, nothing selected. (Unit with fake clipboard and window classifier.)
9a. *Whole-line copy.* Copied text with the VS Code empty-selection marker, or ending in a line break (LF or CRLF), gives the nothing-selected message and leaves the Vocabulary unchanged; the clipboard is still restored. (Unit with fake clipboard.)
10. *Uniqueness.* Setting any of the three shortcuts to a combination held by another (including a turned-off Vocabulary Shortcut) is rejected with a message naming the other shortcut; the previous value stays. (Unit + frontend.)
11. *Editing combinations.* Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+Z, Ctrl+Y, Ctrl+A, Ctrl+S, Ctrl+Insert, Shift+Insert and Shift+Delete are rejected for all three shortcuts; Ctrl+Alt+D is accepted. (Unit.)
12. *Off.* With the switch off, FSL pressing Ctrl+Alt+D triggers nothing and the keystrokes are not swallowed. (FSL / hook unit test.)
13. *AltGr.* A synthetic AltGr+D (fake LCtrl with the AltGr scan code plus RAlt) does not trigger Ctrl+Alt+D, both through the global hook and through keys delivered while Echo's own window has focus. (Hook unit test.)
14. *Settings round-trip.* Changed combination and switch survive a settings reload. (Settings test.)
15. *Live list.* With the main window open on the Vocabulary page, an entry added by the shortcut appears in the list without reopening the page. (Frontend + event.)
16. *Clipboard.* Given the clipboard holds "previous" and "Tauri" is selected in Notepad; pressing Ctrl+Alt+D adds "Tauri" and the clipboard holds "previous" afterwards. (HW: real desktop session; orchestrator.)

## Decisions

- **Source:** the selected text, read by copying it — no extra input window.
- **Default:** Ctrl+Alt+D, enabled out of the box.
- **Feedback:** Overlay message, shown even with the Overlay off; clicking "Added" opens the Vocabulary page (removing the chip there is the undo) — a hidden undo in a 2.5 s pill is easy to miss.
- **Copy keystroke:** Ctrl+Insert first, then Ctrl+C except in terminals — Ctrl+C with no selection interrupts the program running in a terminal. The copied text may still enter Windows clipboard history, because the copying application, not Echo, writes it.
- **Whole-line copies:** treated as nothing selected (rule 7a) — otherwise pressing the shortcut with no selection in an editor would add a line of code to the Vocabulary. Trade-off: a selection that really ends in a line break (e.g. a triple-clicked line) must be reselected without it.
- **Size limit:** 50 characters and at most 3 words — Vocabulary entries are names and terms; a sentence is almost certainly selected by mistake.
- **Polish endings:** added as selected; the user selects the stem. Guessing would break names that really end in -a or -u.
- **Placement:** with the other shortcuts on the Dictation page.
- **Busy:** ignored and swallowed during a Dictation — a copy would race the Insertion's use of the clipboard, and after Insertion the selection is usually gone.
- **Shortcut conflicts:** rejected with the name of the shortcut that holds the combination, never swapped; a turned-off shortcut still holds its combination, so it can always be turned back on.
- **Editing combinations:** banned for every Echo shortcut — Echo swallows its shortcuts, so they would take copy/paste away from every application.
