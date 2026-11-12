; Test harness for app/src-tauri/installer-hooks.nsh: runs the pre-install hook the way Tauri's template does, then
; copies PAYLOAD as hrm.exe into the folder passed with /D=. The caller compares file hashes afterwards.
; Run by scripts/installer-hooks-test.ps1, which builds it with and without HOOKS.
; Without HOOKS it is the installer as it was, to show the failure the hook fixes.
Unicode true
Name "hooktest"
OutFile "${OUT}"
RequestExecutionLevel user
SilentInstall silent
!ifdef HOOKS
  !include "${HOOKS}"
!endif

Section
  SetOutPath "$INSTDIR"
  !ifmacrodef NSIS_HOOK_PREINSTALL
    !insertmacro NSIS_HOOK_PREINSTALL
  !endif
  ; "try" skips a locked target instead of asking Abort/Retry/Ignore, which a silent test could not answer.
  SetOverwrite try
  File /a "/oname=hrm.exe" "${PAYLOAD}"
SectionEnd
