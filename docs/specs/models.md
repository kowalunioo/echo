# Models

Selecting, downloading, verifying, switching and deleting the speech-recognition Models the Engine runs. 0.1.0 ships a fixed list of three Models.

## Behaviour

### The 0.1.0 Model list

1. Echo offers exactly these Models, in this order:

   | Model | Size on disk | Languages | Notes |
   |---|---|---|---|
   | Whisper large-v3-turbo (**default / recommended**) | 886 381 760 bytes (~845 MiB) | 100 | best accuracy; slower on CPU |
   | Parakeet TDT 0.6B v3 | 739 508 576 bytes (~705 MiB) | 25 European | fast and accurate; always auto-detects language; Vocabulary applied as spelling correction |
   | Whisper small | 269 751 136 bytes (~257 MiB) | 99 | small and fast; lower accuracy |

2. Each Model is a single GGUF file in 8-bit quantisation, downloaded from the Model publisher's Hugging Face repository at a pinned revision. Reference values (the implementer must confirm them against the publisher before shipping):

   | Model | Repository @ revision | File | SHA-256 |
   |---|---|---|---|
   | Whisper large-v3-turbo | `<publisher>/whisper-large-v3-turbo-gguf` @ `5eaf945c7978e564bae5b28a5b1639dd93c2bfb1` | `whisper-large-v3-turbo-Q8_0.gguf` | `b2e30cc286bc9f3aba4db9099fc7403543497c05ce7100d0d83091ddfd25a183` |
   | Parakeet TDT 0.6B v3 | `<publisher>/parakeet-tdt-0.6b-v3-gguf` @ `85ac09ea12fc4b1112fa76810059364bc6adc9de` | `parakeet-tdt-0.6b-v3-Q8_0.gguf` | `5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7` |
   | Whisper small | `<publisher>/whisper-small-gguf` @ `c0214bd34be9296695486f838e0142f900803159` | `whisper-small-Q8_0.gguf` | `9b9c8811bbcc82a7766f0fb0925614bdacb0923b2cc630daeac17108b655b860` |

   (`<publisher>` is the Hugging Face organisation of the transcribe-cpp authors — the Engine library chosen in ADR 0003 — as linked from that library's public documentation.)
3. The list, sizes, URLs and checksums are built into the app; there is no remote catalogue in 0.1.0.

### Storage

4. Model files are stored under `%LOCALAPPDATA%\com.enloque.echo\models\`. An in-progress download is stored next to its final name with a `.partial` suffix and renamed only after verification succeeds.
5. A Model counts as **downloaded** only when its final file exists and has the expected size. Verification by checksum happens at download time, not on every start.
6. Echo never reads Model files from any other application's folders or caches.

### Download

7. The user starts a download per Model. Only one download runs at a time; starting another while one runs queues it.
8. Before downloading, Echo checks free disk space; if less than the Model size plus 100 MB is free, the download does not start and the user is told how much space is needed.
9. **Resume:** if a `.partial` file exists, the download continues from its current length by requesting the remaining byte range.
   - If the server ignores the range request and sends the whole file, the partial file is discarded and the download restarts from zero.
   - If the server's reported start of the range does not match the partial length, or the server reports the range is invalid, the partial file is discarded and the download fails with a message (the next attempt starts from zero).
   - If the partial file is larger than the expected size, it is discarded and the download starts from zero.
   - If the partial file already has exactly the expected size, Echo skips the transfer and goes straight to verification.
10. **Integrity:** the download fails, and the partial file is deleted, if the server announces or sends a different total size than expected.
11. **Timeouts:** connecting must succeed within 15 s; if no data arrives for 60 s, the download fails as "stalled". A failed or stalled download keeps its partial file so a retry resumes.
12. **Verification:** after the last byte, Echo computes the SHA-256 of the whole file. On match, the file is renamed to its final name and the Model becomes downloaded. On mismatch, the file is deleted and the user sees "Download was corrupted — please try again".
13. **Progress:** while downloading, the UI shows bytes downloaded / total, a percentage and the current speed (MB/s), updated at least every 250 ms and at most every 100 ms. During verification the state reads "Verifying…".
14. **Cancel:** the user can cancel a running download at any time. The transfer stops within 1 s, the partial file is kept, and the Model shows "Paused — X% downloaded" with a "Resume" action.
15. Downloads continue while the main window is hidden. Quitting Echo stops the download; the partial file is kept for resume.
16. Network errors are shown with a plain-language reason and a "Retry" action. Download failures and Model load failures also trigger the error indication of `dictation-pipeline.md` rules 39a–39d (Overlay message, red tray icon, window notice).

### Active Model

17. Exactly zero or one Model is **active**. Dictations always use the active Model.
18. Making a Model active requires it to be downloaded. Echo loads it into memory immediately so the next Dictation is fast; while loading, the UI shows "Loading <Model>…".
19. If loading fails, the previously active Model stays active and the user sees "Couldn't load <Model>" with the reason.
20. Switching the active Model while a Dictation is in progress is not allowed; the switch control is disabled during Recording and Transcribing.
21. After the first Model finishes downloading during first run, it becomes active automatically.
22. Outside first run, a download finishing does not change the active Model, except when no Model is active — then the downloaded Model becomes active.
23. On start, if the active Model's file is missing, no Model is active; if another Model is downloaded, the first downloaded Model in list order becomes active.
24. The active Model stays loaded in memory while Echo runs, unless the "Unload Model after inactivity" setting says otherwise. It is always unloaded on quit.
24a. **Unload after inactivity:** with a value other than Never, the active Model is unloaded from memory once that much time has passed since the last Dictation ended (or since the Model was loaded, if no Dictation has happened since). The timer restarts at the end of every Dictation and is never triggered during Recording, Transcribing or Inserting.
24b. After an inactivity unload the Model stays active (it remains the selected Model); the next Dictation loads it again in the background while recording, as in `dictation-pipeline.md` rule 6. The Model indicator shows "Unloaded (loads on next Dictation)".
24c. Changing the setting takes effect immediately: the timer restarts with the new value; choosing Never cancels a pending unload but does not reload an already-unloaded Model.

### Delete

25. The user can delete a downloaded Model or a paused partial download. Deleting removes the final file and any partial file, after a confirmation that names the Model and the space freed.
26. Deleting the active Model unloads it; then the first remaining downloaded Model in list order becomes active, or none if there is no other.
27. Deleting is not possible while that Model is being used by a Dictation.
28. Deleting a Model whose files are already gone succeeds and simply marks it not downloaded.

### Acceleration

29. The Engine uses a GPU through Vulkan when one is available and falls back to the CPU otherwise; this is automatic, without a setting in 0.1.0. When the x64 build runs emulated on ARM64 Windows, the CPU is always used.

## Settings

| Setting | Values | Default |
|---|---|---|
| Active Model | one downloaded Model, or none | none until first download; recommended choice is Whisper large-v3-turbo |
| Unload Model after inactivity | Never, 2 min, 5 min, 10 min, 15 min, 60 min | Never |

## UI

- A Models page listing the three Models as cards: name, one-line description, size, number of languages, a "Recommended" badge on Whisper large-v3-turbo, and a state with its action:
  - Not downloaded → "Download (845 MB)";
  - Downloading → progress bar, "X% · Y MB/s", "Cancel";
  - Verifying → "Verifying…";
  - Paused → "Resume", "Delete";
  - Downloaded, not active → "Use this Model", "Delete";
  - Active → "Active" badge, "Delete";
  - Loading → "Loading…".
- A compact Model indicator in the main window shows the active Model and its state (ready / loading / error / none — "Download a Model to start").
- The tray menu offers switching between downloaded Models (see `tray.md`).
- On the Models page, a drop-down "Unload Model after inactivity" (Never / 2 / 5 / 10 / 15 / 60 minutes) with the description "Frees memory when you haven't dictated for a while. The next Dictation then takes a few seconds longer to start."

## Acceptance tests

Use a local HTTP test server serving a small generated file with known size and SHA-256 in place of the real URLs.

1. *Fresh download.* Download completes, checksum matches, final file exists, no `.partial`, Model is downloaded. (Test server.)
2. *Resume.* Stop the server after 40%; retry → client requests the remaining range, final file verifies. (Test server.)
3. *Server ignores range.* Server responds with full content → partial discarded, full download succeeds. (Test server.)
4. *Wrong range start.* Server returns a range starting elsewhere → partial deleted, error reported. (Test server.)
5. *Oversized partial.* Partial larger than expected → discarded, fresh download. (Test server.)
6. *Full-size partial.* Partial of exact size → no request made, verification only. (Test server.)
7. *Checksum mismatch.* Server serves wrong bytes of correct length → file deleted, "corrupted" error. (Test server.)
8. *Size mismatch.* Server advertises a different length → error, partial deleted. (Test server.)
9. *Stall.* Server stops sending for longer than the stall timeout (configurable in tests) → "stalled" error, partial kept. (Test server.)
10. *Cancel.* Cancel at 30% → transfer stops within 1 s, partial kept, state Paused; Resume continues from 30%. (Test server.)
11. *Disk space.* Fake free-space probe reports too little → download refused with the needed amount. (Fake.)
12. *Progress cadence.* Progress events arrive no more often than every 100 ms and at least every 250 ms during transfer. (Test server, throttled.)
13. *Activate.* Activating a downloaded Model loads it (fake Engine load called once); load failure keeps the previous active Model. (Fake Engine.)
14. *Busy lock.* During Recording, activate/delete requests are refused. (FSL, fakes.)
15. *Delete active.* With two downloaded Models, deleting the active one makes the other active; with one, none is active. (Fakes.)
16. *Missing file at start.* Active Model's file removed while Echo was closed → on start, the next downloaded Model is active or none. (Storage test.)
17. *Idle unload.* With a controllable clock and the setting at 5 min: 4 min 59 s after a Dictation ends the Model is still loaded; at 5 min it is unloaded and still active. (Fake Engine, fake clock.)
18. *No unload while busy.* With the setting at 2 min and a Recording lasting 3 min, the Model is not unloaded during the Recording; the timer starts when the Dictation ends. (FSL, fake Engine, fake clock.)
19. *Reload on next Dictation.* After an idle unload, pressing the Record Shortcut starts the Recording immediately and loads the Model in parallel; the Transcript is inserted. (WAV, FSL, fake Engine, FI.)
20. *Never.* With Never, no unload happens after 24 h of fake-clock inactivity; switching from 5 min to Never cancels a pending unload. (Fake Engine, fake clock.)
21. *Default.* Fresh settings have "Never". (Unit.)
22. *Load failure reported.* A download or load failure triggers the error indication (Overlay message, red tray icon, window notice) per `dictation-pipeline.md` rules 39a–39d. (Test server / fake Engine, fake tray.)
23. *Real download.* Downloading Whisper small from the real source verifies against the published SHA-256. (Network; manual or nightly, not in CI.)

## Decisions

- **Disk location:** `%LOCALAPPDATA%\com.enloque.echo\models\` only, no shared download cache — large files must not roam with the user profile.
- **Parallel downloads:** one at a time, others queued — simpler progress and less bandwidth contention.
- **Cancel:** pauses the download (partial file kept) with Resume and Delete — legacy kept the file but hid that it could resume.
- **Switching during a Dictation:** not allowed during Recording/Transcribing — a switch could invalidate the Model in use.
- **Idle unload:** a setting "Unload Model after inactivity" (Never / 2 / 5 / 10 / 15 / 60 min), default Never; the tray "Unload Model" item stays dropped — fast first word by default, memory relief for those who want it.
- **Deleting the active Model:** falls back to the next downloaded Model — legacy left none active.
- **Acceleration:** automatic Vulkan GPU with CPU fallback, CPU forced under ARM64 emulation; manual selector to backlog.
- **Other Models:** exactly the three listed Models in 0.1.0.
- **Source of the files:** download the GGUF files from the transcribe-cpp authors' Hugging Face repositories at pinned revisions — they are model weights, not code; confirm checksums before release.
