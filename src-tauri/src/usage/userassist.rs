//! UserAssist: Windows ghi lại mỗi lần người dùng mở chương trình qua Explorer /
//! Start menu / taskbar, tại `HKCU\...\Explorer\UserAssist\{GUID}\Count`.
//!
//! - Tên value mã hoá ROT13 (chỉ chữ cái A-Z/a-z), có thể bắt đầu bằng GUID
//!   của known folder (vd `{6D809377-...}\App\app.exe` = Program Files).
//! - Dữ liệu 72 byte (Win7+): +4 số lần chạy (u32), +60 FILETIME lần chạy cuối.

use std::path::PathBuf;

use crate::startup::approved::filetime_to_unix_ms;

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\UserAssist";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Đường dẫn đã giải mã và thay known folder; có thể là .exe hoặc .lnk,
    /// hoặc AppUserModelID (không phải đường dẫn) với app Store.
    pub target: String,
    pub run_count: u32,
    pub last_run_ms: Option<i64>,
}

/// ROT13 chỉ xoay chữ cái Latin; ký tự khác giữ nguyên.
pub fn rot13(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a') + 13) % 26 + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A') + 13) % 26 + b'A') as char,
            _ => c,
        })
        .collect()
}

/// (số lần chạy, FILETIME lần cuối) từ dữ liệu 72 byte. Định dạng XP (16 byte) bị bỏ qua.
pub fn parse_data(b: &[u8]) -> Option<(u32, Option<i64>)> {
    if b.len() < 68 {
        return None;
    }
    let count = u32::from_le_bytes(b.get(4..8)?.try_into().ok()?);
    let ft = u64::from_le_bytes(b.get(60..68)?.try_into().ok()?);
    Some((count, if ft == 0 { None } else { filetime_to_unix_ms(ft) }))
}

/// Known folder GUID hay gặp trong UserAssist → biến môi trường tương ứng.
const KNOWN_FOLDERS: &[(&str, &str)] = &[
    ("{6D809377-6AF0-444B-8957-A3773F02200E}", "%ProgramW6432%"),
    ("{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}", "%ProgramFiles(x86)%"),
    ("{905E63B6-C1BF-494E-B29C-65B732D3D21A}", "%ProgramFiles%"),
    ("{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}", r"%SystemRoot%\System32"),
    ("{D65231B0-B2F1-4857-A4CE-A8E7C6EA7D27}", r"%SystemRoot%\SysWOW64"),
    ("{F38BF404-1D43-42F2-9305-67DE0B28FC23}", "%SystemRoot%"),
    ("{0139D44E-6AFE-49F2-8690-3DAFCAE6FFB8}", r"%ProgramData%\Microsoft\Windows\Start Menu\Programs"),
    ("{A77F5D77-2E2B-44C3-A6A2-ABA601054A51}", r"%APPDATA%\Microsoft\Windows\Start Menu\Programs"),
    ("{9E3995AB-1F9C-4F13-B827-48B24B6C7174}", r"%APPDATA%\Microsoft\Internet Explorer\Quick Launch\User Pinned"),
    ("{F1B32785-6FBA-4FCF-9D55-7B8E7F157091}", "%LOCALAPPDATA%"),
    ("{3EB685DB-65F9-4CF6-A03A-E3EF65729F3D}", "%APPDATA%"),
    ("{374DE290-123F-4565-9164-39C4925E467B}", r"%USERPROFILE%\Downloads"),
];

/// Thay GUID known folder ở đầu chuỗi bằng đường dẫn thật.
pub fn expand_known_folder(decoded: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    for (guid, var) in KNOWN_FOLDERS {
        // `get` thay vì `[..]`: đường dẫn có ký tự nhiều byte (tiếng Việt…) có thể khiến vị trí
        // cắt rơi giữa một ký tự — `[..]` sẽ panic (từng làm app tắt khi mở trang Ứng dụng).
        if let Some(rest) = decoded.get(..guid.len()).filter(|p| p.eq_ignore_ascii_case(guid)).and_then(|_| decoded.get(guid.len()..)) {
            let base = crate::startup::command::expand_env(var, env);
            return format!("{base}{rest}");
        }
    }
    decoded.to_owned()
}

/// Đọc toàn bộ UserAssist của người dùng hiện tại.
pub fn read() -> Vec<Entry> {
    let Ok(root) = windows_registry::CURRENT_USER.open(KEY) else { return Vec::new() };
    let Ok(guids) = root.keys() else { return Vec::new() };
    let env = |k: &str| std::env::var(k).ok();
    let mut out = Vec::new();
    for g in guids {
        let Ok(count) = root.open(format!(r"{g}\Count")) else { continue };
        let Ok(values) = count.values() else { continue };
        for (name, data) in values {
            let Some((run_count, last_run_ms)) = parse_data(&data) else { continue };
            let target = expand_known_folder(&rot13(&name), &env);
            out.push(Entry { target, run_count, last_run_ms });
        }
    }
    out
}

/// Đích là đường dẫn file (không phải AppUserModelID).
pub fn as_path(e: &Entry) -> Option<PathBuf> {
    (e.target.len() > 3 && e.target.as_bytes()[1] == b':').then(|| PathBuf::from(&e.target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rot13_roundtrip_and_non_letters() {
        assert_eq!(rot13("Hello, World! 123"), "Uryyb, Jbeyq! 123");
        assert_eq!(rot13(&rot13("C:\\Program Files\\Ứng dụng\\a.exe")), "C:\\Program Files\\Ứng dụng\\a.exe");
        // Giá trị thật lấy từ registry: Microsoft.Windows.Explorer
        assert_eq!(rot13("Zvpebfbsg.Jvaqbjf.Rkcybere"), "Microsoft.Windows.Explorer");
    }

    #[test]
    fn known_folder_guid_is_expanded() {
        let env = |k: &str| match k {
            "ProgramW6432" => Some(r"C:\Program Files".to_owned()),
            "SystemRoot" => Some(r"C:\Windows".to_owned()),
            _ => None,
        };
        let enc = rot13(r"{6D809377-6AF0-444B-8957-A3773F02200E}\7-Zip\7zFM.exe");
        assert_eq!(expand_known_folder(&rot13(&enc), &env), r"C:\Program Files\7-Zip\7zFM.exe");
        // GUID không phân biệt hoa thường.
        assert_eq!(
            expand_known_folder(r"{1ac14e77-02e7-4e5d-b744-2eb1ae5198b7}\notepad.exe", &env),
            r"C:\Windows\System32\notepad.exe"
        );
        assert_eq!(expand_known_folder("Microsoft.Windows.Explorer", &env), "Microsoft.Windows.Explorer");
    }

    /// Lỗi thật: đường dẫn có ký tự nhiều byte (tiếng Việt…) vắt qua byte thứ 38 (độ dài GUID)
    /// từng làm `decoded[..38]` panic => app (v1.0.0 dùng panic = "abort") tắt ngay khi mở trang Ứng dụng.
    #[test]
    fn non_ascii_path_across_guid_length_does_not_panic() {
        let env = |_: &str| None;
        for prefix_len in 30..45 {
            let path = format!(r"C:\{}\Nguyễn Văn Ứng dụng\app.exe", "x".repeat(prefix_len));
            assert_eq!(expand_known_folder(&path, &env), path);
        }
        // Đúng ca hiểm: 37 byte ASCII rồi một ký tự 3 byte => byte 38 nằm giữa ký tự.
        let tricky = format!("{}ễ\\app.exe", "x".repeat(37));
        assert_eq!(expand_known_folder(&tricky, &env), tricky);
        // Chuỗi ngắn hơn GUID có dấu.
        assert_eq!(expand_known_folder("Ứng dụng", &env), "Ứng dụng");
    }

    #[test]
    fn parses_72_byte_record() {
        let mut b = [0u8; 72];
        b[4..8].copy_from_slice(&7u32.to_le_bytes());
        // 2024-01-01T00:00:00Z dưới dạng FILETIME.
        let ft: u64 = 116_444_736_000_000_000 + 1_704_067_200_000 * 10_000;
        b[60..68].copy_from_slice(&ft.to_le_bytes());
        assert_eq!(parse_data(&b), Some((7, Some(1_704_067_200_000))));
        assert_eq!(parse_data(&[0u8; 72]), Some((0, None)));
        assert_eq!(parse_data(&[0u8; 16]), None, "định dạng XP");
    }

    #[test]
    fn reads_real_registry_without_error() {
        let entries = read();
        // Máy đã dùng Explorer thì gần như chắc chắn có entry.
        assert!(entries.iter().all(|e| !e.target.is_empty()));
    }
}
