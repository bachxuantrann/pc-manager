# PC Manager (Rust + Tauri) — hướng dẫn cho AI agent

App quản lý máy Windows 10/11 x64 giống Windows PC Manager: RAM, dọn rác, tiến trình, ứng dụng khởi động, app không dùng.
Mục tiêu: **nhẹ, nhanh, an toàn**. App tối ưu RAM thì bản thân không được ngốn RAM.

> **Kế hoạch đang triển khai: [`docs/PLAN.md`](docs/PLAN.md)** — 6 giai đoạn từ bản rà soát 2026-11-11, kèm quy tắc
> chung cho mọi giai đoạn và bảng trạng thái. Khi được giao "làm giai đoạn N", đọc file đó trước; xong thì cập nhật
> bảng trạng thái trong cùng commit.

## Stack (không tự ý đổi)
- Tauri 2 + React + TypeScript + Vite (frontend), Tailwind cho style
- Rust stable, toolchain `x86_64-pc-windows-msvc`
- `windows` crate (windows-rs) cho Win32/NT API (không dùng `sysinfo`); `windows-registry` cho registry; `webview2-com` chỉ để hạ mức RAM của WebView2
- `rusqlite` (feature `bundled`) cho lưu trữ, `serde`, `toml` (chỉ parse), `thiserror`, `anyhow` (chỉ ở tầng command), `rayon`, `jwalk`, `blake3`
- Chạy **1 tiến trình**, mặc định **quyền thường** (manifest `asInvoker`, mở app không hỏi UAC). Chức năng cần admin: UI hiện nút "Chạy với quyền Administrator" → `ShellExecuteExW` verb `runas` chạy lại chính exe (UAC hỏi), bản cũ thoát. Backend phải chạy được khi thiếu quyền (báo "cần quyền Administrator", không crash).
- **Không tách Windows Service** (quyết định của người dùng): app dùng cho cá nhân và vài người bạn; service chạy quyền hệ thống nhận lệnh từ người dùng thường làm tăng rủi ro bảo mật mà lợi ích không đáng. Không đề xuất lại trừ khi người dùng yêu cầu.

## Cấu trúc
```
pc-manager/
├── src/                      # React UI
│   ├── pages/  Dashboard, Diagnosis, SystemInfo, Memory, Cleaner, Processes, Startup, Apps, Disk, Settings
│   ├── components/  hooks/  lib/api.ts   # mọi invoke() gom ở api.ts, có type
│   └── dev/demo.ts           # dữ liệu giả cho `?demo` (chỉ bản dev)
├── src-tauri/
│   ├── build.rs              # nhúng app manifest (asInvoker + Common-Controls v6)
│   └── src/
│       ├── main.rs  lib.rs   # đăng ký commands, luồng nền (auto boost, sampler, auto clean)
│       ├── tray.rs  jobs.rs  settings.rs  state.rs  bg.rs (pool nền)  sync.rs  crashlog.rs  error.rs
│       ├── commands/         # mỏng: validate input -> gọi module -> map lỗi
│       ├── sys/              # TOÀN BỘ code unsafe/windows-rs nằm ở đây, bọc thành hàm safe
│       ├── user/             # ngữ cảnh "user đang đăng nhập" (env, HKCU, Thùng rác)
│       ├── memory/  cleaner/  process/  startup/  usage/  disk/
│       ├── diagnosis/  hwinfo/   # Smart Diagnosis, Cấu hình máy
│       └── store/            # SQLite
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

## Máy mới: chuẩn bị môi trường
- Cần: Node 22+, Rust stable `x86_64-pc-windows-msvc`, Visual Studio Build Tools (C++). Trên Windows:
  `winget install --id Rustlang.Rustup -e`, rồi
  `winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"` (UAC hỏi một lần).
  Bash của Claude Code chưa có `cargo` trong PATH sau khi cài: thêm `export PATH="$PATH:$USERPROFILE/.cargo/bin"`.
- `npm ci` → `npm run tauri dev`. Máy chưa chắc có `gh`, Python: dùng `node` (có `node:sqlite` để đọc DB của app khi cần chẩn đoán).
- Skill `rust-skills` nằm trong repo ở `.claude/skills/rust-skills/` (xem mục "Chuẩn code Rust"); `.claude/launch.json` có cấu hình `vite` (dev, cổng 1420) và `vite-prod` (bản build, cổng 4173) để xem giao diện.
- Xem giao diện không cần backend: `npm run dev` rồi mở `http://localhost:1420/?demo&page=<trang>` (dữ liệu giả trong `src/dev/demo.ts`, chỉ có ở bản dev).

## Chuẩn code Rust (rust-skills)
Code Rust bám theo `.claude/skills/rust-skills/` (đọc `SKILL.md`, mở file `rules/<prefix>-*.md` liên quan trước khi viết). Quy tắc của dự án (file này) luôn thắng. **Không dùng** các quy tắc sau (gây crash / rò rỉ bộ nhớ / hỏng build / làm app nặng):
- `panic = "abort"` (giữ `unwind`), `expect()`/`unwrap()` ngoài test.
- `target-cpu=native` (exe crash trên CPU cũ của máy khác), SIMD portable / `likely` / `collect_into` (chỉ có ở nightly), cú pháp riêng edition 2024 như `if let` chains (dự án là edition 2021, MSRV 1.85).
- Arena allocator (bộ nhớ phình mãi với app chạy nền lâu), `serde(deny_unknown_fields)` cho dữ liệu người dùng đã lưu (chỉ dùng cho dữ liệu nhúng lúc build như `cleaner.toml`).
- Thêm thư viện để tối ưu vi mô khi chưa đo (SmallVec, ArrayVec, ThinVec, CompactString, ahash), tokio/tokio-util, tracing, PGO, `inline(always)`; framework test nặng (mockall, proptest, criterion, loom, insta) — chỉ khi người dùng đồng ý.
- Quy tắc cho thư viện xuất bản crates.io (serde optional, cargo metadata, proc-macro, workspace, Miri).

## Người dùng đang đăng nhập vs tài khoản chạy app (máy domain)
Máy công ty: user thường (vd `DC1\bachtx`) nâng quyền bằng tài khoản admin **khác** (vd `bavv`) ở hộp UAC. Tài khoản admin chỉ để lấy quyền — mọi thứ "của người dùng" phải là của người đang đăng nhập:
- `crate::user` (`src-tauri/src/user/`): `user::env(k)` thay cho `std::env::var` với biến theo user (`%TEMP%`, `%LOCALAPPDATA%`, `%APPDATA%`, `%USERPROFILE%`, OneDrive…); `user::hkcu()` thay cho `CURRENT_USER` (= `HKEY_USERS\<SID>` khi khác user); `user::hkcu_reg_root()` cho reg.exe; `user::environment()` cho tiến trình con. **Không** đọc `CURRENT_USER`/biến môi trường theo user trực tiếp ở code mới.
- `user::recycle`: Thùng rác của user đang đăng nhập (khác user => ghi trực tiếp cặp `$R`/`$I` vào `X:\$Recycle.Bin\<SID>`, đã kiểm chứng Explorer khôi phục được). Mọi "xoá vào Thùng rác" đi qua đây.
- `sys::session`: so SID tiến trình với SID phiên (WTS); `sys::launch::spawn_as_shell_user`: chạy chương trình dưới user đang đăng nhập (cha = Explorer) — dùng cho trình gỡ app HKCU khi đang nâng quyền.
- WebView2 tự hạ quyền về user đang đăng nhập => thư mục dữ liệu WebView2 phải trong profile của user đó (`tray.rs`), nếu không: lỗi 0x8000FFFF "can't read and write to its data directory". Chẩn đoán ghi ở `%APPDATA%\com.pcmanager.app\logs\webview.log` (của user đang đăng nhập).
- Dữ liệu app (SQLite), crash.log, task khởi động cùng Windows (XML, InteractiveToken cho SID của user) đều theo user đang đăng nhập.
- Gỡ app: HKLM chạy trình gỡ trực tiếp, cần admin mà đang quyền thường (mã 740) => mở lại qua UAC (`run_elevated_pid`); HKCU khi đang nâng quyền bằng tài khoản khác => `spawn_as_shell_user`. App riêng của tài khoản khác không hiện.

## Tác vụ dài & giữ trạng thái giao diện
- Tác vụ dài mới (quét/dọn/DISM/…) chạy qua `jobs::run(app, "<kind>", …)` (`src-tauri/src/jobs.rs`): ghi tiến độ/kết quả, phát sự kiện `jobs://update`; thêm `kind` vào `jobs::KINDS`, `JobKind` (api.ts), `JOB_PAGE` (App.tsx); phía UI gọi qua `tracked()` trong `api.ts` và nối lại bằng `useJobResume` — đóng cửa sổ (xuống tray) rồi mở lại vẫn thấy tiến độ/kết quả.
- Tác vụ lâu phải có tiến độ % (`ProgressBar`; không biết tổng => dạng chạy qua lại, ước lượng thì ghi rõ "ước lượng").
- App giữ sống các trang đã mở (chỉ ẩn): polling dùng `useWindowVisible`/`useVisibleInterval` (tự dừng khi trang ẩn hoặc cửa sổ ẩn); tác vụ đang chạy báo bằng `useReportBusy` (dấu xoay ở Sidebar); phím tắt toàn cửa sổ phải kiểm tra `usePageActive()`.
- Thông báo kết quả/lỗi của thao tác: `<Notice onClose>` để người dùng tắt được.

## Giao diện (theme)
- Tối mặc định, đổi ở Cài đặt → Giao diện (Tối/Sáng/Theo Windows; lưu `localStorage` `pcm.theme`, `src/lib/theme.ts`). Biến thể `dark:` của Tailwind do `data-theme` trên `<html>` điều khiển (không theo Windows), thanh tiêu đề đổi theo qua `plugin:window|set_theme`.
- WebView chạy `--disable-gpu` (tiết kiệm RAM) => mọi hiệu ứng do CPU vẽ: **không** dùng `backdrop-blur`, glow/animation lặp, ảnh nền lớn; chỉ gradient tĩnh và SVG. Biểu tượng tự vẽ trong `components/Icons.tsx` (không thêm thư viện icon). Không import cả module `@tauri-apps/api/window` chỉ để gọi một lệnh (nặng ~18 KB) — gọi `invoke` thẳng.
- Class dùng chung ở `src/components/ui.ts` (`SURFACE` bảng/khung, `THEAD` tiêu đề bảng dính, `INPUT`, `SELECT`, `PANEL`, `DIALOG`, `pill()` nút lọc) — trang mới dùng các class này, không tự viết màu/bo góc riêng.
- Sửa giao diện xong phải chạy `scripts/layout-check.js` (hướng dẫn trong file) ở 860x560 (nhỏ nhất), 1100x720, 1366x768, 1920x1080, cả hai theme: `issues` phải rỗng; rồi chụp ảnh xem bằng mắt.
- Giữ React (đã đo: toàn bộ JS heap ~2.3 MB so với WebView2 ~132 MB riêng — bỏ React lợi ~1 MB, không đáng viết lại).
- Thanh công cụ (tìm kiếm, Tải lại, Sắp xếp) đặt ở `PageShell actions`, không đặt cuối hàng bộ lọc (hàng bộ lọc đổi độ dài sẽ đẩy chúng xuống dòng). Lưới card 2 cột: ghép card cao gần nhau cùng hàng, card nhiều nội dung `xl:col-span-2`.

## Hiệu năng — mốc đo & ngân sách
- Mốc (máy 12 luồng, đo trên app cài thật, trang Tổng quan): v1.0.4 — Rust ~10.8 MB private, WebView2 6 tiến trình ~132 MB private (cửa sổ đã mở lâu, nhiều trang đã nạp), CPU ~0.5%. v1.1.0 (theme mới + Cấu hình máy + Chẩn đoán) — Rust ~10.2 MB private, WebView2 ~106 MB private (cửa sổ vừa mở), CPU ~0.1%. Cửa sổ đóng (tray): Rust ~10.7 MB, WebView2 bị huỷ (0). Smart Diagnosis quét nhanh ~0.2 giây.
- Ngân sách cho tính năng mới: Rust lúc rảnh ≤ ~12 MB private, WebView2 tăng ≤ 5 MB, CPU trên Tổng quan không tăng, đóng cửa sổ = 0%. Đo bằng `Get-Process pc-manager` + `msedgewebview2` có `com.pcmanager.app` trong command line (Win32_Process). Phần nào vượt => báo người dùng để cắt/giảm.
- Không dùng WMI, không polling khi không hiển thị, đọc thông tin phần cứng một lần khi mở trang; tránh PDH cho GPU (tốn CPU), tránh nạp DXGI vào tiến trình Rust (đọc thông tin GPU từ registry).

## Phát hành
- `CHANGELOG.md`: mỗi phiên bản một mục `## [x.y.z] - YYYY-MM-DD`; CI (`release.yml`) lấy mục đó làm "Có gì mới" trên GitHub Release, **thiếu mục => không phát hành**.
- Nâng phiên bản ở 4 chỗ: `package.json`, `package-lock.json` (2 dòng đầu), `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`. Chạy clippy/test/build → commit → `git tag -a vX.Y.Z -m "..."` → push `main` và tag. Push tag `v*` = CI build bộ cài + tạo Release.
- Chỉ commit/push/tag khi người dùng yêu cầu. Người dùng thử bộ cài build trên máy trước khi phát hành.
