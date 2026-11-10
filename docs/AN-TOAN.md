# An toàn và quyền Administrator

[English](SAFETY.md) · [Tiếng Việt](AN-TOAN.md)

PC Manager đụng tới file, tiến trình, registry và dịch vụ của Windows. Tài liệu này mô tả
**app được làm gì, không được làm gì**, và các lớp bảo vệ trong code.

## 1. Mô hình quyền

| Cách chạy | Quyền | Hỏi UAC? |
|---|---|---|
| Mở app bình thường | **Quyền thường** (manifest `asInvoker`) | Không |
| Bấm "Chạy với quyền Administrator" | Admin — app mở lại chính nó qua `ShellExecuteExW("runas")`, bản cũ thoát | Có, mỗi lần |
| Khởi động cùng Windows (nếu bạn bật) | Admin — task Task Scheduler "chạy với quyền cao nhất" khi đăng nhập | Chỉ một lần, lúc bật |

- Khi thiếu quyền, chức năng cần admin báo "cần quyền Administrator" thay vì lỗi.
- **Khởi động cùng Windows chỉ bật được khi app nằm trong `Program Files`** (cài bằng bộ cài).
  Task chạy file exe với quyền admin mà không hỏi lại; nếu exe nằm ở nơi người dùng thường ghi
  được (Desktop, Downloads…), phần mềm độc hại có thể thay exe để mượn quyền admin. Đường dẫn
  được giải symlink/junction trước khi kiểm tra.
- Bật/tắt tuỳ chọn này cần quyền admin. Gỡ app bằng bộ cài sẽ xoá luôn task.
- App **không** cài Windows Service và không chạy gì với quyền SYSTEM.

## 2. Dọn rác (Cleaner)

**Chỉ xoá file khớp quy tắc cố định** trong [`rules/cleaner.toml`](../rules/cleaner.toml)
(được nhúng vào app lúc build). Không có lệnh nào nhận đường dẫn tuỳ ý từ giao diện để xoá.

| Quy tắc | Vị trí | Rủi ro | Chỉ xoá file cũ hơn |
|---|---|---|---|
| File tạm của bạn | `%LOCALAPPDATA%\Temp` | thấp | 24 giờ |
| File tạm của Windows | `%SystemRoot%\Temp` | thấp | 24 giờ |
| Cache ảnh thu nhỏ | `%LOCALAPPDATA%\Microsoft\Windows\Explorer` (`thumbcache_*`) | thấp | — |
| Cache shader DirectX | `%LOCALAPPDATA%\D3DSCache` | thấp | 24 giờ |
| Crash dump của app | `%LOCALAPPDATA%\CrashDumps` | thấp | 24 giờ |
| Minidump màn hình xanh | `%SystemRoot%\Minidump` | trung bình | 7 ngày |
| `MEMORY.DMP` | `%SystemRoot%` (chỉ đúng file này) | trung bình | 7 ngày |
| Báo cáo lỗi Windows | `WER` (của bạn và hệ thống) | thấp | 24 giờ |
| Nhật ký Windows cũ | `%SystemRoot%\Logs` | trung bình | 7 ngày |
| Cache Chrome / Edge / Firefox | thư mục cache của trình duyệt | thấp | — (bỏ qua khi trình duyệt đang chạy) |
| Windows Update cache | `SoftwareDistribution\Download` | trung bình | dừng `wuauserv` trước, chạy lại sau |
| Delivery Optimization | cache của `DoSvc` | trung bình | dừng/chạy lại service |
| Thùng rác | `SHEmptyRecycleBin` | cao | mặc định **không** chọn |

**Các lớp kiểm tra, áp dụng cho từng file, cả lúc quét lẫn ngay trước khi xoá:**
1. **Quét → xem trước → bạn chọn → xác nhận → xoá.** Có chế độ **chạy thử (dry-run)**:
   báo sẽ xoá gì mà không xoá.
2. **Không đi theo symlink, junction, reparse point**, không đụng file OneDrive "chỉ online".
3. **Bỏ qua file đang bị khoá** và **file mới hơn ngưỡng tuổi** của quy tắc. Tuổi tính theo
   mốc mới nhất giữa ngày tạo và ngày sửa.
4. **File thay đổi sau khi quét** (khác kích thước/thời gian) thì không xoá.
5. **Danh sách cấm cứng**, không bao giờ xoá dù quy tắc nói gì: `System32`, `SysWOW64`,
   `WinSxS`, `Prefetch`, `Boot`, gốc thư mục người dùng, Documents/Desktop/Pictures/Videos/
   Music/Downloads (kể cả khi đã chuyển sang ổ khác), Program Files, gốc mọi ổ đĩa `X:\`,
   ổ mạng, đường dẫn có `..`.
6. **Thư mục bảo vệ (whitelist)** do bạn tự thêm trong Cài đặt: không gì trong đó bị xoá
   (áp dụng cho Dọn rác, Ổ đĩa và Dọn phần còn sót).
7. Kết quả quét lưu phía Rust, hết hạn sau 30 phút; giao diện chỉ gửi **id của quy tắc**.

**Dọn tự động theo lịch** (tắt mặc định) chỉ được chọn các quy tắc **rủi ro thấp, không dừng
service, không làm rỗng Thùng rác**. Việc bạn bật và chọn quy tắc trong Cài đặt là bước xác
nhận; mọi lớp kiểm tra ở trên vẫn áp dụng.

**Dọn sâu hệ thống** chỉ dùng công cụ chính thức của Windows:
- WinSxS: `DISM /Online /Cleanup-Image /StartComponentCleanup`. **Không** dùng `/ResetBase`,
  nên bạn vẫn gỡ được các bản cập nhật Windows.
- Windows.old: Disk Cleanup (`cleanmgr /sagerun`) chỉ bật mục "Previous Installations".
  App không tự xoá file trong Windows.old.

## 3. Ổ đĩa — file lớn và file trùng

- Bạn chọn file từ kết quả quét; file **chỉ được chuyển vào Thùng rác** (`SHFileOperationW` +
  `FOF_ALLOWUNDO`), khôi phục được. File quá lớn so với Thùng rác: Windows hỏi lại trước khi
  xoá hẳn.
- **Bị chặn:** Windows, Program Files, ProgramData, file ở gốc ổ (`pagefile.sys`,
  `hiberfil.sys`…), file hồ sơ người dùng (`ntuser.dat`), `$Recycle.Bin`,
  `System Volume Information`.
- **Cảnh báo:** file trong AppData, ổ đĩa ảo (`.vhdx`, `.vmdk`… của WSL/Docker/máy ảo).
  Bộ lọc "Ẩn file rủi ro" bật mặc định.
- File trùng: luôn giữ **ít nhất một bản** mỗi nhóm; hard link (cùng một file, nhiều đường dẫn)
  không tính là bản trùng; trước khi chuyển, kiểm tra lại kích thước và thời gian sửa.

## 4. Ứng dụng — gỡ cài đặt và dọn phần còn sót

- Gỡ cài đặt chạy **trình gỡ của chính app** (đọc lại `UninstallString` từ registry, chỉ chạy
  đường dẫn tuyệt đối). Gỡ xong, app tự tìm phần còn sót.
- **Dọn phần còn sót:** backend tự tìm thư mục cài, thư mục AppData/ProgramData trùng tên app,
  shortcut Start Menu trỏ vào thư mục cài, key `Uninstall`. Mục chỉ khớp một phần tên ("có thể")
  mặc định không được chọn. Không bao giờ chọn:
  - thư mục dùng chung (Microsoft, Common Files, Packages, `.ssh`…);
  - thư mục chứa hoặc nằm trong thư mục cài của **app khác**;
  - thư mục đang có chương trình chạy.
- App còn **service, driver hoặc scheduled task** trỏ vào thư mục: cảnh báo, không chọn sẵn.
- File vào **Thùng rác**; key registry được **sao lưu ra file `.reg`** trước khi xoá
  (`%APPDATA%\com.pcmanager.app\backups`).

## 5. Tiến trình và khởi động

- **Không cho kết thúc** tiến trình hệ thống quan trọng (`csrss`, `wininit`, `winlogon`,
  `lsass`, `smss`, `services`, System, PID 0/4), tiến trình được đánh dấu critical, và chính
  PC Manager. Mọi thao tác xác minh lại PID + thời điểm tạo để không nhầm tiến trình mới
  trùng PID.
- Khởi động: tắt một mục bằng `StartupApproved` (cùng cơ chế Task Manager), **không xoá** mục
  gốc trong `Run`. Scheduled task và service chỉ hiển thị.

## 6. Nhật ký

Mọi thao tác thay đổi hệ thống (xoá, chuyển vào Thùng rác, kết thúc tiến trình, bật/tắt khởi
động, gỡ app, đổi cài đặt, nâng quyền…) được ghi vào SQLite cục bộ: thời gian, hành động, đối
tượng, kết quả. Xem trong **Cài đặt → Nhật ký thao tác**.

## 7. Boost RAM thực sự làm gì

Nói thẳng để không kỳ vọng sai:

- **Boost (nhẹ):** ghi dữ liệu đang chờ ra đĩa (modified list) và xoá phần cache **ưu tiên
  thấp** của standby list. Cache của app hay dùng được giữ lại.
- **Boost sâu:** xoá **toàn bộ** standby list. RAM "trống" tăng mạnh, nhưng **RAM khả dụng
  gần như không đổi** (cache standby vốn đã được tính là khả dụng), và app mở lần sau sẽ
  **chậm hơn** vì phải đọc lại từ đĩa. Chỉ nên dùng khi thật sự cần.
- **Smart Boost:** chỉ chạy khi RAM khả dụng dưới ngưỡng (mặc định 15%).
- Báo cáo sau mỗi lần Boost tách riêng "RAM khả dụng tăng thêm" (lợi ích thật) và "cache đã
  xoá", để không thổi phồng kết quả.
- **Thu gọn bộ nhớ (trim working set)** chỉ áp dụng cho tiến trình bạn tự chọn, không làm hàng
  loạt. Windows tự quản lý bộ nhớ khá tốt; thu gọn chỉ hữu ích trong vài trường hợp, và trang
  bộ nhớ sẽ được nạp lại khi app cần.

## 8. Giới hạn đã biết

- Bộ cài **không có chữ ký số từ nhà cung cấp chứng chỉ**. Windows SmartScreen và phần mềm
  diệt virus có thể cảnh báo; xem mục cài đặt trong [README](../README.vi.md#cài-đặt).
- Chỉ hỗ trợ Windows 10/11 x64.
- Test cần quyền admin chạy bằng `scripts/admin-test.ps1` (Windows hỏi UAC mỗi lần).
