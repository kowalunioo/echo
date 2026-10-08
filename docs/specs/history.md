# History

The stored list of recent Transcripts, so the user can recover, copy or re-insert text after a Dictation. Text only — Echo never stores audio.

## Behaviour

### What is stored

1. One History entry is created for each Dictation that produced a non-empty Transcript and was not cancelled (`dictation-pipeline.md` rule 38).
2. Not stored: cancelled Dictations, Dictations with no speech or an empty Transcript, Dictations whose Engine failed, and any audio.
3. An entry stores: a unique id, the creation time (UTC, shown in local time), the Transcript text exactly as inserted, the Model used, and the effective Dictation Language (or "automatic" plus the detected language if the Engine reported one).
4. An entry is stored even if Insertion later fails (when the limit is above 0). Whatever the limit, a Transcript whose Insertion failed can also be copied from the error notice until the next Recording starts (`dictation-pipeline.md` rule 36).
5. Entries are stored in a local SQLite database in Echo's data directory (`com.enloque.echo`). Nothing is sent anywhere.

### Limit

6. History keeps at most **N** entries, where N is the History limit. Default **5**; allowed range **0–100**.
7. When a new entry would exceed the limit, the oldest entries are deleted so exactly N remain.
8. Lowering the limit deletes the oldest surplus entries immediately. When that removes entries (the new limit is below the current number of entries), the page shows "N Transcripts removed" with an **Undo** for 5 s, as for Delete (rule 12). Undo puts the previous limit back and then restores the removed entries with their original ids and times. Lowering the limit again within those 5 s adds to the same Undo. This applies to the − stepper and to a typed value (applied on Enter or when the field loses focus), including 0.
9. With a limit of 0, no entries are kept: History stays empty and the tray's "Copy last Transcript" is disabled. Messages must then never say a Transcript "is in History" (see `dictation-pipeline.md` rule 36).
10. Deleted entries are removed permanently from the database (not soft-deleted).

### Actions

11. **Copy:** puts the entry's text on the clipboard as plain text (normal clipboard behaviour; it does appear in Windows clipboard history, because the user asked for it). A short confirmation "Copied" is shown.
12. **Delete:** removes the entry immediately without confirmation; an "Undo" is offered for 5 s. Keyboard focus moves to the next entry's Delete button (or the new last entry's, or Undo when the list is empty).
13. **Re-insert:** inserts the entry's text into the application that was focused before the user opened Echo's window, using the same Insertion method as a Dictation; Echo's window hides first so focus returns to that application.
14. **Clear all:** deletes every entry after a confirmation dialog. The dialog starts on Cancel, keeps Tab inside itself, closes on Escape or a click outside it, and returns focus to the button that opened it.
15. The tray menu item "Copy last Transcript" copies the newest entry's text (see `tray.md`).
16. The History list updates live: an entry added by a Dictation appears without reopening the page.

## Settings

| Setting | Values | Default |
|---|---|---|
| History limit | integer 0–100 | 5 |

## UI

- A History page listing entries newest first. Each entry shows its local date and time (format per UI Language, e.g. "5 października 2026, 14:03" / "October 5, 2026, 2:03 PM"), the full text (long texts wrap; very long ones are collapsible), and icon buttons for Copy, Re-insert and Delete with tooltips.
- An empty state: "No Transcripts yet. Press <Record Shortcut> and speak."
- The History limit control sits in the History page header: a number field with steppers, range 0–100, and a line "Keeps the last N Transcripts".
- A "Clear all" action with confirmation.

## Acceptance tests

1. *Stored on success.* After a successful Dictation (fake Engine "Hello"), History has one entry with text "Hello", the Model id and a timestamp within 2 s of now. (Fake Engine, FI.)
2. *Not stored.* Cancelled Dictation, empty Transcript, Engine error → History unchanged in each case. (Fakes.)
3. *Stored when Insertion fails.* Fake Inserter fails → entry exists. (Fakes.)
4. *Limit enforced.* With limit 5, after 7 Dictations exactly the 5 newest remain, in order. (Fakes.)
5. *Lowering limit.* With 5 entries, setting limit 2 leaves the 2 newest immediately. (Storage test.) In the UI, "3 Transcripts removed · Undo" appears; Undo within 5 s puts back limit 5 and the 3 entries with their original times; after 5 s they stay deleted. The same holds for the − stepper and for typing 0 and leaving the field. (Frontend.)
6. *Limit 0.* With limit 0, a Dictation leaves History empty; tray copy item disabled. (Fakes.)
7. *Range.* Values −1 and 101 are rejected by the setting; 0 and 100 accepted. (Unit.)
8. *Copy.* Copy places the exact text on the clipboard. (HW or clipboard fake.)
9. *Delete and undo.* Delete removes the entry from the list and DB; Undo within 5 s restores it with its original timestamp. (Storage + frontend.)
10. *Re-insert.* With Notepad focused before opening Echo, Re-insert of "Ala ma kota" hides Echo and Notepad receives the text. (HW.)
11. *Live update.* With the History page open, a new Dictation's entry appears without reload. (Frontend with mocked event.)
12. *No audio.* After any Dictation, no audio files exist in Echo's data directory. (WAV.)
13. *Fresh format.* The database is created from scratch under the new identifier; no data from any previous app is read. (Storage test.)

## Decisions

- **Stored fields:** text, time, Model, effective language; no title, no starring — enough to recover text; extras go to the backlog.
- **Limit:** range 0–100, default 5, count-based only; starring and time-based retention go to the backlog — simple and predictable.
- **Delete:** immediate, with a 5 s undo — no confirmation dialog for a small list.
- **Lowering the limit:** immediate, with the same 5 s Undo that restores the limit and the removed entries — one stray click on − used to lose a Transcript for good, and a confirmation dialog on every step would get in the way.
- **Re-insert:** added; retranscription dropped — no audio is kept.
- **Where the limit lives:** visible on the History page — legacy hid it in a debug section.
