; Installer hooks (tauri.conf.json, bundle.windows.nsis.installerHooks).
;
; hrm.exe is the file every open terminal session runs (`hrm run <account>`), and Windows keeps a running executable
; locked against writing. An update with a session open used to stop on "Error opening file for writing ...\hrm.exe":
; Retry could not work until every session was closed, and Ignore left the old hrm.exe next to the new app.
;
; Windows does allow renaming a running executable. So a locked hrm.exe is moved aside: the open sessions keep running
; from the renamed file, and the installer writes the new hrm.exe in its place. The copies moved aside are deleted by
; the next update or the uninstaller, once nothing runs from them (Delete skips a file that is still in use).

!macro NSIS_HOOK_PREINSTALL
  Delete "$INSTDIR\hrm.old-*.exe"
  IfFileExists "$INSTDIR\hrm.exe" 0 hrm_hook_done
    ; Opening for append fails while the file is running and changes nothing when it is not.
    ClearErrors
    FileOpen $0 "$INSTDIR\hrm.exe" a
    IfErrors hrm_hook_in_use
    FileClose $0
    Goto hrm_hook_done
  hrm_hook_in_use:
    ; A tick count keeps the name free when an earlier copy is still running.
    System::Call 'kernel32::GetTickCount()i.r0'
    Rename "$INSTDIR\hrm.exe" "$INSTDIR\hrm.old-$0.exe"
  hrm_hook_done:
!macroend

!macro HRM_PATH action
  Push $0
  Push $1
  nsExec::ExecToStack '"$INSTDIR\hrm.exe" path ${action}'
  Pop $0
  Pop $1
  StrCmp $0 "0" hrm_path_done_${action}
    DetailPrint "Could not ${action} hrm on the PATH ($0): $1"
    IfSilent +2
    ; a PATH entry left behind is harmless; an uninstaller that cannot finish is not, so this only warns
    MessageBox MB_OK|MB_ICONEXCLAMATION "Could not ${action} hrm on the PATH.$\r$\n$1"
  hrm_path_done_${action}:
  Pop $1
  Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  IfFileExists "$INSTDIR\hrm.exe" 0 hrm_add_done
    !insertmacro HRM_PATH add
  hrm_add_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  StrCmp $UpdateMode "1" hrm_remove_done
  IfFileExists "$INSTDIR\hrm.exe" 0 hrm_remove_done
    ; Tauri calls this again after the hook. Let the user cancel before PATH changes.
    !ifmacrodef CheckIfAppIsRunning
      !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
    !endif
    !insertmacro HRM_PATH remove
  hrm_remove_done:
  Delete "$INSTDIR\hrm.old-*.exe"
!macroend
