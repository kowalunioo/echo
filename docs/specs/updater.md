# Updater

A simple in-app updater: Echo checks for a newer signed release, downloads and installs it, and restarts — without ever interrupting a Dictation. Releases are signed with Echo's new updater key.

## Behaviour

1. Echo checks for updates against the release feed of the public Echo repository on GitHub (the latest release's update manifest).
2. An update is accepted only if its signature verifies against Echo's own updater public key built into the app. An update with a missing or invalid signature is rejected and never installed; the user is told "Update could not be verified".
3. Only newer versions (semantic versioning) are offered; Echo never downgrades.
4. **Automatic checks** (when enabled): 30 s after start, then every 4 hours while Echo runs.
5. When an automatic check finds an update, Echo downloads it in the background. When the download is complete, Echo installs it and restarts **only when Idle** (no Recording, Transcribing or Inserting in progress and no Model download running). If not Idle, it waits until Echo has been Idle for 10 s.
6. After an automatic restart, Echo comes back in the same visibility state it had (hidden in the tray stays hidden) and shows a brief notice "Echo was updated to <version>" when the window is next opened.
7. **Manual check:** "Check for updates…" (tray menu or settings) checks immediately and shows the result in the main window: "Checking…", then "Echo is up to date" (for 3 s) or "Version <x> is available — Install and restart". Installing from a manual check happens only when the user confirms.
8. While downloading an update for installation, the UI shows progress in percent, then "Installing…".
9. A network or server error during a check is silent for automatic checks (logged only) and shown as "Couldn't check for updates" for manual checks.
10. Automatic checks can be turned off; manual checks remain available.
11. The updater can be disabled entirely by an environment variable for managed/packaged installs; then automatic and manual checks are off, the tray item is hidden, and the setting shows "Updates are managed by your system".
12. Settings, History and Models are preserved across updates.

## Settings

| Setting | Values | Default |
|---|---|---|
| Check for updates automatically | on, off | on |

## UI

- In settings (about/app area): current version, the automatic-updates toggle, a "Check for updates" button, and the update status line (checking / up to date / available with Install / downloading x% / installing). While an update is available or ready, the Install button takes the place of "Check for updates": only one of them shows at a time. While an update downloads, a progress bar sits under the status line. The version row also says when a check (manual or automatic) last got an answer from the release feed: "Last checked: today at 14:02", or with the date when it was not today; failed checks do not count, and it is remembered only while Echo runs (hidden until the first check after a start).
- The tray menu item "Check for updates…" opens the main window and runs a manual check.

## Acceptance tests

Use a local HTTP server that serves an update manifest and package signed with a test key; the test build embeds the test public key.

1. *Up to date.* Manifest version equals current → manual check shows "up to date". (Test server.)
2. *Newer, valid signature.* Manual check shows "available"; confirming downloads, installs, and requests a restart. (Test server; install step via a fake installer.)
3. *Invalid signature.* Package signed with another key → rejected, "could not be verified", nothing installed. (Test server.)
4. *No downgrade.* Manifest with an older version → not offered. (Test server.)
5. *Automatic install waits for Idle.* With an update ready and a Recording in progress (FSL), no restart happens; after the Dictation ends and 10 s Idle pass, the restart is requested. (FSL, fake installer, controllable clock.)
6. *Automatic checks off.* With the setting off, no automatic request reaches the server within the schedule; manual check still works. (Test server, clock.)
7. *Environment disable.* With the disabling variable set, no requests are made, the tray item is hidden, and the toggle is read-only with the explanation. (Test server.)
8. *Silent automatic error.* Server returns 500 on an automatic check → no user-visible message. Manual check → "Couldn't check for updates". (Test server.)
9. *Release smoke test.* Installing release N, then publishing N+1, the installed app updates itself. (HW, part of the release checklist.)

## Decisions

- **Automatic installation:** download in the background, install and restart only after 10 s Idle, no prompt — legacy could restart mid-transcription.
- **After update:** brief "Echo was updated to <version>" notice only; release-notes dialog goes to the backlog.
- **Check interval:** every 4 hours, first check 30 s after start — a tray app runs for days; a delayed first check keeps startup fast.
