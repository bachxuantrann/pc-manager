# PC Manager (Rust + Tauri) — hướng dẫn cho AI agent

App quản lý máy Windows 10/11 x64 giống Windows PC Manager: RAM, dọn rác, tiến trình, ứng dụng khởi động, app không dùng.
Mục tiêu: **nhẹ, nhanh, an toàn**. App tối ưu RAM thì bản thân không được ngốn RAM.

## Stack (không tự ý đổi)
- Tauri 2 + React + TypeScript + Vite (frontend), Tailwind cho style
- Rust stable, toolchain `x86_64-pc-windows-msvc`
- `windows` crate (windows-rs) cho Win32/NT API; `sysinfo` chỉ dùng tạm khi chưa có impl native
- `rusqlite` (feature `bundled`) cho lưu trữ, `serde`, `thiserror`, `anyhow` (chỉ ở tầng command), `rayon`, `jwalk`, `blake3`
- Chạy **1 tiến trình**, mặc định **quyền thường** (manifest `asInvoker`, mở app không hỏi UAC). Chức năng cần admin: UI hiện nút "Chạy với quyền Administrator" → `ShellExecuteExW` verb `runas` chạy lại chính exe (UAC hỏi), bản cũ thoát. Backend phải chạy được khi thiếu quyền (báo "cần quyền Administrator", không crash).
- **Không tách Windows Service** (quyết định của người dùng): app dùng cho cá nhân và vài người bạn; service chạy quyền hệ thống nhận lệnh từ người dùng thường làm tăng rủi ro bảo mật mà lợi ích không đáng. Không đề xuất lại trừ khi người dùng yêu cầu.

## Cấu trúc
```
pc-manager/
├── src/                      # React UI
│   ├── pages/  Dashboard, Memory, Cleaner, Processes, Startup, Apps, Settings
│   ├── components/  hooks/  lib/api.ts   # mọi invoke() gom ở api.ts, có type
├── src-tauri/
│   ├── build.rs              # nhúng app manifest (asInvoker + Common-Controls v6)
│   └── src/
│       ├── main.rs  lib.rs   # đăng ký commands, tray
│       ├── commands/         # mỏng: validate input -> gọi module -> map lỗi
│       ├── sys/              # TOÀN BỘ code unsafe/windows-rs nằm ở đây, bọc thành hàm safe
│       ├── memory/  cleaner/  process/  startup/  usage/  disk/
│       ├── store/            # SQLite
│       └── error.rs
└── rules/cleaner.toml        # rule dọn rác, nhúng bằng include_str!
```

## Quy tắc code
- `unsafe` chỉ xuất hiện trong `src-tauri/src/sys/`. Mỗi block `unsafe` có comment `// SAFETY:` giải thích.
- Mọi handle Win32 bọc trong struct có `Drop` (CloseHandle). Không leak handle.
- Không `unwrap()`/`expect()` ngoài test. Lỗi trả về UI dạng `{ code, message }`.
- Tauri command nặng (scan, hash) phải `async` + `tauri::async_runtime::spawn_blocking`, có progress qua `Channel`/event, có cancel token.
- Thread quét đĩa: gọi `SetThreadPriority(THREAD_MODE_BACKGROUND_BEGIN)` để không làm giật máy. Giới hạn rayon pool = max(2, cpus/2).
- Không polling khi cửa sổ bị ẩn/minimize. Dashboard refresh 2s khi đang hiển thị.
- Viết unit test cho logic thuần (parse rule, glob, tính tuổi file, decode UserAssist ROT13).

## AN TOÀN — bắt buộc, không được nới lỏng
1. Cleaner **chỉ xoá path khớp rule trong `rules/cleaner.toml`**. Không có command nào nhận path tùy ý từ UI để xoá. Ngoại lệ (bắt buộc gửi vào Recycle Bin qua `IFileOperation`/`SHFileOperationW` với `FOF_ALLOWUNDO`, không xoá vĩnh viễn):
   - tính năng "file lớn/trùng lặp";
   - "Dọn phần còn sót" sau khi gỡ app: backend tự tìm (InstallLocation, thư mục AppData/LocalAppData/ProgramData/Program Files trùng tên app, shortcut Start Menu trỏ vào thư mục cài, key `Uninstall`), lưu kết quả quét phía Rust; UI chỉ gửi `scanId` + id mục đã chọn. Mục "có thể" (khớp heuristic) mặc định không tick. Không bao giờ chọn thư mục là tổ tiên/chứa thư mục cài của app khác, thư mục dùng chung (Microsoft, Common Files, Packages…), hay thư mục đang có tiến trình chạy. Key registry phải export `.reg` backup trước khi xoá.
2. Luôn **Scan → Preview → người dùng xác nhận → Delete**. Có chế độ dry-run.
3. Không follow symlink/junction/reparse point. Bỏ qua file đang bị lock, bỏ qua file mới hơn `min_age_hours`.
4. Danh sách cấm cứng (không bao giờ xoá dù rule nói gì): `C:\Windows\System32`, `SysWOW64`, `WinSxS` (chỉ dọn qua `DISM /StartComponentCleanup`), `Prefetch`, `%USERPROFILE%` gốc, Documents/Desktop/Pictures, mọi ổ gốc `X:\`.
5. Kill process: chặn kill process hệ thống quan trọng (csrss, wininit, winlogon, lsass, smss, services, System, PID 0/4) và chính app.
6. Startup: tắt bằng `...\Explorer\StartupApproved\Run|Run32|StartupFolder` (byte đầu 0x02 = bật, 0x03 = tắt) giống Task Manager. **Không xoá** entry gốc trong `Run`.
7. Mọi thao tác thay đổi hệ thống ghi log vào SQLite (thời gian, hành động, đối tượng, kết quả).

## Chức năng & API Windows
- **Memory**: `GlobalMemoryStatusEx`, `GetPerformanceInfo`; standby/modified list qua `NtQuerySystemInformation(SystemMemoryListInformation)`.
  Boost = enable `SeProfileSingleProcessPrivilege` rồi `NtSetSystemInformation(SystemMemoryListInformation, MemoryPurgeStandbyList / MemoryFlushModifiedList)`.
  Smart Boost: chỉ chạy khi available < ngưỡng (mặc định 15%). Trim working set chỉ áp dụng cho process người dùng chọn — không trim hàng loạt.
- **Process**: liệt kê bằng `NtQuerySystemInformation(SystemProcessInformation)`; CPU% tính delta giữa 2 lần sample; kill (`TerminateProcess`), suspend/resume (`NtSuspendProcess`/`NtResumeProcess`), priority, mở vị trí file, kiểm tra chữ ký (`WinVerifyTrust`).
- **Startup**: Run/RunOnce HKCU+HKLM (+WOW6432Node), thư mục Startup (user + common), scheduled task trigger logon (COM `ITaskService`), service auto-start (chỉ đọc ở v1).
- **Apps không dùng**: app cài đặt từ registry `Uninstall` (HKLM, HKCU, WOW6432Node) với DisplayName, size, InstallDate, UninstallString. Last-used = max(UserAssist, Prefetch `.pf` last run time, sampling của app lưu SQLite mỗi 60s khi app chạy ngầm ở tray). Gỡ cài đặt = chạy UninstallString (hỏi xác nhận).
- **Cleaner**: rule engine TOML; dọn Temp, Windows\Temp, thumbnail cache, crash dumps, log, cache Chrome/Edge/Firefox (bỏ qua khi trình duyệt đang chạy), Windows Update cache (stop/start `wuauserv`), Delivery Optimization, Recycle Bin (`SHEmptyRecycleBin`).
- **Disk**: dung lượng các ổ, tìm file lớn (jwalk), file trùng (nhóm theo size → hash blake3 phần đầu → hash full).
- **Tray**: chạy nền, menu Boost nhanh / Mở / Thoát. Tuỳ chọn khởi động cùng Windows qua Task Scheduler (task "chạy với quyền cao nhất" để app có quyền admin mà không hỏi UAC; chỉ cho bật khi exe nằm trong Program Files; bật/tắt cần quyền admin).

## Build & kiểm tra
- Dev: `npm run tauri dev` (quyền thường). Cần thử chức năng admin: `scripts/admin-test.ps1 -Mode dev` hoặc bấm "Chạy với quyền Administrator" trong app.
- Release: `npm run tauri build` → installer NSIS ở `src-tauri/target/release/bundle/nsis/` (chỉ NSIS trên Windows — đặt trong `tauri.windows.conf.json`; NSIS có hook xoá task khởi động khi gỡ, MSI không có).
- Trước khi báo xong một task: `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npm run build` đều phải pass.
- Test cần quyền admin: đánh dấu `#[ignore = "cần quyền Administrator…"]`, chạy bằng `scripts/admin-test.ps1` (UAC hỏi mỗi lần, người dùng bấm Yes). Không tìm cách bỏ qua UAC.
- Release profile: `opt-level=3, lto="fat", codegen-units=1, panic="unwind", strip=true`. **Không đổi lại `abort`** (quyết định của người dùng): với abort, một panic ở bất kỳ luồng nào làm cả app tắt không dấu vết (lỗi thật ở v1.0.0).
- Chịu lỗi panic: khoá dùng `crate::sync::LockExt` (`lock_recover`/`try_lock_recover`, không coi mutex poisoned là lỗi vĩnh viễn); vòng lặp luồng nền bọc `sync::survive`; JoinError của `spawn_blocking` map bằng `error::task_failed` (lỗi `internal` thân thiện). Mọi panic được `crashlog` ghi vào `%APPDATA%\com.pcmanager.app\logs\crash.log`.
- Cắt chuỗi: không dùng `s[..n]` với vị trí có thể rơi giữa ký tự Unicode — dùng `s.get(..n)`; tìm vị trí trên bản `to_ascii_lowercase()` (giữ độ dài byte), không phải `to_lowercase()`.
- Mục tiêu hiệu năng: RAM của tiến trình Rust < 30MB khi rảnh; installer < 10MB.

## Lưu ý manifest (build.rs)
Dùng `tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"))`.
Manifest có `requestedExecutionLevel level="asInvoker"` VÀ dependency `Microsoft.Windows.Common-Controls` version 6.0.0.0 — thiếu cái này dialog/message box sẽ lỗi. Không đổi lại `requireAdministrator` (quyết định của người dùng: mở app không hỏi UAC).
