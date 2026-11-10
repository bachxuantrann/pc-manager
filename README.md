# PC Manager

<img src="app-icon.svg" width="72" align="right" alt="">

Tiện ích nhẹ để chăm sóc máy Windows 10/11: theo dõi RAM, dọn rác, quản lý tiến trình và ứng dụng
khởi động, tìm app lâu không dùng, file lớn và file trùng. Viết bằng Tauri 2 + React + Rust.

- **Nhẹ:** bộ cài ~2.5 MB; chạy ngầm ở khay hệ thống chỉ ~2–15 MB RAM. Đóng cửa sổ là giải
  phóng hẳn phần giao diện.
- **An toàn:** chỉ xoá theo quy tắc cố định, luôn cho xem trước, file người dùng chọn chỉ vào
  Thùng rác, mọi thao tác đều được ghi nhật ký. Chi tiết: [docs/AN-TOAN.md](docs/AN-TOAN.md).
- **Không đòi quyền admin khi mở:** chức năng nào cần thì mới hỏi.

![Tổng quan](docs/screenshots/dashboard.png)

<table>
  <tr>
    <td><img src="docs/screenshots/cleaner.png" alt="Dọn rác: xem trước theo quy tắc"></td>
    <td><img src="docs/screenshots/disk.png" alt="Ổ đĩa: file trùng, ẩn file rủi ro"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/apps.png" alt="Ứng dụng: lần dùng cuối, gỡ cài đặt, dọn sót"></td>
    <td><img src="docs/screenshots/settings.png" alt="Cài đặt: Smart Boost, lịch dọn, thư mục bảo vệ, nhật ký"></td>
  </tr>
</table>

<sub>Ảnh chụp bằng dữ liệu giả (chế độ demo), không phải dữ liệu máy thật.</sub>

## Tính năng

| Trang | Làm được gì |
|---|---|
| Tổng quan, Bộ nhớ | RAM đang dùng / khả dụng / standby, biểu đồ 60 giây; Boost nhẹ, Smart Boost, Boost sâu |
| Dọn rác | Quét file tạm, cache trình duyệt, crash dump, log, Windows Update cache, Thùng rác; xem trước rồi mới dọn; chạy thử; dọn sâu WinSxS (DISM) và Windows.old (Disk Cleanup) |
| Tiến trình | CPU/RAM, nhà phát hành (chữ ký số), kết thúc, tạm dừng, đổi ưu tiên, thu gọn bộ nhớ, mở vị trí file |
| Khởi động | App chạy cùng Windows (registry, thư mục Startup, scheduled task, service); bật/tắt giống Task Manager |
| Ứng dụng | App đã cài, dung lượng, lần dùng cuối; lọc app lâu không dùng; gỡ cài đặt rồi tự dọn phần còn sót |
| Ổ đĩa | Dung lượng các ổ, file lớn nhất, file trùng nội dung (blake3); chuyển vào Thùng rác |
| Cài đặt | Ngưỡng Smart Boost, lịch dọn tự động, khởi động cùng Windows, thư mục bảo vệ, nhật ký thao tác |

## Cài đặt

1. Tải `PC Manager_x.y.z_x64-setup.exe` ở mục [Releases](https://github.com/bachxuantrann/pc-manager/releases).
2. Chạy bộ cài (cài vào `Program Files`, Windows hỏi UAC).
3. Bộ cài **chỉ ký bằng chứng chỉ tự ký**, không phải chứng chỉ của nhà cung cấp được Windows
   tin cậy. Vì vậy Windows SmartScreen có thể hiện "Windows protected your PC": bấm
   **More info → Run anyway**. Nếu không yên tâm, bạn có thể tự build từ mã nguồn (bên dưới).
4. Cần Microsoft Edge WebView2 Runtime (có sẵn trên Windows 11; bộ cài tự tải nếu thiếu).

Gỡ cài đặt trong Settings → Apps như mọi app khác; task khởi động cùng Windows (nếu đã bật)
được xoá theo.

## Quyền Administrator

| Cách chạy | Quyền |
|---|---|
| Mở app bình thường | Quyền thường, không hỏi UAC |
| Bấm **"Chạy với quyền Administrator"** (sidebar hoặc ngay chỗ cần) | Admin; app mở lại qua UAC, đúng trang đang xem |
| Khởi động cùng Windows | Admin, không hỏi lại (chỉ bật được khi app nằm trong Program Files) |

Cần quyền admin cho: Boost RAM, dọn file hệ thống (Windows\Temp, crash dump, Windows Update…),
đọc Prefetch, bật/tắt mục khởi động của cả máy, thao tác tiến trình của hệ thống, gỡ app cài
cho cả máy, dọn WinSxS.

## An toàn

Tóm tắt (đầy đủ ở [docs/AN-TOAN.md](docs/AN-TOAN.md)):

- Dọn rác **chỉ xoá file khớp quy tắc cố định** trong [`rules/cleaner.toml`](rules/cleaner.toml),
  không có lệnh nào nhận đường dẫn tuỳ ý để xoá.
- Luôn **quét → xem trước → xác nhận → xoá**; có **chạy thử (dry-run)**.
- Không đi theo symlink/junction; bỏ qua file đang khoá, file mới, file thay đổi sau khi quét.
- **Danh sách cấm cứng:** System32, SysWOW64, WinSxS (chỉ dọn qua DISM), Prefetch, gốc thư mục
  người dùng, Documents/Desktop/Pictures…, Program Files, gốc mọi ổ đĩa. Bạn có thể tự thêm
  **thư mục bảo vệ**.
- File lớn/trùng và phần còn sót sau khi gỡ app **chỉ vào Thùng rác**; key registry được sao lưu
  `.reg` trước khi xoá.
- Không cho kết thúc tiến trình hệ thống quan trọng; tắt mục khởi động không xoá mục gốc.
- Mọi thao tác thay đổi hệ thống được ghi nhật ký (Cài đặt → Nhật ký thao tác).
- Có 140+ test tự động, gồm test hộp cát cho logic chọn file xoá (junction, file khoá, file mới,
  thư mục cấm, file đổi sau khi quét).

## Boost RAM thực sự làm gì

Windows quản lý bộ nhớ khá tốt, nên PC Manager không hứa "tăng tốc thần kỳ":

- **Boost (khuyên dùng):** ghi dữ liệu chờ ra đĩa và xoá cache ưu tiên thấp; giữ cache của app
  hay dùng.
- **Boost sâu:** xoá toàn bộ cache standby. RAM "trống" tăng mạnh nhưng **RAM khả dụng gần như
  không đổi**, và app mở lần sau **chậm hơn**. Chỉ dùng khi thật cần.
- **Smart Boost:** chỉ chạy khi RAM khả dụng dưới ngưỡng (mặc định 15%).
- Kết quả luôn tách "RAM khả dụng tăng thêm" (lợi ích thật) khỏi "cache đã xoá".
- Thu gọn bộ nhớ chỉ áp dụng cho tiến trình bạn tự chọn.

## License

Phát hành theo **MIT hoặc Apache-2.0**, tuỳ bạn chọn: [LICENSE-MIT](LICENSE-MIT),
[LICENSE-APACHE](LICENSE-APACHE).

---

## English summary

PC Manager is a lightweight Windows 10/11 utility (Tauri 2 + React + Rust): memory monitor and
standby-list "Boost", rule-based junk cleaner with preview and dry-run, process and startup
manager, unused-app finder with leftover cleanup, large/duplicate file finder. It opens without
admin rights and asks for elevation only when a feature needs it. Deletion is limited to fixed
rules in `rules/cleaner.toml`; user-picked files only go to the Recycle Bin; system folders are
hard-blocked; every change is logged. The installer is self-signed, so SmartScreen may warn.
See [docs/AN-TOAN.md](docs/AN-TOAN.md) (Vietnamese) for the full safety model.
Licensed under MIT or Apache-2.0.

---

## Phát triển

Quy tắc cho AI agent: [CLAUDE.md](CLAUDE.md). Lộ trình theo giai đoạn: [PROMPTS.md](PROMPTS.md).

### Yêu cầu
- Node 20+, Rust stable `x86_64-pc-windows-msvc`, VS 2022 Build Tools (C++ + Windows SDK), WebView2.

### Chạy
```powershell
npm install
npm run tauri dev      # quyền thường; cần thử chức năng admin: bấm "Chạy với quyền Administrator" trong app
npm run tauri build    # installer NSIS ở src-tauri/target/release/bundle/nsis/ (chưa ký)
npm run build:signed   # như trên + ký bằng chứng chỉ tự ký (xem scripts/sign.ps1)
npx tauri icon app-icon.svg -o src-tauri/icons   # tạo lại icon từ app-icon.svg (xoá thư mục android/ios sinh thêm)
```

### Kiểm tra trước khi xong một giai đoạn
```powershell
cd src-tauri; cargo clippy --all-targets -- -D warnings; cargo test; cd ..; npm run build
```

### Test với quyền Administrator
Một số test (Boost thật, DISM, đọc đường dẫn service…) cần quyền admin. Script tự mở lại chính nó
bằng "Run as administrator" — mỗi lần chạy Windows hiện UAC để bạn xác nhận, không lưu quyền:
```powershell
.\scripts\admin-test.ps1                 # toàn bộ test, kể cả test #[ignore] cần admin
.\scripts\admin-test.ps1 -Filter memory  # chỉ test có "memory" trong tên
.\scripts\admin-test.ps1 -Mode dev       # mở `npm run tauri dev` trong cửa sổ admin
.\scripts\admin-test.ps1 -Mode ram       # đo RAM bản release (cần `npm run tauri build` trước)
```
Log nằm ở `src-tauri/target/admin-test.log`.

### CI và phát hành
- **Mỗi lần push lên `main` / mở PR** ([ci.yml](.github/workflows/ci.yml)): build giao diện, clippy, test, build bộ cài
  (chưa ký) — bộ cài tải được ở trang Actions, mục Artifacts.
- **Phát hành bản mới** ([release.yml](.github/workflows/release.yml)): đổi `version` ở `package.json`,
  `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, commit, rồi:
  ```powershell
  git tag -a v1.0.1 -m "PC Manager v1.0.1"; git push origin main v1.0.1
  ```
  GitHub tự kiểm tra, build bộ cài và tạo Release kèm SHA-256. Tag phải khớp phiên bản trong `package.json`.
- **Ký bộ cài trên CI (tuỳ chọn):** chạy `.\scripts\new-ci-signing-cert.ps1` một lần (cần `gh auth login`) — tạo
  chứng chỉ tự ký riêng cho CI và cất vào GitHub Secrets. Không có secret thì Release là bản chưa ký.

### Ảnh chụp màn hình
`npm run dev` rồi `.\scripts\screenshots.ps1` — mở các trang ở chế độ demo (`?demo&page=...`, dữ liệu giả trong
`src/dev/demo.ts`, chỉ có ở bản dev) và chụp bằng Edge headless vào `docs/screenshots/`.

### Manifest
`src-tauri/app.manifest` (asInvoker + Common-Controls v6) cho cả bản debug và release: mở app
không hỏi UAC. Nâng quyền dùng `ShellExecuteExW("runas")` chạy lại chính exe.
