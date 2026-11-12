; Hook cho bộ cài NSIS (tauri.conf.json > bundle.windows.nsis.installerHooks).

; Gỡ PC Manager => xoá task "khởi động cùng Windows" (Task Scheduler, quyền cao nhất).
; Nếu không, task còn lại sẽ trỏ tới exe đã bị xoá. Tên phải khớp settings::TASK_NAME.
; Cập nhật (bộ cài mới chạy trình gỡ của bản cũ với /UPDATE): GIỮ task — app vẫn ở đúng chỗ cũ, xoá task thì
; người dùng mất "khởi động cùng Windows" sau mỗi lần cập nhật.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    nsExec::Exec '"$SYSDIR\schtasks.exe" /Delete /TN "PC Manager Autostart" /F'
    Pop $0
  ${EndIf}
!macroend
