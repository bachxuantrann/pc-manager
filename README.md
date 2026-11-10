# PC Manager

App quản lý máy Windows 10/11 (RAM, dọn rác, tiến trình, khởi động, app không dùng) — Tauri 2 + React + Rust.
Quy tắc cho AI agent: [CLAUDE.md](CLAUDE.md). Lộ trình theo giai đoạn: [PROMPTS.md](PROMPTS.md).

## Yêu cầu
- Node 20+, Rust stable `x86_64-pc-windows-msvc`, VS 2022 Build Tools (C++ + Windows SDK), WebView2.

## Chạy
```powershell
npm install
npm run tauri dev      # chạy trong terminal "Run as Administrator"
npm run tauri build    # installer ở src-tauri/target/release/bundle/{msi,nsis}/
```

## Kiểm tra trước khi xong một giai đoạn
```powershell
cd src-tauri; cargo clippy --all-targets -- -D warnings; cargo test; cd ..; npm run build
```

## Manifest
- `src-tauri/app.manifest` (requireAdministrator + Common-Controls v6) nhúng vào bản **release**.
- `src-tauri/app.dev.manifest` (asInvoker + Common-Controls v6) cho bản **debug**, để `cargo test`
  và `tauri dev` không lỗi 740. Muốn có quyền admin khi dev thì mở terminal bằng quyền Administrator.
