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

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$INSTDIR\hrm.old-*.exe"
!macroend
