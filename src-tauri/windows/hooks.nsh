; NSIS hooks for Echo's installer (tauri.conf.json > bundle.windows.nsis.installerHooks).

; Uninstalling Echo removes its sign-in registration (docs/specs/autostart.md rule 10). The value
; is named after the app identifier (src-tauri/src/autostart/commands.rs); Windows keeps the
; user's Startup apps choice for it under StartupApproved.
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "com.enloque.echo"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "com.enloque.echo"
!macroend
