//! Ứng dụng đã cài, đọc từ các key `Uninstall` (HKLM, HKLM 32-bit, HKCU).

use std::collections::HashSet;
use std::path::PathBuf;

use windows_registry::{Key, LOCAL_MACHINE};

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

#[derive(Debug, Clone)]
pub struct Installed {
    /// `HKLM\<subkey>` / `HKLM32\<subkey>` / `HKCU\<subkey>` — định danh để gỡ cài.
    pub id: String,
    pub name: String,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub install_date_ms: Option<i64>,
    pub estimated_bytes: Option<u64>,
    pub install_location: Option<PathBuf>,
    pub display_icon: Option<PathBuf>,
    pub uninstall: Option<String>,
    pub no_remove: bool,
}

/// Gốc registry của một nguồn. `HKCU` là của user đang đăng nhập (`HKU<SID>` khi app được nâng
/// quyền bằng tài khoản admin khác), không phải của tài khoản admin.
fn root_of(tag: &str) -> Option<Key> {
    match tag {
        "HKCU" => crate::user::hkcu().ok(),
        _ => LOCAL_MACHINE.open("").ok(),
    }
}

const SOURCES: [(&str, &str); 3] = [(UNINSTALL, "HKLM"), (UNINSTALL32, "HKLM32"), (UNINSTALL, "HKCU")];

/// "20240115" → unix ms (00:00 UTC). Ngày không hợp lệ → None.
pub fn parse_install_date(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let y: i64 = s[0..4].parse().ok()?;
    let m: i64 = s[4..6].parse().ok()?;
    let d: i64 = s[6..8].parse().ok()?;
    if !(1990..=2100).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // days_from_civil (Howard Hinnant) — lịch Gregory, không cần crate ngày tháng.
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400_000)
}

/// `"C:\App\app.exe",0` → `C:\App\app.exe`.
pub fn parse_icon_path(s: &str) -> Option<PathBuf> {
    let s = s.trim();
    let s = match s.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => s.rsplit_once(',').filter(|(_, idx)| idx.trim().parse::<i32>().is_ok()).map_or(s, |(p, _)| p),
    };
    (!s.is_empty()).then(|| PathBuf::from(s))
}

fn read_key(root: &Key, base: &str, tag: &str, sub: &str) -> Option<Installed> {
    let k = root.open(format!(r"{base}\{sub}")).ok()?;
    let name = k.get_string("DisplayName").ok()?.trim().to_owned();
    if name.is_empty() {
        return None;
    }
    // Bản cập nhật / thành phần hệ thống: Programs and Features cũng ẩn.
    if k.get_u32("SystemComponent").unwrap_or(0) == 1 || k.get_string("ParentKeyName").is_ok() {
        return None;
    }
    if k.get_string("ReleaseType").is_ok_and(|t| t.contains("Update") || t.contains("Hotfix")) {
        return None;
    }
    let s = |n: &str| k.get_string(n).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    Some(Installed {
        id: format!(r"{tag}\{sub}"),
        publisher: s("Publisher"),
        version: s("DisplayVersion"),
        install_date_ms: s("InstallDate").and_then(|d| parse_install_date(&d)),
        estimated_bytes: k.get_u32("EstimatedSize").ok().filter(|&kb| kb > 0).map(|kb| u64::from(kb) * 1024),
        install_location: s("InstallLocation").map(|p| PathBuf::from(p.trim_matches('"').trim_end_matches('\\'))),
        display_icon: s("DisplayIcon").and_then(|p| parse_icon_path(&p)),
        uninstall: s("UninstallString"),
        no_remove: k.get_u32("NoRemove").unwrap_or(0) == 1,
        name,
    })
}

pub fn list() -> Vec<Installed> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (base, tag) in SOURCES {
        let Some(root) = root_of(tag) else { continue };
        let Ok(key) = root.open(base) else { continue };
        let Ok(subs) = key.keys() else { continue };
        for sub in subs {
            if let Some(app) = read_key(&root, base, tag, &sub) {
                // Cùng tên + phiên bản xuất hiện ở 2 view registry => giữ một.
                if seen.insert((app.name.to_lowercase(), app.version.clone())) {
                    out.push(app);
                }
            }
        }
    }
    out
}

/// Vị trí key Uninstall của app `id`: (gốc, đường dẫn con dưới gốc, tên đầy đủ cho reg.exe).
pub fn registry_location(id: &str) -> Option<(Key, String, String)> {
    let (tag, sub) = id.split_once('\\')?;
    if sub.is_empty() || sub.contains('\\') {
        return None;
    }
    let (base, tag) = SOURCES.into_iter().find(|(_, t)| *t == tag)?;
    let root = root_of(tag)?;
    let hive = if tag == "HKCU" { crate::user::hkcu_reg_root() } else { "HKLM".to_owned() };
    let path = format!(r"{base}\{sub}");
    Some((root, path.clone(), format!(r"{hive}\{path}")))
}

/// Tìm lại một app theo id (đọc trực tiếp registry, không tin dữ liệu từ UI).
pub fn find(id: &str) -> Option<Installed> {
    let (tag, sub) = id.split_once('\\')?;
    if sub.is_empty() || sub.contains('\\') {
        return None;
    }
    let (base, tag) = SOURCES.into_iter().find(|(_, t)| *t == tag)?;
    read_key(&root_of(tag)?, base, tag, sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_date() {
        assert_eq!(parse_install_date("19700101"), None, "ngoài khoảng hợp lý");
        assert_eq!(parse_install_date("20240101"), Some(1_704_067_200_000));
        assert_eq!(parse_install_date("20240229"), Some(1_709_164_800_000));
        assert_eq!(parse_install_date("2024-01-01"), None);
        assert_eq!(parse_install_date("20241301"), None);
        assert_eq!(parse_install_date(""), None);
    }

    #[test]
    fn icon_paths() {
        assert_eq!(parse_icon_path(r#""C:\A b\app.exe",0"#), Some(PathBuf::from(r"C:\A b\app.exe")));
        assert_eq!(parse_icon_path(r"C:\App\app.exe,-101"), Some(PathBuf::from(r"C:\App\app.exe")));
        assert_eq!(parse_icon_path(r"C:\App\app.ico"), Some(PathBuf::from(r"C:\App\app.ico")));
        // Dấu phẩy là một phần tên thư mục, không phải chỉ số icon.
        assert_eq!(parse_icon_path(r"C:\A,B\app.exe"), Some(PathBuf::from(r"C:\A,B\app.exe")));
        assert_eq!(parse_icon_path("  "), None);
    }

    #[test]
    fn real_list_and_find_roundtrip() {
        let apps = list();
        if let Some(a) = apps.first() {
            let again = find(&a.id).unwrap();
            assert_eq!(again.name, a.name);
        }
        assert!(find(r"HKLM\..\..\Run").is_none());
        assert!(find(r"HKXX\abc").is_none());
        let (_, p, full) = registry_location(r"HKLM32\Foo").unwrap();
        assert_eq!(p, format!(r"{UNINSTALL32}\Foo"));
        assert_eq!(full, format!(r"HKLM\{UNINSTALL32}\Foo"));
        assert!(registry_location(r"HKCU\a\b").is_none());
    }
}
