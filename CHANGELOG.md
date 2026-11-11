# Nhật ký thay đổi

Mỗi bản phát hành có một mục `## [x.y.z]`. CI lấy đúng mục của phiên bản được tag để làm phần
"Có gì mới" trên trang Release (thiếu mục => không phát hành).

## [1.1.2] - 2026-11-11

### Sửa lỗi
- **Chẩn đoán không còn đề xuất "Kho WinSxS nên được dọn" khi dọn chẳng được gì.** Trước đây quét sâu
  cứ làm theo câu "nên dọn" của DISM — câu này hay báo "Có" mãi, kể cả khi bấm Dọn WinSxS chỉ được
  0 byte. Giờ PC Manager nhớ kết quả các lần phân tích/dọn và tự đánh giá:
  - Dọn mà không lấy lại được gì => các gói còn lại là phần Windows đang giữ; không đề xuất nữa cho
    tới khi có bản cập nhật Windows mới (hoặc sau 30 ngày).
  - Vừa dọn từ lần khởi động gần nhất => phần còn lại chỉ dọn được sau khi khởi động lại máy.
  - **Windows đang chờ khởi động lại để hoàn tất cập nhật** => không đề xuất dọn WinSxS (lúc này DISM
    gần như không gỡ được gì), chỉ đề xuất Restart. Lần dọn lỡ bấm trong lúc này không bị tính là
    "dọn vô ích", sau khi Restart vẫn được đề xuất lại nếu còn cần.
- Dọn rác → Dọn sâu hệ thống → **Phân tích** WinSxS hiện kết luận rõ ràng ("Nên dọn" / "Chưa cần dọn" /
  "Khởi động lại máy trước") kèm lý do; thông báo sau khi dọn được 0 byte nói đúng nguyên nhân.

### Thay đổi
- Trang Chẩn đoán khi PC Manager chạy quyền thường hiện thông báo "Quét sâu cần quyền Administrator"
  kèm nút **Chạy với quyền Administrator** (giống các trang khác), mở lại thẳng trang Chẩn đoán.

## [1.1.1] - 2026-11-11

### Thay đổi
- **Mọi trang đồng bộ giao diện mới**: Bộ nhớ, Dọn rác, Tiến trình, Khởi động, Ứng dụng, Ổ đĩa, Cài
  đặt cùng kiểu với Tổng quan / Cấu hình máy / Chẩn đoán — bảng bo góc viền mảnh, tiêu đề bảng dính
  nền đặc khi cuộn, ô tìm kiếm và ô chọn cùng một kiểu, nút lọc dạng viên, hộp thoại xác nhận nền và
  viền mới. Thanh tiến độ của Dọn rác dùng chung kiểu với các trang khác.
- Đã kiểm tra bố cục tự động ở 4 kích thước cửa sổ (từ nhỏ nhất 860×560 tới 1920×1080) × mọi trang ×
  cả theme tối và sáng: không cuộn ngang, không tràn, không nút nào đè lên nhau hay bị cắt chữ.

## [1.1.0] - 2026-11-11

### Mới
- **Giao diện tối mới** (mặc định): nền tối có ánh xanh, card bo góc, thanh bên có biểu tượng, đồng
  hồ RAM dạng vòng cung ở Tổng quan. Đổi Tối / Sáng / Theo Windows ở Cài đặt → Giao diện. Không
  dùng hiệu ứng kính mờ hay animation nên không làm app nặng thêm.
- **Trang "Cấu hình máy"**: Windows (bản, build, ngày cài, thời gian chạy liên tục), CPU (nhân/luồng,
  xung), card đồ hoạ (VRAM, driver), mainboard & BIOS, RAM (số khe đã cắm/tổng, từng thanh: dung
  lượng, DDR4/DDR5, tốc độ thực tế và tốc độ thanh hỗ trợ, hãng, mã), ổ đĩa (model, dung lượng,
  NVMe/SATA/USB, SSD/HDD). Nút **Sao chép cấu hình** để dán gửi người khác. Không cần quyền admin,
  không thu thập số serial hay tên máy. "Bật máy gần nhất" lấy đúng lần bật nguồn (kể cả khi Fast
  Startup bật), không phải thời gian từ lần Restart.
- **Chẩn đoán thông minh** (trang "Chẩn đoán", hoặc nút ở Tổng quan): một lần quét kiểm tra RAM, ổ
  đĩa, rác, Thùng rác, app khởi động, app lâu không dùng, Windows (chờ khởi động lại, lâu chưa
  Restart, Windows.old) và phần cứng (RAM chưa bật XMP/EXPO, ổ HDD, khe RAM trống). Mỗi nhóm có trạng
  thái Tốt / Gợi ý / Cần chú ý / Nên xử lý; mỗi vấn đề có lý do kèm số liệu và nút xử lý (mở đúng
  trang để xem trước & xác nhận; Boost chạy sau hộp xác nhận). Quét lại sẽ hiện **đã cải thiện gì**
  (số liệu trước → sau). Quét nhanh chỉ đọc, dưới 1 giây; "Quét sâu" thêm phân tích WinSxS (~1 phút,
  cần quyền Administrator).

### Thay đổi
- Trang Cài đặt sắp lại: card cùng hàng cao bằng nhau.

## [1.0.4] - 2026-11-11

### Mới
- **Hiện % cho các tác vụ lâu** bằng một thanh tiến độ thống nhất:
  - Phân tích WinSxS (trước chỉ có biểu tượng xoay).
  - Xoá Windows.old — % ước lượng theo dung lượng đã lấy lại (Disk Cleanup không báo tiến độ).
  - Quét file lớn / file trùng — khi quét cả ổ, % của bước duyệt ước lượng theo dung lượng đã dùng
    của ổ; quét file trùng hiện rõ "Bước 1/3, 2/3, 3/3".
  - Tải danh sách Ứng dụng: "x / y ứng dụng" kèm %.

### Thay đổi
- **Gỡ được app cài riêng cho người dùng khi đang nâng quyền bằng tài khoản admin khác** (trước đây
  bị chặn). Trình gỡ được chạy dưới chính tài khoản đang đăng nhập Windows (như khi bạn tự gỡ trong
  Settings), nên gỡ đúng profile của bạn; sau khi gỡ vẫn mở "Dọn phần còn sót" như bình thường.
- **Gỡ app cài cho cả máy khi PC Manager đang chạy quyền thường:** trình gỡ cần quyền admin giờ được
  mở qua hộp UAC (như khi gỡ trong Settings) thay vì báo lỗi "requires elevation" (mã 740). Bấm No ở
  UAC thì báo rõ là chưa gỡ gì.
- Dọn WinSxS xong chỉ báo kết quả và ẩn bảng phân tích cũ; bỏ nút "Phân tích lại" (bấm "Phân tích"
  khi muốn xem).
- Khi DISM báo thành công nhưng không lấy lại được gì, thông báo nói rõ là "không còn gì để dọn
  thêm" thay vì "Xong sau 1 giây — không thấy dung lượng trống tăng thêm".

### Sửa lỗi
- Thông báo kết quả / lỗi của thao tác (Ứng dụng, Dọn rác, Dọn sâu hệ thống, Ổ đĩa, Tiến trình,
  Khởi động) giờ có nút × để tắt, không còn đọng lại mãi.
- Gỡ một app mà trình gỡ đã không còn trên máy (thường là app đã gỡ nhưng còn sót mục trong danh
  sách): thay vì "Không tìm thấy: không tìm thấy …", thông báo giải thích và hướng dẫn bấm "Dọn sót"
  để xoá phần còn lại.
- Trang Khởi động: số trên tab "Tác vụ khi đăng nhập" / "Dịch vụ tự khởi động" đếm cả mục của
  Microsoft đang bị ẩn nên không khớp danh sách. Giờ số trên tab = số dòng đang hiện, và ô "Ẩn mục
  của Microsoft" ghi rõ đang ẩn bao nhiêu mục. Nút "Tải lại" chuyển lên cạnh ô tìm kiếm nên không
  còn bị đẩy xuống dòng khi bật/tắt ô này.
- Trang Ứng dụng: ô "Sắp xếp" chuyển lên cạnh nút "Tải lại", không còn bị đẩy xuống dòng khi chọn
  bộ lọc "Không dùng > …" (lúc hiện thêm ô "Gồm cả app chưa rõ lần dùng").

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
