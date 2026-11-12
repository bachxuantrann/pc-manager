# PC Manager (Rust + Tauri) — hướng dẫn cho AI agent

App quản lý máy Windows 10/11 x64 giống Windows PC Manager: RAM, dọn rác, tiến trình, khởi động, app không dùng.
Mục tiêu: **nhẹ, nhanh, an toàn**. App tối ưu RAM thì bản thân không được ngốn RAM.

> **Kế hoạch: [`docs/PLAN.md`](docs/PLAN.md)**. Được giao "làm giai đoạn N" thì đọc file đó trước; xong cập nhật bảng trạng thái trong cùng commit. Máy mới / cài môi trường: `CONTRIBUTING.md`.

## Stack (không tự ý đổi)
- Tauri 2 + React + TypeScript + Vite + Tailwind. Rust stable `x86_64-pc-windows-msvc`, edition 2021, MSRV 1.85.
- `windows` (windows-rs) cho Win32/NT (không `sysinfo`, WMI, WinRT); `windows-registry`; `webview2-com` chỉ để hạ mức RAM WebView2;
  `rusqlite` (`bundled`), `serde`, `toml` (chỉ parse), `thiserror`, `anyhow` (chỉ tầng command), `rayon`, `jwalk`, `blake3`;
  `tauri-plugin-updater` (native-tls) cho tự cập nhật — chỉ kiểm tra khi cửa sổ mở, không tự cài, kiểm chữ ký minisign.
- **1 tiến trình, quyền thường**: manifest `asInvoker` + dependency `Microsoft.Windows.Common-Controls` 6.0.0.0 (thiếu => dialog lỗi),
  nhúng qua `tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"))`. **Không đổi lại `requireAdministrator`**
  (quyết định của người dùng: mở app không hỏi UAC). Cần admin: nút "Chạy với quyền Administrator" → `ShellExecuteExW` `runas` chạy lại exe,
  bản cũ thoát. Backend phải chạy được khi thiếu quyền (báo "cần quyền Administrator", không crash).
- **Không tách Windows Service** (quyết định của người dùng): app cho cá nhân và vài người bạn; service quyền hệ thống nhận lệnh từ
  người dùng thường tăng rủi ro bảo mật mà lợi ích không đáng. Không đề xuất lại trừ khi người dùng yêu cầu.

## Cấu trúc
Sơ đồ: `CONTRIBUTING.md`. `src/lib/api.ts` gom mọi `invoke()`; `src/dev/demo.ts` dữ liệu `?demo`. `src-tauri/src/commands/` mỏng
(validate → gọi module → map lỗi); `sys/` chứa TOÀN BỘ unsafe/windows-rs; `user/` ngữ cảnh user đang đăng nhập. Rule: `rules/cleaner.toml`.

## Quy tắc code
- `unsafe` chỉ trong `src-tauri/src/sys/`, mỗi block có `// SAFETY:`. Mọi handle Win32 bọc struct có `Drop`.
- Không `unwrap()`/`expect()` ngoài test. Lỗi về UI dạng `{ code, message }` (`AppError`; thiếu quyền => `Forbidden`, UI xét `code`).
- Command nặng: `async` + `spawn_blocking`, tiến độ qua `Channel`, có cancel. Quét đĩa: `THREAD_MODE_BACKGROUND_BEGIN`; rayon = max(2, cpus/2).
- Không polling khi cửa sổ ẩn/minimize; Dashboard refresh 2s khi hiển thị. Unit test cho logic thuần (rule, glob, tuổi file, ROT13…).
- Chịu lỗi panic: khoá dùng `crate::sync::LockExt` (`lock_recover`/`try_lock_recover`); vòng lặp nền bọc `sync::survive`;
  JoinError của `spawn_blocking` map bằng `error::task_failed` (dùng `commands::blocking`). Panic ghi `%APPDATA%\com.pcmanager.app\logs\crash.log`.
- Cắt chuỗi: không `s[..n]` ở vị trí có thể giữa ký tự Unicode — dùng `s.get(..n)`; tìm vị trí trên `to_ascii_lowercase()` (giữ độ dài byte).
- Rust bám skill `.claude/skills/rust-skills/` (đã rút gọn, xem `SOURCE.txt`); file này luôn thắng. Thư viện mới, tokio, tracing,
  framework test nặng, PGO, `target-cpu=native`, nightly, cú pháp edition 2024: chỉ khi người dùng đồng ý.

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
Chi tiết API theo từng chức năng: `docs/FEATURES.md` (đọc khi sửa module đó). Quy tắc phải nhớ:
- Smart Boost (tay/tự động) chỉ chạy khi available < ngưỡng (mặc định 15%) và dùng **Boost nhẹ**; trim working set chỉ cho process người dùng chọn.
- Gỡ app HKCU khi PC Manager đang có quyền admin (cùng hay khác tài khoản) => **luôn** chạy dưới user đang đăng nhập, quyền thường
  (`spawn_as_shell_user`); lỗi thì báo, không bao giờ quay về token admin. HKLM thiếu quyền (740) => mở lại qua UAC (`run_elevated_pid`).
- Khởi động cùng Windows (task "quyền cao nhất"): chỉ cho bật khi exe nằm trong Program Files; bật/tắt cần admin.
- Startup: task, service, app Store chỉ đọc. Cache trình duyệt: bỏ qua khi đang chạy, không đụng IndexedDB/Local Storage.

## Người dùng đang đăng nhập vs tài khoản chạy app (máy domain)
User thường (vd `DC1\bachtx`) nâng quyền bằng admin **khác** (vd `bavv`) — admin chỉ để lấy quyền; "của người dùng" = người đang đăng nhập:
- `crate::user`: `user::env(k)` thay `std::env::var` cho biến theo user (`%TEMP%`, `%LOCALAPPDATA%`, `%APPDATA%`, `%USERPROFILE%`, OneDrive…);
  `user::hkcu()` thay `CURRENT_USER` (= `HKEY_USERS\<SID>` khi khác user); `user::hkcu_reg_root()` cho reg.exe; `user::environment()` cho tiến
  trình con. **Không** đọc `CURRENT_USER`/biến môi trường theo user trực tiếp ở code mới.
- Mọi "xoá vào Thùng rác" qua `user::recycle` (khác user => ghi `$R`/`$I` vào `X:\$Recycle.Bin\<SID>`, Explorer khôi phục được).
- `sys::session`: so SID tiến trình với SID phiên (WTS); `sys::launch::spawn_as_shell_user`: chạy dưới user đang đăng nhập (cha = Explorer).
- WebView2 tự hạ quyền => thư mục dữ liệu WebView2 trong profile user đang đăng nhập (`tray.rs`), nếu không lỗi 0x8000FFFF (`logs\webview.log`).
- SQLite, crash.log, task khởi động (InteractiveToken cho SID của user) theo user đang đăng nhập; app riêng của tài khoản khác không hiện.

## Tác vụ dài & trạng thái giao diện
- Tác vụ dài chạy qua `jobs::run(app, "<kind>", …)`: thêm `kind` vào `jobs::KINDS`, `JobKind` (api.ts), `JOB_PAGE` (App.tsx); UI gọi qua `tracked()`
  và nối lại bằng `useJobResume` (đóng cửa sổ rồi mở lại vẫn thấy tiến độ/kết quả). Có % (`ProgressBar`; không biết tổng => chạy qua lại).
- Trang đã mở được giữ sống (chỉ ẩn): polling dùng `useVisibleInterval`; dữ liệu RAM dùng chung `useMemoryFeed` (chỉ lấy mẫu khi có trang
  tiêu thụ đang chọn); tác vụ đang chạy báo bằng `useReportBusy`; phím tắt toàn cửa sổ kiểm tra `usePageActive()`. Thông báo: `<Notice onClose>`.

## Giao diện
- Theme: `src/lib/theme.ts` (tối mặc định); `dark:` theo `data-theme` trên `<html>`; thanh tiêu đề qua `plugin:window|set_theme`.
- WebView chạy `--disable-gpu` => **không** `backdrop-blur`, glow/animation lặp, ảnh nền lớn; chỉ gradient tĩnh và SVG. Biểu tượng tự vẽ
  trong `components/Icons.tsx` (không thư viện icon, không emoji). Không import cả `@tauri-apps/api/window` (~18 KB) — gọi `invoke` thẳng.
- Class dùng chung `components/ui.ts` (`SURFACE`, `THEAD`, `INPUT`, `SELECT`, `PANEL`, `DIALOG`, `pill()`); trang dùng `PageShell` (tìm kiếm/
  Tải lại/Sắp xếp ở `actions`). Lưới 2 cột: card cao gần nhau cùng hàng, card nhiều nội dung `xl:col-span-2`.
- Sửa giao diện xong: `scripts/layout-check.js` ở 860x560, 1100x720, 1366x768, 1920x1080, cả hai theme, `issues` rỗng; rồi chụp ảnh xem bằng mắt.
  Không cần backend: `npm run dev` → `http://localhost:1420/?demo&page=<trang>`.
- Giữ React (bỏ React chỉ lợi ~1 MB, xem `docs/PERF.md`).

## Hiệu năng — ngân sách
- Rust lúc rảnh ≤ ~12 MB private, WebView2 tăng ≤ 5 MB, CPU trên Tổng quan không tăng, đóng cửa sổ = 0%. Vượt => báo người dùng để cắt/giảm.
  Mốc/cách đo: `docs/PERF.md`. Rust < 30MB tuyệt đối, installer < 10MB. Phần cứng đọc một lần khi mở trang; không PDH cho GPU, không nạp DXGI.

## Build & kiểm tra
- Dev: `npm run tauri dev` (admin: `scripts/admin-test.ps1 -Mode dev`). Release: `npm run tauri build` → `target/release/bundle/nsis/`
  (chỉ NSIS — `tauri.windows.conf.json`; NSIS có hook xoá task khởi động khi gỡ).
- Trước khi báo xong: `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npm run build` đều pass.
- Test cần admin: `#[ignore = "cần quyền Administrator…"]`, chạy bằng `scripts/admin-test.ps1` (UAC hỏi mỗi lần). Không tìm cách bỏ qua UAC.
- Release profile: `opt-level=3, lto="fat", codegen-units=1, panic="unwind", strip=true`. **Không đổi lại `abort`** (quyết định của người dùng):
  với abort, một panic ở bất kỳ luồng nào làm cả app tắt không dấu vết (lỗi thật ở v1.0.0). `crate-type = ["rlib"]` (không cần staticlib/cdylib).

## Phát hành
- `CHANGELOG.md`: mỗi phiên bản một mục `## [x.y.z] - YYYY-MM-DD`; CI (`release.yml`) lấy mục đó làm "Có gì mới", **thiếu mục => không phát hành**.
- Nâng phiên bản ở 4 chỗ: `package.json`, `package-lock.json` (2 dòng đầu), `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`. Chạy clippy/test/build
  → commit → `git tag -a vX.Y.Z -m "..."` → push `main` và tag (tag `v*` = CI build bộ cài + Release).
- Chỉ commit/push/tag khi người dùng yêu cầu. Người dùng thử bộ cài trên máy trước khi phát hành.
