//! Prefetch: Windows tạo `C:\Windows\Prefetch\APP.EXE-HASH.pf` mỗi lần chương
//! trình chạy; ngày sửa của file ≈ lần chạy cuối. Chỉ ĐỌC (Prefetch nằm trong
//! danh sách cấm xoá). Cần quyền admin để liệt kê thư mục.

use std::collections::HashMap;
use std::time::UNIX_EPOCH;

/// "CHROME.EXE-A1B2C3D4.pf" → "chrome.exe".
pub fn exe_from_pf(file: &str) -> Option<String> {
    let stem = file.strip_suffix(".pf").or_else(|| file.strip_suffix(".PF"))?;
    let (exe, hash) = stem.rsplit_once('-')?;
    (hash.len() == 8 && hash.chars().all(|c| c.is_ascii_hexdigit()) && !exe.is_empty()).then(|| exe.to_lowercase())
}

/// exe (viết thường) → lần chạy cuối (unix ms). `None` nếu không đọc được thư mục.
pub fn read() -> Option<HashMap<String, i64>> {
    let dir = std::path::Path::new(&std::env::var("SystemRoot").ok()?).join("Prefetch");
    let rd = std::fs::read_dir(dir).ok()?;
    let mut out: HashMap<String, i64> = HashMap::new();
    for e in rd.flatten() {
        let Some(exe) = exe_from_pf(&e.file_name().to_string_lossy()) else { continue };
        let Some(ms) = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
        else {
            continue;
        };
        let slot = out.entry(exe).or_insert(ms);
        *slot = (*slot).max(ms);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pf_names() {
        assert_eq!(exe_from_pf("CHROME.EXE-5349D0F2.pf").as_deref(), Some("chrome.exe"));
        assert_eq!(exe_from_pf("MY-APP.EXE-0A1B2C3D.pf").as_deref(), Some("my-app.exe"));
        assert_eq!(exe_from_pf("Layout.ini"), None);
        assert_eq!(exe_from_pf("X.EXE-ZZZ.pf"), None);
        assert_eq!(exe_from_pf("-12345678.pf"), None);
    }
}
