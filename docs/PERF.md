# Hiệu năng — mốc đo

Ngân sách và cách đo nằm trong `CLAUDE.md` (mục "Hiệu năng"). File này giữ các số đo chi tiết.

## Cách đo

- Rust: `Get-Process pc-manager` (cột private bytes).
- WebView2: các tiến trình `msedgewebview2` có `com.pcmanager.app` trong command line (đọc qua `Win32_Process`).
- Đo trên app **cài thật** (bản release), trang Tổng quan, máy 12 luồng. `scripts/admin-test.ps1 -Mode ram` đo bản release.

## Mốc

| Phiên bản | Rust (private) | WebView2 (private) | CPU (Tổng quan) | Ghi chú |
|---|---|---|---|---|
| v1.0.4 | ~10.8 MB | 6 tiến trình ~132 MB | ~0.5% | cửa sổ đã mở lâu, nhiều trang đã nạp |
| v1.1.0 | ~10.2 MB | ~106 MB | ~0.1% | theme mới + Cấu hình máy + Chẩn đoán; cửa sổ vừa mở |
| v1.1.0 (tray) | ~10.7 MB | 0 (WebView2 bị huỷ) | 0% | cửa sổ đóng |

- Smart Diagnosis quét nhanh ~0.2 giây.
- JS heap toàn bộ ~2.3 MB so với WebView2 ~132 MB riêng — bỏ React chỉ lợi ~1 MB (vì vậy giữ React).

## Build release (B1, 2026-11-12)

Đo thời gian build lại crate (sau `touch src/lib.rs`, dependency đã build sẵn), `lto = "fat"`:

| crate-type | Lần 1 | Lần 2 |
|---|---|---|
| `staticlib, cdylib, rlib` (cũ) | 194 s | 194 s |
| `rlib` (mới) | 159 s | 162 s |

Nhanh hơn ~17%. File exe release ~13 MB (trước khi đóng gói NSIS).
