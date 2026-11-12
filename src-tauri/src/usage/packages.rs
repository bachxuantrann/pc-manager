//! App Store (MSIX/UWP) đã cài cho người dùng đang đăng nhập — chỉ xem; gỡ qua Windows Settings.
//!
//! Đọc `HKCU\Software\Classes\Local Settings\…\AppModel\Repository\Packages` (mỗi gói một key, có
//! `PackageRootFolder`) và `AppxManifest.xml` của gói. Không dùng WinRT `PackageManager` để tiến trình
//! không phải nạp thêm WinRT (giữ RAM lúc rảnh như cũ).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use windows_registry::Key;

use super::installed::Installed;
use crate::startup::packaged::{resolve_name, APP_MODEL};

/// Tiền tố id của app Store (phân biệt với key `Uninstall`).
pub const ID_PREFIX: &str = r"MSIX\";

/// Nội dung thẻ `<tag>…</tag>` đầu tiên.
fn element<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml.get(start..)?.find(&format!("</{tag}>"))?;
    xml.get(start..end).map(str::trim)
}

/// Gói người dùng thấy như một app: không phải framework / gói tài nguyên, có ít nhất một Application.
pub fn is_user_app(manifest: &str) -> bool {
    element(manifest, "Framework") != Some("true")
        && element(manifest, "ResourcePackage") != Some("true")
        && manifest.contains("<Application ")
}

/// `Name_Version_Arch_ResourceId_PublisherId` → (Name, Version, family `Name_PublisherId`).
pub fn split_full_name(full: &str) -> Option<(&str, &str, String)> {
    let mut parts = full.split('_');
    let name = parts.next().filter(|n| !n.is_empty())?;
    let version = parts.next()?;
    let publisher_id = full.rsplit('_').next().filter(|p| !p.is_empty())?;
    Some((name, version, format!("{name}_{publisher_id}")))
}

fn created_ms(p: &Path) -> Option<i64> {
    let t = std::fs::metadata(p).ok()?.created().ok()?;
    i64::try_from(t.duration_since(UNIX_EPOCH).ok()?.as_millis()).ok()
}

/// App Store của người dùng đang đăng nhập (mỗi family một mục — bản mới nhất còn thư mục).
pub fn list() -> Vec<Installed> {
    let Ok(hkcu) = crate::user::hkcu() else { return Vec::new() };
    list_from(&hkcu)
}

fn list_from(hkcu: &Key) -> Vec<Installed> {
    let Ok(repo) = hkcu.open(format!(r"{APP_MODEL}\Repository\Packages")) else { return Vec::new() };
    let Ok(keys) = repo.keys() else { return Vec::new() };
    let windir = std::env::var("SystemRoot").unwrap_or_default().to_lowercase();
    // family → (full name, root) của bản mới nhất còn thư mục.
    let mut latest: HashMap<String, (String, PathBuf, Option<String>)> = HashMap::new();
    for full in keys {
        let Some((_, _, family)) = split_full_name(&full) else { continue };
        let Ok(k) = repo.open(&full) else { continue };
        let Ok(root) = k.get_string("PackageRootFolder") else { continue };
        let root = PathBuf::from(root);
        // App hệ thống (C:\Windows\SystemApps…) gỡ không được — bỏ.
        if !windir.is_empty() && root.display().to_string().to_lowercase().starts_with(&windir) {
            continue;
        }
        if !root.is_dir() {
            continue;
        }
        let display = k.get_string("DisplayName").ok();
        match latest.get(&family) {
            Some((cur, _, _)) if *cur >= full => {}
            _ => {
                latest.insert(family, (full, root, display));
            }
        }
    }
    latest
        .into_values()
        .filter_map(|(full, root, display)| {
            let manifest = std::fs::read_to_string(root.join("AppxManifest.xml")).ok()?;
            if !is_user_app(&manifest) {
                return None;
            }
            let (name, version, _) = split_full_name(&full)?;
            let resolve = |raw: &str| resolve_name(raw, &full, name);
            let title = display
                .as_deref()
                .and_then(resolve)
                .or_else(|| element(&manifest, "DisplayName").and_then(resolve))
                .unwrap_or_else(|| name.to_owned());
            Some(Installed {
                id: format!("{ID_PREFIX}{full}"),
                name: title,
                publisher: element(&manifest, "PublisherDisplayName").and_then(resolve),
                version: Some(version.to_owned()),
                install_date_ms: created_ms(&root),
                estimated_bytes: None,
                install_location: Some(root),
                display_icon: None,
                uninstall: None,
                no_remove: true,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names_and_kinds() {
        assert_eq!(
            split_full_name("MSTeams_26288.2004.5269.3420_x64__8wekyb3d8bbwe"),
            Some(("MSTeams", "26288.2004.5269.3420", "MSTeams_8wekyb3d8bbwe".to_owned()))
        );
        assert_eq!(split_full_name("bad"), None);
        let app = "<Package><Properties><DisplayName>X</DisplayName></Properties><Applications><Application Id=\"A\"/></Applications></Package>";
        let framework = "<Package><Properties><Framework>true</Framework></Properties></Package>";
        let resources = "<Package><Properties><ResourcePackage>true</ResourcePackage></Properties></Package>";
        assert!(is_user_app(app));
        assert!(!is_user_app(framework));
        assert!(!is_user_app(resources));
        assert_eq!(element(app, "DisplayName"), Some("X"));
    }

    #[test]
    fn real_list_has_readable_names() {
        for a in list() {
            assert!(a.id.starts_with(ID_PREFIX) && !a.name.is_empty() && !a.name.starts_with("ms-resource"), "{a:?}");
        }
    }
}
