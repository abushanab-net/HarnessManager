; Exercise the same hook order as Tauri using the real CLI as PAYLOAD.
; Run by scripts/installer-hooks-test.ps1, which builds it with and without HOOKS.
; Without HOOKS it is the installer as it was, to show the failure the hook fixes.
Unicode true
Name "hooktest"
OutFile "${OUT}"
RequestExecutionLevel user
SilentInstall silent
SilentUnInstall silent
Var UpdateMode
!ifdef HOOKS
  !include "${HOOKS}"
!endif

!macro RequireTestEnvironment
  ReadEnvStr $0 "HRM_HOME"
  StrCmp $0 "" unsafe
  ReadEnvStr $0 "HRM_PATH_TEST_KEY"
  StrCmp $0 "" unsafe safe
  unsafe:
    SetErrorLevel 2
    Abort
  safe:
!macroend

Section
  !insertmacro RequireTestEnvironment
  SetOutPath "$INSTDIR"
  !ifmacrodef NSIS_HOOK_PREINSTALL
    !insertmacro NSIS_HOOK_PREINSTALL
  !endif
  ; "try" skips a locked target instead of asking Abort/Retry/Ignore, which a silent test could not answer.
  SetOverwrite try
  File /a "/oname=hrm.exe" "${PAYLOAD}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  !ifmacrodef NSIS_HOOK_POSTINSTALL
    !insertmacro NSIS_HOOK_POSTINSTALL
  !endif
SectionEnd

Section Uninstall
  !insertmacro RequireTestEnvironment
  !ifmacrodef NSIS_HOOK_PREUNINSTALL
    !insertmacro NSIS_HOOK_PREUNINSTALL
  !endif
  Delete "$INSTDIR\hrm.exe"
  Delete "$INSTDIR\uninstall.exe"
SectionEnd
