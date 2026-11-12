# Nhật ký thay đổi

Mỗi bản phát hành có một mục `## [x.y.z]`. CI lấy đúng mục của phiên bản được tag để làm phần
"Có gì mới" trên trang Release (thiếu mục => không phát hành).

## [1.2.1] - 2026-11-12

### Thay đổi
- Bản phát hành để kiểm tra chức năng tự cập nhật từ 1.2.0 (không đổi chức năng). Từ 1.2.0 trở đi, PC Manager tự báo khi
  có bản mới; bấm "Cập nhật" trong Cài đặt để cài.

## [1.2.0] - 2026-11-12

### Mới
- **Tự cập nhật trong app.** Có bản mới trên GitHub Release => dải thông báo ở đầu cửa sổ và thẻ "Cập nhật" trong Cài đặt
  (kèm "Có gì mới"). Bấm "Cập nhật" => tải bộ cài, **kiểm chữ ký** (khoá riêng chỉ nằm trong secret của CI), Windows hỏi UAC,
  cài xong app tự mở lại. Không bao giờ tự cài; tự kiểm tra tối đa 1 lần/ngày khi mở cửa sổ (tắt được trong Cài đặt), chỉ gửi
  một yêu cầu tới github.com. Cập nhật giữ nguyên tuỳ chọn "Khởi động cùng Windows" (trước đây cài đè sẽ xoá task này).

### Bảo mật
- **Gỡ app cài riêng cho người dùng (HKCU) không còn chạy bằng quyền Administrator.** Khi PC Manager đang
  chạy quyền admin, trình gỡ của các app này giờ luôn chạy dưới tài khoản đang đăng nhập với quyền thường
  (như Windows Settings). Lệnh gỡ trong HKCU sửa được bởi mọi chương trình quyền thường, nên chạy nó bằng
  quyền admin là đường leo thang đặc quyền. Trình gỡ nào tự đòi quyền admin => PC Manager báo lỗi và hướng
  dẫn gỡ từ quyền thường (Windows sẽ hỏi UAC), không bao giờ tự dùng quyền admin của mình.

### Sửa lỗi
- Dọn rác tự động không còn thay kết quả quét bạn đang xem ở trang Dọn rác (trước đây bấm Dọn sau đó sẽ báo
  "kết quả quét đã cũ").
- **Smart Boost (bấm tay và tự động) dùng Boost nhẹ** thay vì Boost sâu. Khi RAM thấp, xoá toàn bộ cache
  standby không làm tăng RAM khả dụng (Windows vốn nhường cache đó ngay khi app cần) mà chỉ làm máy đọc đĩa
  nhiều hơn; lợi ích thật đến từ việc ghi dữ liệu chờ (modified) xuống đĩa. Sửa lại mô tả thẻ Smart Boost.
  Đo trên máy thật (3 lần): Boost nhẹ tăng RAM khả dụng +4…+215 MB, chỉ xoá 57–585 MB cache; Boost sâu chỉ tăng
  +54…+93 MB nhưng xoá 3,8–4,4 GB cache.
- Chẩn đoán: khi RAM còn dưới 10%, hành động đề xuất là "Mở Tiến trình" để đóng bớt app nặng (Boost chỉ giúp
  chút ít); mô tả khi RAM 10–20% nói đúng việc Boost làm.
- Dọn rác tự động kiểm tra lịch lần đầu sau ~2 phút kể từ khi mở PC Manager (trước đây 30 phút — phiên dùng
  máy ngắn thì không bao giờ dọn).

### Thay đổi
- **Tác vụ dài có tiến độ và nối lại được** khi đóng cửa sổ (xuống tray) rồi mở lại: chuyển file vào Thùng rác
  ở trang Ổ đĩa (có nút Dừng), dọn phần còn sót sau khi gỡ app, tải danh sách ứng dụng.
- Thanh tiến độ Dọn rác chạy theo số file đã dọn, không đứng yên trong một mục có hàng chục nghìn file.
- Quét file lớn / file trùng có ngưỡng tối thiểu ở backend (1 MB / 64 KB) để không giữ danh sách quá lớn trong RAM.
- **Trang Khởi động hiện cả app Microsoft Store** tự chạy khi đăng nhập (Teams, Terminal, Xbox…, chỉ xem — như tab
  Startup của Task Manager); dịch vụ hiện đúng tên hiển thị thay vì tên key (`@dll,-123`); shortcut `.Lnk` hoa thường
  lẫn lộn được cắt đuôi đúng. Mục app Store có nút "Mở Task Manager" mở thẳng tab Startup để bật/tắt
  (không dùng Settings → Apps → Startup vì trang đó crash trên một số bản Windows 11).
- **Trang Ứng dụng liệt kê cả app Microsoft Store** (đọc từ registry + manifest, không nạp WinRT nên không tốn thêm
  RAM lúc rảnh); nút "Gỡ trong Settings" mở Windows Settings → Apps.
- Dọn rác: thêm cache trình duyệt Brave, Vivaldi, Cốc Cốc, Opera / Opera GX; Chrome/Edge thêm `Service Worker\CacheStorage`,
  `ShaderCache`, `GrShaderCache`; Firefox thêm `startupCache`. Không đụng dữ liệu trang web (IndexedDB, Local Storage).
- "Bản cập nhật Windows đã tải": mô tả đúng hơn (có cả bản đang chờ cài, Windows sẽ tải lại) và không còn được chọn sẵn.
- Chẩn đoán: gợi ý "Bật XMP/EXPO" chỉ hiện với RAM rời của máy bàn (DIMM), không còn báo sai trên laptop.
- Bật/tắt mục khởi động của máy khi thiếu quyền: thông báo có nút "Chạy với quyền Administrator", không phụ thuộc ngôn
  ngữ của Windows.
- Thanh trượt ngưỡng Smart Boost chỉ lưu (và ghi nhật ký) khi giá trị thật sự đổi.
- Giao diện: trang Tiến trình và Khởi động dùng khung trang chung (tiêu đề to lên ở màn lớn như các trang khác); emoji/ký tự
  (🔒 🛡 ✓ ↻ ▸▾ ⚠) thay bằng biểu tượng SVG tự vẽ, hiển thị giống nhau trên mọi máy.

### Hiệu năng
- Số liệu RAM chỉ được hỏi khi đang mở Tổng quan hoặc Bộ nhớ (trước đây 2 giây/lần ở mọi trang); trang ẩn không còn vẽ lại
  biểu đồ. Đo ở bản dev: trang Cài đặt 0 lần `memory_status` trong 6,5 giây (trước: ~3 lần).
- Bảng Tiến trình chỉ vẽ lại các dòng có số liệu thay đổi (trước: dựng lại cả 300+ dòng mỗi 2 giây).
- Lấy mẫu "lần dùng cuối" ghi SQLite ít hơn hẳn: exe mới ghi ngay, exe đang chạy làm mới tối đa 15 phút/lần
  (trước: 150–300 lệnh ghi mỗi phút).
- Quét rác kiểm tra danh sách cấm theo thư mục (một lần cho cả thư mục) và hạ chữ thường mẫu lọc một lần lúc nạp rule;
  bước kiểm tra lại trước khi xoá vẫn từng file.
- Mở trang Cài đặt không còn chạy `schtasks.exe` (đọc trạng thái khởi động cùng Windows qua COM).
- Cơ sở dữ liệu không lớn dần mãi: giữ 5.000 dòng nhật ký mới nhất, bỏ mẫu "lần dùng cuối" của exe không thấy chạy hơn 1 năm.

### Nội bộ
- Build release chỉ tạo `rlib` cho thư viện (bỏ `staticlib`/`cdylib` chỉ cần cho mobile): build lại crate bản release
  ~194 giây → ~160 giây (nhanh hơn ~17%, đo 2 lần, dependency đã build sẵn).
- `scripts/admin-test.ps1` chạy được khi nâng quyền bằng tài khoản admin khác (truyền `CARGO_HOME`/`RUSTUP_HOME` của
  người đang đăng nhập); các test phụ thuộc môi trường dùng `%TEMP%`/biến của user đang đăng nhập.
- Gộp mã lặp (`commands::blocking`, `now_ms`, `expand_env`, `run_elevated`); skill `rust-skills` rút gọn cho dự án
  (265 → 129 quy tắc); `CLAUDE.md` gọn lại (chi tiết API ở `docs/FEATURES.md`, mốc đo ở `docs/PERF.md`); thêm
  `AGENTS.md`; `PROMPTS.md` chuyển vào `docs/history/`; bỏ README tiếng Nhật và tiếng Trung.

## [1.1.3] - 2026-11-12

### Sửa lỗi
- **Dọn rác tự động không còn bị bỏ cả kỳ** khi tới hạn đúng lúc bạn đang tự quét/dọn ở trang Dọn rác:
  trước đây phải chờ thêm 1/7/30 ngày, giờ PC Manager thử lại sau 30 phút.
- Xoá Windows.old xong không còn đo lại cả cây thư mục (hàng chục GB) chỉ để kiểm tra đã xoá chưa —
  kết quả hiện ngay.
- Thông báo lỗi khi mở trình gỡ cài đặt gặp sự cố nói đúng thao tác ("mở trình gỡ cài đặt").

### Thay đổi
- Liệt kê tiến trình nhẹ hơn: không còn chép lại 0,5–1 MB dữ liệu mỗi lần làm mới (2 giây/lần ở trang
  Tiến trình, 60 giây/lần khi chạy nền).
- Quét rác nhanh hơn với các quy tắc "mọi file" (không tạo dữ liệu tạm cho từng file).
- "Mở vị trí file" ở trang Tiến trình luôn mở đúng `explorer.exe` của Windows (đường dẫn tuyệt đối),
  kể cả khi PC Manager chạy với quyền Administrator.
- Dọn mã nội bộ theo bản rà soát toàn dự án 2026-11-11 (không đổi hành vi). Kế hoạch cải tiến tiếp
  theo: `docs/PLAN.md`.

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
