# Plan triển khai — PC Manager (từ bản rà soát 2026-11-11)

Nguồn: báo cáo rà soát [`docs/audit-2026-11-11.html`](audit-2026-11-11.html) (bằng chứng theo file:dòng cho từng mục).
13 sửa nhỏ "đã sửa ngay" trong báo cáo đã phát hành ở **v1.1.3**. File này là các đề xuất đã được duyệt,
triển khai lần lượt theo 6 giai đoạn.

**Agent: đọc `CLAUDE.md` rồi đọc file này trước khi làm. Làm xong mục nào thì đánh dấu `[x]` ở bảng
"Trạng thái" (kèm phiên bản/commit) trong cùng commit của giai đoạn đó.**

## Quy tắc chung agent phải giữ ở mọi giai đoạn

1. Đọc `CLAUDE.md` trước; mục “AN TOÀN” không được nới lỏng. `unsafe` chỉ trong `sys/`, không `unwrap/expect` ngoài test.
2. Không thêm thư viện mới, không đổi stack, không commit/push/tag khi chưa được yêu cầu.
3. Trước khi báo xong: `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npm run build` đều qua.
4. Sửa giao diện: chạy `scripts/layout-check.js` ở 860x560, 1100x720, 1366x768, 1920x1080, cả hai theme — `issues` rỗng, rồi chụp ảnh xem bằng mắt.
5. Thay đổi có thể ảnh hưởng RAM/CPU: đo lại theo mục “Hiệu năng — mốc đo & ngân sách”.
6. Mỗi thay đổi người dùng thấy được: thêm dòng vào `CHANGELOG.md` (mục phiên bản kế tiếp).

Thêm: mỗi giai đoạn một commit (chỉ commit khi người dùng yêu cầu). Người dùng thử bộ cài trên máy trước khi phát hành.

## Trạng thái

| Giai đoạn | Mục | Trạng thái |
|---|---|---|
| 1 — An toàn và chính xác | S1, S2, E1 | [x] xong (v1.2.0; E1 đã đo bằng test admin; chờ thử gỡ app per-user) |
| 2 — Tác vụ dài: tiến độ và nối lại | P6, P7 | [x] xong (v1.2.0; chờ thử tay đóng/mở cửa sổ khi đang chuyển file vào Thùng rác) |
| 3 — Hiệu năng và thời gian build | P1, P2, P3, P4, P5, B1 | [x] xong (P1 đo ở bản dev; P2 chưa đo CPU WebView2 trên app cài thật; B1: build release ~194 s → ~160 s) |
| 4 — Đồng bộ giao diện | U1, U2, U3, U4, U5 | [x] xong (layout-check sạch 4 kích thước × 2 theme) |
| 5 — Bổ sung về chuyên môn Windows | E2, E3, E4, E5, E6 | [x] xong (E5 không dùng WinRT: registry + manifest; chờ so tab Startup với Task Manager) |
| 6 — Dọn mã, tài liệu, skills | S3, B2, B3, T1, T2, T3, T4, T5 | [x] xong (T2: máy này không có skill cấp người dùng; trên máy khác cần tự gỡ) |
| 7 — Tự cập nhật | U6 | [x] xong v1.2.0 (khoá đã tạo; chờ thử cập nhật thật 1.2.0 → 1.2.1) |

### Quyết định của người dùng (2026-11-12)
- **S3:** giữ 5.000 dòng `action_log` mới nhất; xoá `app_usage` có `last_seen` cũ hơn 1 năm.
- **B3:** xoá chú thích `eslint-disable` thừa (không thêm ESLint).
- **T5:** bỏ README tiếng Nhật và tiếng Trung; giữ tiếng Anh + tiếng Việt.
- **E5:** làm và đo; được vượt 12 MB nếu chức năng đáng giá (báo số đo cho người dùng).
- **T2:** agent tự tắt các skill cấp người dùng không dùng/trùng/lỗi thời theo bản rà soát.
- **T4:** chuyển `PROMPTS.md` vào `docs/history/` và tạo `AGENTS.md` trỏ tới `CLAUDE.md`.
- **E6:** sửa mô tả và bỏ tick mặc định (`default_selected = false`).
- **E2:** chỉ dùng `form_factor == "DIMM"` (không đọc chassis SMBIOS).

### Cần người dùng quyết định (câu hỏi gốc)
- **S3:** giữ bao nhiêu dòng `action_log` (đề xuất 5.000); có xoá `app_usage` cũ hơn 1 năm không.
- **B3:** (a) thêm ESLint + `eslint-plugin-react-hooks` vào CI (thêm devDependency) hay (b) xoá chú thích thừa.
- **T5:** giữ hay bỏ README tiếng Nhật / tiếng Trung.
- **E5:** chỉ làm nếu đo RAM Rust lúc rảnh vẫn ≤ 12 MB; vượt => báo người dùng và bỏ.

### Ghi chú khi triển khai (từ lần đối chiếu code 2026-11-12)
- **S1:** nếu không chạy được trình gỡ HKCU dưới quyền thường (vd. không có Explorer, `spawn_as_shell_user` lỗi)
  thì **báo lỗi**, tuyệt đối không quay về chạy bằng token admin. Trình gỡ per-user tự đòi quyền admin (mã 740)
  => đi qua UAC như nhánh HKLM hiện có, không chạy thẳng bằng token admin của PC Manager.
- **T3:** khi rút gọn `CLAUDE.md`, giữ lại mục "Phát hành" (agent cần mỗi lần nâng phiên bản/tag).
- **U3:** cờ "không lấy lại được gì" của WinSxS: backend đã có `cleaner::system::NOTHING_FREED` (v1.1.2) —
  trả cờ trong `SystemCleanReport`, bỏ hằng trùng trong `SystemDeepClean.tsx`.

---

## Giai đoạn 1 — An toàn và chính xác

### S1. Trình gỡ của app cài riêng cho người dùng (HKCU) chạy bằng quyền Administrator (ưu tiên cao)
- Vấn đề: Khi PC Manager đang nâng quyền bằng CHÍNH tài khoản đang đăng nhập, `UninstallString` đọc từ HKCU được chạy thẳng với token admin. HKCU ghi được bởi mọi chương trình quyền thường, nên đây là đường leo thang đặc quyền (phần mềm xấu sửa `UninstallString`, chờ người dùng bấm Gỡ). Hiện chỉ hạ quyền khi nâng quyền bằng tài khoản KHÁC.
- File: src-tauri/src/usage/mod.rs:361
- Cần làm: App `HKCU\…` và tiến trình đang elevated ⇒ luôn chạy qua `sys::launch::spawn_as_shell_user` (token của Explorer), không phụ thuộc `is_other_user()`. Windows Settings cũng chạy trình gỡ per-user ở quyền thường.
- Xong khi: Test: id `HKCU\…` + elevated đi nhánh `spawn_as_shell_user`. Thử tay: gỡ một app per-user khi PC Manager đang admin, trình gỡ không có khiên UAC và vẫn gỡ được.

### S2. Dọn tự động ghi đè bản xem trước của trang Dọn rác (ưu tiên trung bình)
- Vấn đề: `auto_clean` gọi `scan()` (lưu kết quả vào `last`) rồi `clean()`. Nếu người dùng đang xem kết quả quét, bản đó bị thay — bấm Dọn sẽ báo “kết quả quét đã cũ”.
- File: src-tauri/src/cleaner/mod.rs:179
- Cần làm: Cho `auto_clean` quét và dọn trên dữ liệu riêng (tách hàm nội bộ nhận `StoredScan` cục bộ), không đụng `self.last`.
- Xong khi: Test: có kết quả quét A, chạy `auto_clean`, sau đó `clean(A.scan_id)` vẫn chạy được.

### E1. Smart Boost / tự động dùng “Boost sâu” đúng lúc máy đang thiếu RAM (ưu tiên cao)
- Vấn đề: Standby là cache Windows ĐÃ tính vào RAM khả dụng và nhường ngay khi app cần (chính `memory/calc.rs` đã ghi rõ). Xoá toàn bộ standby khi RAM thấp không tăng RAM khả dụng, chỉ làm mất cache nên máy đọc đĩa nhiều hơn. Lợi ích thật chỉ đến từ flush modified. Câu chữ giao diện (“xoá toàn bộ cache để ứng dụng có RAM ngay”) và Chẩn đoán (“Boost giải phóng cache”) vì thế chưa đúng.
- File: src-tauri/src/memory/mod.rs:220, src/components/BoostPanel.tsx:183, src-tauri/src/diagnosis/rules.rs:127
- Cần làm: Smart Boost và auto dùng `BoostMode::Light` (flush modified + standby ưu tiên thấp). Sửa mô tả thẻ Smart Boost; trong Chẩn đoán khi RAM < 10% đặt “đóng bớt app nặng (mở Tiến trình)” làm hành động chính, Boost là phụ.
- Xong khi: Test `smart_boost` trả báo cáo `mode = light`. Đo trên máy thật trước/sau: RAM khả dụng tăng tương đương, standby còn lại lớn hơn.

## Giai đoạn 2 — Tác vụ dài: tiến độ và nối lại

### P6. Một số tác vụ dài chưa có tiến độ / không nối lại được (ưu tiên trung bình)
- Vấn đề: `CLAUDE.md` yêu cầu tác vụ lâu có % và chạy qua `jobs::run`. Hiện: chuyển tới 5.000 file vào Thùng rác (`disk_recycle`), dọn phần còn sót (`apps_leftovers_clean`), tải danh sách ứng dụng (`apps_list`) chạy ngoài `jobs`; dọn rác chỉ báo tiến độ theo rule, một rule 100.000 file thì thanh đứng yên.
- File: src-tauri/src/commands/disk.rs:46, src-tauri/src/commands/apps.rs:104, src-tauri/src/commands/apps.rs:16, src-tauri/src/cleaner/mod.rs:363
- Cần làm: Thêm kind `disk.recycle`, `apps.leftovers`, `apps.list` (KINDS, `JobKind`, `JOB_PAGE`, `tracked()`, `useJobResume`); `engine::delete` báo tiến độ định kỳ theo số file đã xử lý.
- Xong khi: Đóng cửa sổ giữa lúc đang chuyển file vào Thùng rác rồi mở lại vẫn thấy tiến độ/kết quả; thanh tiến độ Dọn rác chạy đều trong một rule lớn.

### P7. Dọn tự động chỉ kiểm tra lần đầu sau 30 phút; quét trùng không kẹp kích thước tối thiểu (ưu tiên thấp)
- Vấn đề: Luồng `auto-clean` ngủ 30 phút rồi mới kiểm tra — phiên dùng máy ngắn hơn thì không bao giờ dọn. `disk_scan_duplicates` nhận `min_bytes` bất kỳ từ UI; giá trị nhỏ trên ổ vài triệu file sẽ giữ toàn bộ danh sách trong RAM.
- File: src-tauri/src/lib.rs:91, src-tauri/src/disk/mod.rs:353
- Cần làm: Kiểm tra lần đầu sau ~2 phút rồi mới theo chu kỳ 30 phút. Kẹp `min_bytes` ở command (đề xuất ≥ 64 KB cho quét trùng, ≥ 1 MB cho file lớn).
- Xong khi: Test `validate`/clamp; bật lịch dọn, mở app, sau 2–3 phút có dòng `cleaner.auto` trong nhật ký.

## Giai đoạn 3 — Hiệu năng và thời gian build

### P1. Số liệu RAM được hỏi 2 giây/lần ở MỌI trang, trang ẩn vẫn vẽ lại (ưu tiên trung bình)
- Vấn đề: `MemoryFeedProvider` nằm ngoài các trang nên luôn “đang hiển thị”. Ở trang Cài đặt/Ổ đĩa… app vẫn gọi `memory_status` mỗi 2 giây, và Tổng quan + Bộ nhớ (đang ẩn nhưng giữ sống) vẫn render lại biểu đồ. Trái tinh thần “không polling khi không hiển thị”.
- File: src/hooks/useMemoryFeed.tsx:55, src/App.tsx
- Cần làm: Provider chỉ chạy khi có ít nhất một trang tiêu thụ đang được chọn (đếm đăng ký từ `useMemoryFeed` kết hợp `usePageActive`). Biểu đồ 60 giây chấp nhận khoảng trống khi rời trang (hoặc xoá lịch sử khi quay lại).
- Xong khi: Ở trang Cài đặt: không còn lệnh `memory_status` trong 10 giây (xem bằng DevTools/log). CPU WebView2 khi rảnh ở trang khác giảm.

### P2. Bảng Tiến trình vẽ lại toàn bộ 300+ dòng mỗi 2 giây (ưu tiên trung bình)
- Vấn đề: WebView chạy không GPU; mỗi lần làm mới React dựng lại mọi `<tr>`. Phía Rust mỗi thao tác (kill, trim…) lại `enumerate()` toàn bộ thêm 1–2 lần.
- File: src/pages/Processes.tsx:74, src-tauri/src/process/mod.rs
- Cần làm: Tách dòng thành component `memo` so sánh theo (cpu, ram, publisher, suspended, selected); giữ thứ tự ổn định. Đo CPU trước/sau bằng trang Tiến trình mở 60 giây.
- Xong khi: CPU của WebView2 ở trang Tiến trình giảm rõ (ghi số đo vào CHANGELOG).

### P3. Sampler ghi lại toàn bộ danh sách exe vào SQLite mỗi 60 giây (ưu tiên thấp)
- Vấn đề: Khoảng 150–300 lệnh UPSERT mỗi phút suốt thời gian app chạy nền, trong khi “lần dùng cuối” chỉ cần độ chính xác theo ngày.
- File: src-tauri/src/usage/sampler.rs:37
- Cần làm: Ghi ngay exe MỚI xuất hiện; các exe còn lại làm mới `last_seen` tối đa 15 phút/lần.
- Xong khi: Test sampler: hai lần `tick` liên tiếp, lần hai không ghi dòng nào nếu không có exe mới.

### P4. Quét rác kiểm tra danh sách cấm cho từng file (ưu tiên thấp)
- Vấn đề: `bans.check` chuẩn hoá đường dẫn 2 lần và lấy khoá whitelist cho MỖI file; thư mục hàng trăm nghìn file tạm tốn đáng kể. Glob cũng hạ chữ thường mẫu ở mỗi lần so.
- File: src-tauri/src/cleaner/engine.rs:119, src-tauri/src/cleaner/glob.rs
- Cần làm: Kiểm tra cấm theo THƯ MỤC một lần rồi chỉ kiểm tra phần “shallow/exact” cho file; hạ chữ thường mẫu include/exclude một lần khi nạp rule. Bước kiểm tra lại trước khi xoá (`recheck`) giữ nguyên từng file.
- Xong khi: Toàn bộ `cleaner/tests.rs` qua; đo thời gian quét `%TEMP%` có ≥ 50.000 file trước/sau.

### P5. Mở trang Cài đặt phải chạy `schtasks.exe` (ưu tiên thấp)
- Vấn đề: `autostart_enabled()` sinh một tiến trình (50–150 ms) mỗi lần đọc cài đặt, trong khi `sys/tasks.rs` đã có sẵn kết nối `ITaskService`.
- File: src-tauri/src/settings.rs:147
- Cần làm: Thêm `sys::tasks::task_exists(name)` qua COM; giữ `schtasks` cho việc tạo/xoá.
- Xong khi: `settings_get` không còn sinh tiến trình con.

### B1. `crate-type` build thừa `staticlib` + `cdylib` (ưu tiên trung bình)
- Vấn đề: Hai loại này chỉ cần cho mobile. Với `lto = "fat"`, mỗi loại là một lần LTO/sinh mã riêng ⇒ build release lâu hơn không cần thiết (đa OS/mobile đang để sau).
- File: src-tauri/Cargo.toml:11
- Cần làm: Để `crate-type = ["rlib"]` (hoặc bỏ dòng). Đo thời gian `npm run tauri build` trước/sau và xác nhận bộ cài chạy.
- Xong khi: Bộ cài NSIS build được, cài và chạy bình thường; ghi thời gian build trước/sau.

## Giai đoạn 4 — Đồng bộ giao diện

### U1. Trang Tiến trình và Khởi động không dùng `PageShell` (ưu tiên trung bình)
- Vấn đề: Hai trang tự viết `<section>/<h1>` nên tiêu đề không to lên ở màn 2xl và khoảng cách khác 8 trang còn lại.
- File: src/pages/Processes.tsx:245, src/pages/Startup.tsx:155
- Cần làm: Chuyển sang `PageShell` (title, subtitle, `actions` chứa ô tìm kiếm / Tải lại).
- Xong khi: `layout-check` sạch ở 4 kích thước × 2 theme; ảnh chụp không lệch so với trước ở 1100x720.

### U2. Emoji và ký tự được dùng làm biểu tượng (ưu tiên thấp)
- Vấn đề: `🛡`, `🔒`, `✓`, `↻`, `▸▾` phụ thuộc font máy, trái quy ước “biểu tượng tự vẽ trong `Icons.tsx`”.
- File: src/components/Elevation.tsx:57, src/pages/Processes.tsx, src/pages/Startup.tsx
- Cần làm: Thêm `IconLock`, `IconCheck`, `IconRefresh`, `IconChevron` vào `Icons.tsx` và thay thế.
- Xong khi: Không còn emoji trong `src/` (grep); layout-check sạch.

### U3. Class và thành phần lặp lại ngoài `ui.ts` (ưu tiên thấp)
- Vấn đề: Ô chọn ưu tiên ở Tiến trình tự viết class; `Notice` bo `rounded-md` trong khi khung khác `rounded-xl`; cột Publisher viết hai lần (Tiến trình, Khởi động); Sidebar/Disk/Settings còn chuỗi màu riêng. `ArcGauge` dùng `id="arc-glow"` cố định (trùng id nếu có 2 đồng hồ). Hằng `NOTHING_FREED` khai báo ở cả Rust và TS.
- File: src/components/Card.tsx:32, src/components/ArcGauge.tsx:38, src/components/SystemDeepClean.tsx:16
- Cần làm: Tách `PublisherCell` dùng chung; dùng `SELECT`/`PANEL`; `useId()` cho gradient; backend trả sẵn cờ “không lấy lại được gì”.
- Xong khi: layout-check sạch; không còn chuỗi class trùng `SURFACE`/`DIALOG` ngoài `ui.ts`.

### U4. Thanh trượt ngưỡng Smart Boost lưu và ghi nhật ký mỗi lần nhả phím/chuột (ưu tiên thấp)
- Vấn đề: `onPointerUp`/`onKeyUp` gọi `save` kể cả khi giá trị không đổi (bấm Tab qua cũng ghi một dòng `settings.memory`).
- File: src/pages/Memory.tsx:75
- Cần làm: Chỉ lưu khi giá trị khác bản đã lưu.
- Xong khi: Bấm Tab qua thanh trượt không tạo dòng mới trong Nhật ký thao tác.

### U5. Nhận biết lỗi “thiếu quyền” bằng cách dò chữ trong thông báo (ưu tiên thấp)
- Vấn đề: Trang Khởi động dùng regex trên message tiếng Anh/Việt của Windows; máy ngôn ngữ khác sẽ không khớp.
- File: src/pages/Startup.tsx:111
- Cần làm: Backend đổi `ERROR_ACCESS_DENIED` thành `AppError::Forbidden("cần quyền Administrator…")`; UI kiểm tra `code === "forbidden"`.
- Xong khi: Tắt một mục HKLM khi chạy quyền thường ⇒ thông báo có nút nâng quyền, không phụ thuộc ngôn ngữ Windows.

## Giai đoạn 5 — Bổ sung về chuyên môn Windows

### E2. Gợi ý “Bật XMP/EXPO” hiện cả trên laptop (ưu tiên trung bình)
- Vấn đề: Quy tắc chỉ so `configured_mts < speed_mts`. Laptop (SODIMM/RAM hàn) hầu như không có XMP; nhiều firmware ghi `Speed` là mức tối đa của khe chứ không phải của thanh RAM ⇒ gợi ý sai.
- File: src-tauri/src/diagnosis/mod.rs:133, src-tauri/src/diagnosis/rules.rs:278
- Cần làm: Chỉ báo khi `form_factor == "DIMM"` (máy bàn); tuỳ chọn đọc thêm SMBIOS type 3 (chassis) để loại laptop/mini-PC.
- Xong khi: Test `rules`: khe SODIMM chạy chậm ⇒ không có finding `ram-xmp`; khe DIMM ⇒ có.

### E3. Rule dọn cache trình duyệt còn thiếu (ưu tiên trung bình)
- Vấn đề: Chrome/Edge mới chỉ dọn `Cache`, `Code Cache`, `GPUCache` trong từng profile. Chưa có `Service Worker\CacheStorage`, `ShaderCache`/`GrShaderCache` ở `User Data`, và các trình duyệt Chromium khác (Brave, Opera, Vivaldi, Cốc Cốc). Firefox thiếu `startupCache`.
- File: rules/cleaner.toml:142
- Cần làm: Thêm path cho rule hiện có và rule mới cho từng trình duyệt (kèm `skip_if_running`). KHÔNG đụng `Service Worker\Database`, `IndexedDB`, `Local Storage` (dữ liệu trang web).
- Xong khi: Test `embedded_rules_are_valid` qua; thử dry-run trên máy có các trình duyệt đó, kiểm tra danh sách file chỉ nằm trong thư mục cache.

### E4. Trang Khởi động thiếu app Store và tên dịch vụ dạng tài nguyên (ưu tiên thấp)
- Vấn đề: Task Manager hiện cả startup task của app UWP/MSIX; PC Manager thì không. Dịch vụ có `DisplayName` dạng `@dll,-123` bị thay bằng tên key thay vì tên hiển thị. `.Lnk` (hoa thường lẫn lộn) không bị cắt đuôi.
- File: src-tauri/src/startup/mod.rs:316
- Cần làm: Đọc startup task của gói (registry `Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\SystemAppData\*\*StartupId`, chỉ đọc ở v1); giải tên bằng `RegLoadMUIStringW` trong `sys/`; cắt đuôi `.lnk` không phân biệt hoa thường.
- Xong khi: Số app ở tab “Ứng dụng” khớp tab Startup của Task Manager trên máy có app Store tự khởi động.

### E5. Trang Ứng dụng không liệt kê app Store (MSIX) (ưu tiên thấp)
- Vấn đề: Chỉ đọc key `Uninstall`. App cài từ Microsoft Store (thường chiếm nhiều dung lượng) không xuất hiện nên “app không dùng” thiếu sót.
- File: src-tauri/src/usage/installed.rs:97
- Cần làm: Tuỳ chọn: liệt kê gói của người dùng qua `PackageManager` (WinRT) — chỉ xem + mở Settings để gỡ. Cần đo RAM vì nạp thêm WinRT; nếu vượt ngân sách thì bỏ.
- Xong khi: Danh sách có thêm app Store, RAM Rust lúc rảnh vẫn ≤ 12 MB.

### E6. Mô tả rule “Bản cập nhật Windows đã tải” chưa chính xác (ưu tiên thấp)
- Vấn đề: Ghi “File cập nhật đã cài xong”, nhưng `SoftwareDistribution\Download` cũng chứa bản đã tải mà CHƯA cài; xoá thì Windows tải lại.
- File: rules/cleaner.toml:188
- Cần làm: Sửa mô tả: “File cập nhật đã tải (đã cài hoặc đang chờ). Windows sẽ tải lại bản chưa cài.” Cân nhắc `default_selected = false`.
- Xong khi: Chỉ đổi văn bản; test rule vẫn qua.

## Giai đoạn 6 — Dọn mã, tài liệu, skills

### S3. Cơ sở dữ liệu lớn dần không giới hạn (ưu tiên thấp)
- Vấn đề: `action_log` không bao giờ được cắt (auto Smart Boost, mỗi file vào Thùng rác đều ghi một dòng). `app_usage` giữ mọi đường dẫn exe từng thấy, kể cả trình cài tên ngẫu nhiên trong Temp.
- File: src-tauri/src/store/mod.rs:13, src-tauri/src/store/mod.rs:110
- Cần làm: Khi mở DB: giữ N dòng nhật ký mới nhất (đề xuất 5.000) và xoá `app_usage` có `last_seen` cũ hơn 1 năm. Cần bạn quyết định con số vì đây là chính sách lưu giữ.
- Xong khi: Test với DB trong RAM: chèn N+100 dòng, mở lại còn đúng N.

### B2. Mã lặp ở tầng command và tiện ích (ưu tiên thấp)
- Vấn đề: 4 hàm `blocking()` giống nhau trong `commands/*`; 2 hàm `now_ms`; 2 hàm `expand_env` (`sys/session.rs` và `startup/command.rs`); `run_elevated` và `run_elevated_pid` gần như trùng.
- File: src-tauri/src/commands/cleaner.rs:10, src-tauri/src/store/mod.rs:158, src-tauri/src/sys/session.rs:103, src-tauri/src/sys/shell.rs:70
- Cần làm: Một `commands::blocking(app, what, f)`; một `now_ms`; `session::expand_env` gọi bản có `lookup`; `run_elevated` gọi `run_elevated_pid`.
- Xong khi: Không đổi hành vi; toàn bộ test qua.

### B3. Có chú thích `eslint-disable` nhưng dự án không có ESLint (ưu tiên thấp)
- Vấn đề: 3 chú thích `react-hooks/exhaustive-deps` không có tác dụng; lỗi thiếu dependency của hook không được CI bắt.
- File: src/components/Elevation.tsx, src/hooks/useWindowVisible.ts, src/pages/Diagnosis.tsx
- Cần làm: Chọn một: (a) thêm ESLint + `eslint-plugin-react-hooks` vào CI (thêm devDependency — cần bạn đồng ý), hoặc (b) xoá chú thích.
- Xong khi: CI có bước lint, hoặc không còn chú thích thừa.

### T1. Skill `rust-skills` trong repo nặng và chứa nhiều quy tắc bị cấm (ưu tiên trung bình)
- Vấn đề: `SKILL.md` 37,8 KB (≈10 nghìn token mỗi lần gọi), 265 file quy tắc (1,4 MB). Khoảng 24 dòng trong mục lục trỏ tới thứ `CLAUDE.md` cấm (tokio, tracing, proptest, criterion, PGO, `target-cpu`, arena…) và nhiều nhóm không áp dụng cho app này (`async-` 18, `proj-` 14, `doc-` 12, `macro-` 8, `obs-` 7). Agent phải đọc rồi tự loại — tốn token và dễ áp dụng nhầm.
- File: .claude/skills/rust-skills/SKILL.md, CLAUDE.md (mục “Chuẩn code Rust”)
- Cần làm: Tạo bản rút gọn theo dự án: giữ nhóm `own/err/mem/unsafe/conc/num/type/conv/serde/pat/coll/name/lint/anti/perf`, xoá file quy tắc bị cấm, ghi rõ trong `SOURCE.txt` đã cắt gì. Khi đó danh sách “không dùng” trong `CLAUDE.md` rút còn 2–3 dòng.
- Xong khi: `SKILL.md` ≤ 15 KB; không còn quy tắc nào mâu thuẫn `CLAUDE.md`.

### T2. 39 skill cấp người dùng nạp vào mọi phiên, trùng và không liên quan (ưu tiên trung bình)
- Vấn đề: `~/.claude/skills` (6,7 MB) có `rust-router` (“dùng cho MỌI câu hỏi Rust”), `m01`–`m15`, `unsafe-checker`, `coding-guidelines` trùng vai trò với `rust-skills`; cùng `domain-web/fintech/iot/embedded/ml/cloud-native/cli`, `rust-daily`, `core-*` không liên quan dự án. Riêng phần mô tả đã ≈10 nghìn ký tự (≈3–4 nghìn token) cho MỖI phiên. Mã nguồn còn trích mã quy tắc của bộ này (`unsafe-checker ffi-15`, `m05-type-driven`) trong khi `CLAUDE.md` nói chuẩn là skill trong repo.
- File: ~/.claude/skills (ngoài repo — tôi không sửa), src-tauri/src/sys/handle.rs, src-tauri/src/sys/process.rs, src-tauri/src/memory/calc.rs
- Cần làm: Chọn MỘT bộ chuẩn Rust. Nếu giữ `rust-skills` trong repo: gỡ/tắt bộ `rust-router` + `m*` + `domain-*` ở cấp người dùng và đổi 5 chú thích trích dẫn sang mô tả bằng lời. Việc gỡ skill cấp người dùng do bạn tự làm.
- Xong khi: Danh sách skill đầu phiên chỉ còn skill thực sự dùng; grep `unsafe-checker|m05-` trong `src-tauri` không còn kết quả.

### T3. `CLAUDE.md` 18 KB (≈6 nghìn token mỗi phiên) có phần lặp và phần ít khi cần (ưu tiên thấp)
- Vấn đề: Quy định manifest `asInvoker` xuất hiện ở 3 mục; “Máy mới”, “Phát hành”, mốc đo hiệu năng chi tiết chỉ cần cho vài loại việc.
- File: CLAUDE.md
- Cần làm: Gộp phần lặp; chuyển “Máy mới” và “Phát hành” sang `CONTRIBUTING.md`, mốc đo sang `docs/PERF.md`, để lại một dòng trỏ tới. Mục AN TOÀN và các “quyết định của người dùng” giữ nguyên văn.
- Xong khi: `CLAUDE.md` ≤ 11 KB, không mất quy tắc nào (so từng gạch đầu dòng).

### T4. `PROMPTS.md` lỗi thời; chưa có `AGENTS.md` (ưu tiên thấp)
- Vấn đề: `PROMPTS.md` chỉ còn giá trị lịch sử (đã gắn ghi chú). Dự án không có `AGENTS.md` — agent không phải Claude Code sẽ không đọc `CLAUDE.md`.
- File: PROMPTS.md
- Cần làm: Xoá `PROMPTS.md` hoặc chuyển vào `docs/history/`. Nếu dùng agent khác: tạo `AGENTS.md` một dòng trỏ tới `CLAUDE.md`.
- Xong khi: Không còn tài liệu mâu thuẫn `CLAUDE.md` ở thư mục gốc.

### T5. 4 bản README + 2 bản tài liệu an toàn phải đồng bộ tay (ưu tiên thấp)
- Vấn đề: `README.md`/`.vi`/`.ja`/`.zh-CN` và `docs/AN-TOAN.md`/`SAFETY.md` — mỗi thay đổi tính năng tốn 4–6 lần công sửa; app dành cho cá nhân và vài người bạn.
- File: README.md, README.vi.md, README.ja.md, README.zh-CN.md
- Cần làm: Giữ tiếng Việt + tiếng Anh; hai bản còn lại bỏ hoặc ghi rõ “có thể chậm cập nhật”.
- Xong khi: Theo quyết định của bạn.

## Giai đoạn 7 — Tự cập nhật (yêu cầu mới 2026-11-12)

### U6. Cập nhật lên bản mới nhất ngay trong app (cách A: plugin updater chính thức của Tauri)
- Vấn đề: người dùng phải vào GitHub Release tải bộ cài bằng tay — bạn bè dùng app gần như không bao giờ cập nhật.
- Quyết định của người dùng: dùng `tauri-plugin-updater` (thêm 1 thư viện — đã đồng ý). Không tự viết bộ tải/kiểm hash.
- Cần làm:
  - Backend: plugin updater, endpoint `latest.json` trên GitHub Release mới nhất; khoá công khai minisign nhúng trong
    `tauri.conf.json`. Chỉ kiểm tra khi cửa sổ mở, tối đa 1 lần/ngày (lưu mốc trong SQLite); không luồng nền thường trực.
  - UI: Cài đặt có công tắc "Tự kiểm tra cập nhật" (mặc định bật, nói rõ chỉ gửi yêu cầu tới GitHub) và nút
    "Kiểm tra cập nhật". Có bản mới => thông báo kèm "Có gì mới"; chỉ cài khi người dùng bấm "Cập nhật" (UAC hỏi
    vì cài vào Program Files). Ghi nhật ký `app.update` (AN TOÀN #7).
  - CI (`release.yml`): build với `TAURI_SIGNING_PRIVATE_KEY`/`..._PASSWORD` từ secret, tải lên `.sig` và `latest.json`.
    Thiếu secret => vẫn phát hành bộ cài như cũ nhưng không có `latest.json` (app báo "không kiểm tra được").
  - Người dùng tự làm một lần: tạo cặp khoá (`npx tauri signer generate`), thêm 2 secret vào repo. Khoá riêng không vào repo.
- Xong khi: từ bản N cài sẵn, phát hành bản N+1 => app thấy bản mới, bấm Cập nhật, UAC, cài xong mở lại đúng N+1;
  `latest.json`/chữ ký sai => từ chối cài. RAM Rust lúc rảnh không tăng quá ngân sách (đo `admin-test.ps1 -Mode ram`).
