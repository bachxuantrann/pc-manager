# Nhật ký thay đổi

Mỗi bản phát hành có một mục `## [x.y.z]`. CI lấy đúng mục của phiên bản được tag để làm phần
"Có gì mới" trên trang Release (thiếu mục => không phát hành).

## [1.0.3] - 2026-11-11

### Mới
- **Chuyển tab không còn mất trạng thái.** Đang quét/dọn ở một tab rồi sang tab khác, khi quay lại
  vẫn thấy đúng tiến độ, kết quả quét và các mục đã chọn. Mỗi tab nhớ vị trí cuộn của riêng nó.
- **Dấu "đang chạy" ở thanh bên:** tab nào còn tác vụ (quét/dọn rác, dọn WinSxS, Windows.old,
  quét ổ đĩa, gỡ app, dọn phần còn sót, Boost) có biểu tượng xoay cạnh tên.
- **Đóng cửa sổ (xuống khay hệ thống) rồi mở lại không mất tác vụ đang chạy.** Tác vụ dài vẫn chạy
  trong nền; mở lại cửa sổ sẽ thấy lại tiến độ, hoặc kết quả nếu tác vụ đã xong trong lúc cửa sổ
  đóng. Áp dụng cho: quét/dọn rác, phân tích/dọn WinSxS, xoá Windows.old, quét file lớn/file trùng.

### Thay đổi
- **Dọn WinSxS xong không tự phân tích lại nữa** (trước đây mất thêm ~1 phút). Bảng phân tích cũ được
  ẩn vì đã không còn đúng; bấm "Phân tích lại để xem kho sau khi dọn" nếu muốn xem. Thông báo kết
  quả dọn được giữ lại khi phân tích lại.
- Tab đang ẩn không còn cập nhật định kỳ (Tiến trình, Khởi động) — đỡ tốn CPU; quay lại tab là cập
  nhật ngay.

### Sửa lỗi
- Tab "Ứng dụng" hiện biểu tượng đang chạy dù không có tác vụ nào.
- Hộp thoại xác nhận của tab đang ẩn không còn bị đóng bởi phím Esc bấm ở tab khác.

## [1.0.2] - 2026-11-11

### Sửa lỗi
- **App không mở được giao diện khi nâng quyền bằng tài khoản admin khác** (máy công ty / domain:
  hộp thoại UAC hỏi tài khoản và mật khẩu admin). Lỗi "Microsoft Edge can't read and write to its
  data directory" đã hết.

### Thay đổi
- Khi nâng quyền bằng tài khoản admin khác, tài khoản đó chỉ dùng để lấy quyền; mọi thứ thuộc về
  người dùng đều theo **người đang đăng nhập Windows**:
  - cài đặt, lịch sử thao tác, log của app;
  - Dọn rác (Temp, cache trình duyệt, ảnh thu nhỏ…), danh sách thư mục cấm (cấm luôn toàn bộ
    profile của tài khoản admin);
  - Thùng rác (làm rỗng, chuyển file lớn/trùng và phần còn sót vào Thùng rác — khôi phục được từ
    Thùng rác của chính bạn);
  - Khởi động (Run, thư mục Startup), Ứng dụng (app cài riêng cho người dùng, lần dùng cuối, dọn
    phần còn sót, bản sao lưu `.reg`);
  - Khởi động cùng Windows: task chạy khi bạn đăng nhập.
- Gỡ app cài riêng cho người dùng bị chặn khi đang nâng quyền bằng tài khoản khác (trình gỡ sẽ gỡ
  nhầm profile) — mở PC Manager ở chế độ thường để gỡ.
- Chẩn đoán WebView2 ghi vào `%APPDATA%\com.pcmanager.app\logs\webview.log`.
