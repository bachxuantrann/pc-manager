# Chức năng & API Windows

Tham chiếu cho agent (tách khỏi `CLAUDE.md` để file đó gọn; quy tắc an toàn vẫn nằm ở `CLAUDE.md`).
- **Memory**: `GlobalMemoryStatusEx`, `GetPerformanceInfo`; standby/modified qua `NtQuerySystemInformation(SystemMemoryListInformation)`.
  Boost = `SeProfileSingleProcessPrivilege` + `NtSetSystemInformation` (`MemoryFlushModifiedList` + purge standby). Smart Boost (tay/tự động)
  chỉ chạy khi available < ngưỡng (mặc định 15%) và dùng **Boost nhẹ** (standby là cache đã tính vào RAM khả dụng — xoá hết chỉ mất cache).
  Trim working set chỉ cho process người dùng chọn.
- **Process**: `NtQuerySystemInformation(SystemProcessInformation)`; CPU% = delta 2 lần sample; kill, suspend/resume (`NtSuspendProcess`),
  priority, mở vị trí file (`%SystemRoot%\explorer.exe`), chữ ký (`WinVerifyTrust`).
- **Startup**: Run/RunOnce HKCU+HKLM (+WOW6432Node), thư mục Startup (user + common), task trigger logon (COM `ITaskService`), service
  auto-start, startup task app Store (`AppModel\SystemAppData`) — 3 nguồn cuối chỉ đọc.
- **Apps**: key `Uninstall` (HKLM, HKCU, WOW6432Node) + app Store (`AppModel\Repository\Packages` + manifest, không WinRT; gỡ qua Settings).
  Last-used = max(UserAssist, Prefetch, sampler SQLite 60s). Gỡ = chạy UninstallString (hỏi xác nhận). App HKCU khi PC Manager đang
  có quyền admin (cùng hay khác tài khoản) => **luôn** chạy dưới user đang đăng nhập, quyền thường (`spawn_as_shell_user`); lỗi thì báo,
  không bao giờ quay về token admin. HKLM thiếu quyền (mã 740) => mở lại qua UAC (`run_elevated_pid`).
- **Cleaner**: rule engine TOML; Temp, Windows\Temp, thumbnail, crash dump, log, cache trình duyệt (bỏ qua khi đang chạy; không đụng
  IndexedDB/Local Storage), Windows Update cache (stop/start `wuauserv`), Delivery Optimization, Thùng rác (`SHEmptyRecycleBin`).
- **Disk**: dung lượng ổ, file lớn (jwalk), file trùng (size → blake3 phần đầu → full). Ngưỡng tối thiểu kẹp ở command.
- **Tray**: Boost nhanh / Mở / Thoát. Khởi động cùng Windows qua Task Scheduler (task "quyền cao nhất"; chỉ khi exe trong Program Files;
  bật/tắt cần admin).

- **Cập nhật**: `tauri-plugin-updater` đọc `latest.json` của Release mới nhất (`plugins.updater` trong `tauri.conf.json`), kiểm
  chữ ký minisign rồi chạy bộ cài NSIS chế độ passive với `/UPDATE` (hook NSIS giữ task khởi động khi `$UpdateMode = 1`).
  Lịch/công tắc: `src-tauri/src/update.rs`; command: `commands/update.rs`; UI: `hooks/useUpdate.tsx`, `components/UpdatePanel.tsx`.
