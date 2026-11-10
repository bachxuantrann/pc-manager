# PC Manager

App quản lý máy Windows 10/11 (RAM, dọn rác, tiến trình, khởi động, app không dùng) — Tauri 2 + React + Rust.
Quy tắc cho AI agent: [CLAUDE.md](CLAUDE.md). Lộ trình theo giai đoạn: [PROMPTS.md](PROMPTS.md).

## Yêu cầu
- Node 20+, Rust stable `x86_64-pc-windows-msvc`, VS 2022 Build Tools (C++ + Windows SDK), WebView2.

## Chạy
```powershell
npm install
npm run tauri dev      # quyền thường; cần thử chức năng admin: bấm "Chạy với quyền Administrator" trong app
npm run tauri build    # installer NSIS ở src-tauri/target/release/bundle/nsis/
```

## Kiểm tra trước khi xong một giai đoạn
```powershell
cd src-tauri; cargo clippy --all-targets -- -D warnings; cargo test; cd ..; npm run build
```

## Test với quyền Administrator
Một số test (Boost thật, đọc đường dẫn service…) cần quyền admin. Script tự mở lại chính nó
bằng "Run as administrator" — mỗi lần chạy Windows hiện UAC để bạn xác nhận, không lưu quyền:
```powershell
.\scripts\admin-test.ps1                 # toàn bộ test, kể cả test #[ignore] cần admin
.\scripts\admin-test.ps1 -Filter memory  # chỉ test có "memory" trong tên
.\scripts\admin-test.ps1 -Mode dev       # mở `npm run tauri dev` trong cửa sổ admin
.\scripts\admin-test.ps1 -Mode ram       # đo RAM bản release khi rảnh (cần `npm run tauri build` trước)
```
Log nằm ở `src-tauri/target/admin-test.log`.

## Manifest
- `src-tauri/app.manifest` (asInvoker + Common-Controls v6) cho cả bản debug và release: mở app
  **không hỏi UAC**, chạy với quyền thường.
- Chức năng cần quyền admin (Boost, dọn file hệ thống, Prefetch, mục khởi động của máy…) hiện nút
  **"Chạy với quyền Administrator"**: app tự mở lại chính nó qua UAC rồi thoát bản cũ.
- Khởi động cùng Windows (task Task Scheduler "quyền cao nhất") chạy luôn với quyền admin, không hỏi UAC.
