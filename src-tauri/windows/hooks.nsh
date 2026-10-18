; Chidori was called Kikitori before its rename. Tauri names the install folder, the uninstall entry and
; the shortcuts after the product name, so an update from Kikitori would leave it installed beside
; Chidori. Remove it silently: a silent uninstall keeps the app data, which Chidori moves to its own
; folders on first launch (src-tauri/src/migrate.rs).
!macro NSIS_HOOK_POSTINSTALL
  ReadRegStr $R8 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Kikitori" "InstallLocation"
  ; Stored quoted, as "C:\...\Kikitori".
  StrCpy $R9 $R8 1
  ${If} $R9 == '"'
    StrCpy $R8 $R8 -1 1
  ${EndIf}
  ${If} $R8 != ""
  ${AndIf} ${FileExists} "$R8\uninstall.exe"
    ; _? runs the uninstaller in place, so ExecWait waits for it instead of for a temp copy.
    ExecWait '"$R8\uninstall.exe" /S _?=$R8'
    Delete "$R8\uninstall.exe"
    RMDir "$R8"
  ${EndIf}
!macroend
