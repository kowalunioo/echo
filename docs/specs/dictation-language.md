# Dictation Language

The spoken language the user tells Echo to expect, or automatic detection where the active Model supports it. Independent of the UI Language.

## Behaviour

1. The Dictation Language setting holds the user's intent: either "Automatic" or one specific language.
2. The default is "Automatic".
3. The intent is stored as chosen and never rewritten because of the active Model. Switching Models and switching back restores the same effective behaviour.
4. For each Dictation, Echo resolves the intent against the active Model into the **effective language**:
   1. If the intent is a specific language the Model supports, that language is used.
   2. Otherwise, if the Model supports automatic detection, automatic detection is used.
   3. Otherwise, English is used if the Model supports it; failing that, the Model's first supported language.
5. Language matching ignores regional variants: an intent "en" matches a Model language "en-US"; "no" and "nb" (Norwegian) are treated as equivalent.
6. Per-Model support in 0.1.0:

   | Model | Languages | Automatic detection | Specific language honoured |
   |---|---|---|---|
   | Whisper large-v3-turbo | 100 (incl. Polish, English) | yes | yes |
   | Whisper small (multilingual) | 99 (incl. Polish, English) | yes | yes |
   | Parakeet TDT 0.6B v3 | 25 European languages (incl. Polish, English) | yes | no — the Model always detects the language itself |

7. With automatic detection, the language may differ between Dictations; the Transcript is in whatever language the Engine detected.
8. A specific Dictation Language makes the Engine transcribe in that language; if the user speaks another language, the result may be poor or translated — Echo does not override this.
9. Changing the Dictation Language takes effect from the next Dictation; a Dictation already Transcribing keeps the language it started with.
10. Echo never translates in 0.1.0.

## Settings

| Setting | Values | Default |
|---|---|---|
| Dictation Language | "Automatic" or one language from the active Model's list | Automatic |

## UI

- The Dictation Language picker lives with the active Model's settings and is titled with the Model's name ("Language for Whisper large-v3-turbo").
- It lists "Automatic" first (only if the Model supports detection), then the Model's languages by their names in the UI Language, sorted alphabetically, with a search field that filters as the user types; Enter picks the first match, Escape closes the list.
- Polish and English are pinned at the top under "Automatic".
- The picker shows the **effective** language for the active Model. If the stored intent is not supported by the active Model, a hint explains what will be used instead ("Polish is not available for this Model — Automatic will be used").
- For a Model that ignores the chosen language (Parakeet), the picker is replaced by a note: "This Model detects the language automatically."
- A reset control returns to "Automatic".

## Acceptance tests

1. *Resolution table.* Unit tests of the resolution rule (4) for: intent "pl" with each 0.1.0 Model → "pl"; intent "ja" with Parakeet → automatic; intent "pl" with a fake English-only Model without detection → "en"; intent "Automatic" with a fake Model without detection and no English → its first language.
2. *Variant matching.* Intent "en" with a Model listing "en-US" resolves to "en-US". (Unit.)
3. *Intent preserved.* Set "pl", switch to a Model without Polish, then back; the setting still says "pl" and the effective language is Polish again. (Settings test with fake Model list.)
4. *Engine receives language.* With intent "pl" and Whisper, the fake Engine is called with language "pl"; with "Automatic" it is called with automatic detection. (Fake Engine.)
5. *Fixture: forced Polish.* `pl-proste.wav` with Polish meets its threshold. (WAV; needs fixture and Model.)
6. *Fixture: automatic.* `en-proste.wav` and `pl-proste.wav` with Automatic produce English and Polish text respectively (reported, see `dictation-pipeline.md` rule 51). (WAV; needs fixtures and Model.)
7. *UI.* With Parakeet active, the picker is replaced by the note; with Whisper active, "Automatic", "Polish", "English" are the first three options. (Frontend.)

## Decisions

- **Default:** "Automatic" — users often speak mixed Polish and English; fixture tests still set the language explicitly.
- **Parakeet ignores the language:** the picker is hidden for Parakeet and replaced by a note — offering a choice that has no effect is misleading.
- **Translate to English:** backlog — not in 0.1.0 scope.
- **Pinned languages:** Polish and English pinned under "Automatic" — quick access for the main audience.
