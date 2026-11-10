; Hook cho bộ cài NSIS (tauri.conf.json > bundle.windows.nsis.installerHooks).

; Gỡ PC Manager => xoá task "khởi động cùng Windows" (Task Scheduler, quyền cao nhất).
; Nếu không, task còn lại sẽ trỏ tới exe đã bị xoá. Tên phải khớp settings::TASK_NAME.
!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec '"$SYSDIR\schtasks.exe" /Delete /TN "PC Manager Autostart" /F'
  Pop $0
!macroend
