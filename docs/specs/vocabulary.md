# Vocabulary

The user's list of words and phrases (names, product names, jargon — often English terms in Polish speech) that the Engine should favour and spell exactly as listed.

## Behaviour

### The list

1. The Vocabulary is an ordered list of entries. It is empty on first run.
2. An entry is a word or short phrase. When the user adds an entry, Echo trims it, collapses internal whitespace to single spaces, and removes the characters `<`, `>` and `"`.
3. An entry may be at most 50 characters after normalisation. Longer input cannot be added (the add action is disabled and a hint says why).
4. Duplicates are not allowed. Two entries are duplicates if they are equal ignoring letter case ("github" duplicates "GitHub"); the user is told the entry already exists and the list is unchanged.
5. Entries keep the exact capitalisation the user typed; that spelling is what Echo aims to produce.
6. Removing an entry takes effect for the next Dictation. Changes never affect a Dictation already Transcribing.
7. The list is saved immediately on every change and survives restarts.

### Using the list during a Dictation

8. **Models that accept a text prompt (Whisper large-v3-turbo, Whisper small):** the entries are given to the Engine as a hint, joined in list order with ", ". No post-correction is applied to these Models' output.
9. The hint has a budget: these Models read at most 224 prompt tokens; entries beyond the budget are silently ignored by the Engine. Echo estimates usage pessimistically as one token per 3 characters of the joined hint text, rounded up.
10. **Models without a prompt (Parakeet TDT 0.6B v3):** after transcription, Echo corrects words in the Transcript that closely resemble a Vocabulary entry, replacing them with the entry's spelling:
    1. Only entries made of ASCII letters and digits (ignoring spaces and punctuation inside the entry) take part; others are skipped for this correction.
    2. Comparison ignores case, spaces and punctuation, so one to three consecutive transcript words can match one entry ("Charge B" → "ChargeBee"; "cloud code" → "Claude Code").
    3. A match is accepted only if the words are very similar: an edit distance of at most 18% of the longer length, with phonetically similar English-sounding words allowed proportionally more tolerance, and lengths differing by at most 25% (or 2 characters, whichever is larger). The closest qualifying entry wins.
    4. A match never spans punctuation inside the group (a comma after the first word ends the candidate group).
    5. Punctuation directly before or after the matched words is kept.
    6. Capitalisation follows the transcript word: an all-capitals word yields the entry in capitals; a capitalised word yields the entry with its first letter capitalised; otherwise the entry's own spelling is used.
    7. An entry containing "&" also matches the spoken form with "and".
    8. Polish endings are preserved: if a transcript word is a close match for an entry followed by a common Polish ending (-a, -ie, -em, -u, -owi, -y, -ach, -ami, -om), the stem is corrected to the entry's spelling and the ending is kept ("githuba" → "GitHuba", "vulkanie" → "Vulkanie").
11. Vocabulary handling never causes a Dictation to fail; if correction fails, the uncorrected Transcript is used.

## Settings

| Setting | Values | Default |
|---|---|---|
| Vocabulary entries | list of up to N entries, each ≤ 50 characters | empty |

No hard cap on the number of entries; the budget indicator (UI) guides the user.

## UI

- A Vocabulary section with a text field and an "Add" button; pressing Enter in the field adds the entry.
- Entries are shown as removable chips in list order; each chip has a remove control with an accessible label "Remove <entry>". Removing is immediate; an "Undo" is offered for 5 s (as in History) and puts the entry back in its place. Keyboard focus moves to the next chip's remove control (or the previous one, or the text field when the list is empty).
- Below the list, a budget indicator (a bar filled to N%, capped at 100%) with the text "Vocabulary fills C of the 672 characters Whisper Models read", where C is the length of the joined hint and 672 is the 224-token budget at 3 characters per token. From 80% upward it turns into a warning: "Vocabulary is nearly full — Whisper Models ignore entries past the limit."
- When the active Model has no prompt (Parakeet), a short note explains that entries are applied as spelling corrections instead, and only for entries written in Latin letters and digits.
- Error/hint texts: duplicate entry, entry too long.

## Acceptance tests

1. *Normalisation.* Adding `  "Claude   Code" ` stores `Claude Code`. (Unit/frontend.)
2. *Duplicate.* With `GitHub` present, adding `github` is rejected with the duplicate message. (Unit/frontend.)
3. *Length.* A 51-character entry cannot be added; a 50-character one can. (Frontend.)
4. *Persistence.* Add entries, restart the app (or reload settings), the list is identical and in order. (Settings round-trip test.)
5. *Hint for Whisper.* With entries [Echo, GitHub, Tauri] and a fake Engine reporting "accepts prompt", the Engine receives the hint "Echo, GitHub, Tauri". (Fake Engine.)
6. *No correction for Whisper.* Fake Engine (accepts prompt) returns "uses git hub"; the Transcript stays "uses git hub". (Fake Engine.)
7. *Correction for Parakeet.* Fake Engine (no prompt) returns "I pushed it to git hub, then tauri."; with entries [GitHub, Tauri] the Transcript becomes "I pushed it to GitHub, then Tauri." (Fake Engine.)
8. *Case pattern.* With entry GitHub (no-prompt Model): transcript word "GITHUB" → "GITHUB"; "Github" → "GitHub"; "github" → "GitHub". (Unit.)
9. *Punctuation boundary.* Transcript "Charge B, che" with entry ChargeBee → "ChargeBee, che" is not produced by consuming "che"; "Charge B," becomes "ChargeBee,". (Unit.)
10. *Non-ASCII skipped.* Entry "Łódź" is not used for correction; Transcript unchanged. (Unit.)
10a. *Polish endings.* No-prompt Model, entries [GitHub, Vulkan]: "wrzuciłem na githuba" → "wrzuciłem na GitHuba"; "na vulkanie" → "na Vulkanie". (Unit.)
11. *Budget.* Entries whose joined text is 538 characters show 80% and the warning; 300 characters show 44% without warning. (Frontend unit test of the estimate: ceil(chars/3)·100/224, floored.)
12. *Fixture.* `pl-slownik.wav` with the six entries meets the condition in `dictation-pipeline.md` rule 50. (WAV; needs fixture and Model.)

## Decisions

- **Duplicates:** compared case-insensitively everywhere — legacy was inconsistent.
- **Polish inflection with Parakeet:** keep the Polish ending and correct only the stem (rule 10.8) — replacing "GitHuba" with "GitHub" is grammatically wrong.
- **Quick-add from selection:** the global "add selected text to Vocabulary" shortcut goes to the backlog — not in 0.1.0 scope.
- **Entry count:** no hard cap; the budget indicator guides the user.
- **Correction strength:** fixed value, no setting.
