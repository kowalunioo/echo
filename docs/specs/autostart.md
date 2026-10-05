# Autostart

Optionally start Echo automatically when the user signs in to Windows, so the Record Shortcut is available without launching the app by hand. Opt-in.

## Behaviour

1. Autostart is off by default.
2. Turning it on registers Echo to start when the current Windows user signs in (per-user, no administrator rights needed). Turning it off removes that registration.
3. The change applies immediately when toggled; no restart needed.
4. On every launch, Echo re-applies the stored preference, so the registration self-heals if it was removed or points to an old install location. A failure to apply is logged and never blocks startup.
5. Turning autostart off when no registration exists is not an error and produces no warning.
6. When Echo is started by autostart, it starts **hidden in the tray**: no main window appears; the Record Shortcut works as soon as the Model has loaded.
7. When started by autostart while first-run setup is not finished (no Model selected), Echo shows the main window so the user can finish setup.
8. When the user starts Echo manually, the main window opens.
9. If the user disables Echo in Windows "Startup apps" settings, Echo respects that and does not re-enable itself; the toggle in Echo reflects the effective state and explains that it was turned off in Windows.
10. Uninstalling Echo removes the autostart registration.

## Settings

| Setting | Values | Default |
|---|---|---|
| Start Echo when I sign in to Windows | on, off | off |

## UI

- A toggle in the general/app settings with a one-line description: "Echo starts in the tray when you sign in."
- If Windows has disabled Echo's startup entry, the toggle shows off with a hint "Turned off in Windows Startup apps" and a link that opens that Windows settings page.

## Acceptance tests

1. *Enable/disable.* Toggling on creates the per-user startup registration for the current executable path; toggling off removes it. (Autostart abstraction with a fake registry; plus HW check of the real registration.)
2. *Self-heal.* With the setting on and the registration missing (or pointing to another path), launching Echo restores it. (Fake.)
3. *Disable without entry.* Turning off with no registration produces no error. (Fake.)
4. *Starts hidden.* Launching with the autostart marker starts with no visible main window and a tray icon present. (e2e.)
5. *First-run exception.* Launching with the autostart marker and no Model shows the main window. (e2e.)
6. *Windows-disabled state.* With the fake reporting "disabled by user in Windows", the toggle shows off with the hint and Echo does not re-register. (Fake.)
7. *Real sign-in.* After enabling, signing out and in starts Echo in the tray. (HW, manual.)

## Decisions

- **Hidden on autostart:** autostart launches always start hidden in the tray (except unfinished first run) — the point of autostart is a ready shortcut, not a window.
- **No "start hidden" setting:** manual launches show the window — one less option.
- **Windows Startup apps override:** respected; Echo never re-enables itself — the user's Windows choice wins.
