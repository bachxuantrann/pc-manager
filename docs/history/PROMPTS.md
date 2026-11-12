# Prompt theo từng giai đoạn cho Claude Code

> **Tài liệu lịch sử** (các prompt dùng để dựng v1.0). Một số chi tiết đã lỗi thời — vd. manifest nay là
> `asInvoker`, không phải `requireAdministrator`; app có 10 trang. Khi làm việc luôn theo `CLAUDE.md`.

Chạy lần lượt, mỗi giai đoạn xong thì build + test thử trên máy rồi commit. Đừng gộp nhiều giai đoạn vào một prompt.

## 0. Khởi tạo
```
Đọc CLAUDE.md. Khởi tạo project Tauri 2 + React + TS + Vite + Tailwind theo đúng cấu trúc trong đó.
Thêm build.rs nhúng app.manifest (requireAdministrator + Common-Controls v6), release profile, tray icon cơ bản,
layout sidebar với 7 trang rỗng. Tạo module sys/ với RAII wrapper cho HANDLE. Chạy được `npm run tauri dev`.
```

## 1. Dashboard + Memory
```
Làm module memory + trang Dashboard và Memory: tổng RAM, đang dùng, available, standby, modified, commit,
biểu đồ 60 giây gần nhất (chỉ refresh khi cửa sổ đang hiển thị). Nút Boost (purge standby + flush modified)
và Smart Boost theo ngưỡng cấu hình. Hiển thị RAM giải phóng được sau khi Boost. Viết test cho phần tính toán.
```

## 2. Process manager
```
Làm module process + trang Processes: bảng tên, PID, CPU%, RAM private, đường dẫn, publisher (WinVerifyTrust, cache kết quả theo path).
Sort, search, gộp theo tên app. Kill / Suspend / Resume / đổi priority / mở vị trí file, có xác nhận.
Chặn kill process hệ thống theo danh sách trong CLAUDE.md.
```

## 3. Startup manager
```
Làm module startup + trang Startup: liệt kê từ Run/RunOnce (HKCU, HKLM, WOW6432Node), thư mục Startup, scheduled task logon.
Bật/tắt qua StartupApproved giống Task Manager, đọc trạng thái hiện tại đúng như Task Manager hiển thị.
Hiển thị publisher, đường dẫn, nguồn. Ghi log mọi thay đổi.
```

## 4. Cleaner
```
Làm rule engine trong cleaner/ đọc rules/cleaner.toml và trang Cleaner: Scan (có progress + cancel) → Preview theo nhóm
(kích thước, số file, mức rủi ro, checkbox) → Clean → Report. Tuân thủ tuyệt đối mục AN TOÀN trong CLAUDE.md,
viết test cho danh sách cấm cứng và việc không follow junction. Thêm chế độ dry-run.
```

## 5. Apps không dùng
```
Làm module usage + trang Apps: danh sách app đã cài với size, ngày cài, lần dùng cuối (UserAssist + Prefetch + sampling SQLite).
Lọc "không dùng > N ngày", sort theo dung lượng. Nút gỡ cài đặt chạy UninstallString sau khi xác nhận.
```

## 6. Disk + hoàn thiện
```
Trang Disk: dung lượng các ổ, quét file lớn, tìm file trùng (size → blake3 4KB đầu → blake3 full). Xoá chỉ vào Recycle Bin.
Settings: ngưỡng Smart Boost, lịch dọn tự động, khởi động cùng Windows (Task Scheduler), whitelist.
Rà lại toàn bộ: clippy sạch, đo RAM khi rảnh, tối ưu những chỗ vượt mục tiêu.
```

## Khi gặp lỗi
- Lỗi build Windows API: "Kiểm tra feature flags của crate `windows` trong Cargo.toml cho API X".
- App ngốn RAM: "Profile RAM của app khi idle, tìm chỗ polling/clone thừa, giữ dưới mục tiêu trong CLAUDE.md".
