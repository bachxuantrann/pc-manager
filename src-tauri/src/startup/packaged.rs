//! Startup task của app Store (MSIX/UWP) — Task Manager hiện chúng ở tab Startup. Chỉ đọc (v1).
//!
//! - Trạng thái: `HKCU\Software\Classes\Local Settings\…\AppModel\SystemAppData\<PackageFamily>\<TaskId>`,
//!   giá trị `State` (0 tắt, 1 người dùng tắt, 2 bật, 3 chính sách tắt, 4 chính sách bật).
//! - Tên + file chạy: khai báo `StartupTask` trong `AppxManifest.xml` của gói (thư mục gói lấy từ
//!   `…\AppModel\Repository\Packages\<PackageFullName>\PackageRootFolder`).

use std::path::PathBuf;

use windows_registry::Key;

pub(crate) const APP_MODEL: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel";

/// Một startup task của gói.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagedTask {
    pub family: String,
    pub task_id: String,
    pub enabled: bool,
    /// Tên hiển thị (của task, hoặc của gói).
    pub name: String,
    /// File chạy khi đăng nhập (nếu đọc được manifest).
    pub exe: Option<PathBuf>,
    /// Nơi khai báo trạng thái (để hiện ở chi tiết).
    pub location: String,
}

/// State 2 / 4 = bật (người dùng / chính sách).
pub fn state_enabled(state: u32) -> bool {
    matches!(state, 2 | 4)
}

/// (family, task_id, state) của mọi startup task đã đăng ký — chỉ registry, rất nhẹ.
pub fn states(hkcu: &Key) -> Vec<(String, String, u32)> {
    let Ok(root) = hkcu.open(format!(r"{APP_MODEL}\SystemAppData")) else { return Vec::new() };
    let Ok(families) = root.keys() else { return Vec::new() };
    let mut out = Vec::new();
    for family in families {
        let Ok(fk) = root.open(&family) else { continue };
        let Ok(tasks) = fk.keys() else { continue };
        for task in tasks {
            if let Ok(state) = fk.open(&task).and_then(|t| t.get_u32("State")) {
                out.push((family.clone(), task, state));
            }
        }
    }
    out
}

/// Gói đã cài thuộc `family` (`Name_PublisherId`): (PackageFullName, thư mục gói, DisplayName thô).
/// Có thể còn key của bản cũ — lấy bản có thư mục còn tồn tại, tên lớn nhất (thường là bản mới nhất).
fn package_of(hkcu: &Key, family: &str) -> Option<(String, PathBuf, Option<String>)> {
    let (name, publisher_id) = family.rsplit_once('_')?;
    let repo = hkcu.open(format!(r"{APP_MODEL}\Repository\Packages")).ok()?;
    let mut found: Vec<(String, PathBuf, Option<String>)> = repo
        .keys()
        .ok()?
        .filter(|full| full_name_matches(full, name, publisher_id))
        .filter_map(|full| {
            let k = repo.open(&full).ok()?;
            let root = PathBuf::from(k.get_string("PackageRootFolder").ok()?);
            root.is_dir().then(|| (full, root, k.get_string("DisplayName").ok()))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.pop()
}

/// `Name_Version_Arch_ResourceId_PublisherId` thuộc family `Name_PublisherId`.
fn full_name_matches(full: &str, name: &str, publisher_id: &str) -> bool {
    full.strip_prefix(name).is_some_and(|r| r.starts_with('_')) && full.rsplit('_').next() == Some(publisher_id)
}

/// Giá trị thuộc tính `attr="…"` trong một thẻ XML (chuỗi thẻ đã cắt sẵn).
pub(crate) fn attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let pat = format!(" {attr}=\"");
    let start = tag.find(&pat)? + pat.len();
    let end = start + tag.get(start..)?.find('"')?;
    tag.get(start..end)
}

/// Từ manifest: (DisplayName, Executable) của `StartupTask` có `TaskId = task_id`.
/// `Executable` nằm ở thẻ `Extension Category="windows.startupTask"` bao ngoài.
pub fn manifest_task(xml: &str, task_id: &str) -> Option<(Option<String>, Option<String>)> {
    let needle = format!("TaskId=\"{task_id}\"");
    let pos = xml.find(&needle)?;
    let tag_start = xml.get(..pos)?.rfind('<')?;
    let tag_end = pos + xml.get(pos..)?.find('>')?;
    let tag = xml.get(tag_start..tag_end)?;
    let display = attr(tag, "DisplayName").map(str::to_owned);
    let ext_start = xml.get(..tag_start)?.rfind("windows.startupTask")?;
    let ext_tag_start = xml.get(..ext_start)?.rfind('<')?;
    let ext_tag_end = ext_start + xml.get(ext_start..)?.find('>')?;
    let exe = xml.get(ext_tag_start..ext_tag_end).and_then(|t| attr(t, "Executable")).map(str::to_owned);
    Some((display, exe))
}

/// Tên đọc được: chuỗi thường giữ nguyên; `ms-resource:` / `@{…}` giải qua shell; không giải được => None.
pub(crate) fn resolve_name(raw: &str, full_name: &str, package_name: &str) -> Option<String> {
    if let Some(res) = raw.strip_prefix("ms-resource:") {
        let uri = if res.starts_with("//") {
            format!("ms-resource:{res}")
        } else if res.contains('/') {
            format!("ms-resource://{package_name}/{res}")
        } else {
            format!("ms-resource://{package_name}/Resources/{res}")
        };
        return crate::sys::shell::load_indirect_string(&format!("@{{{full_name}?{uri}}}"));
    }
    if raw.starts_with('@') {
        return crate::sys::shell::load_indirect_string(raw);
    }
    (!raw.trim().is_empty()).then(|| raw.to_owned())
}

/// Mọi startup task của app Store kèm tên và file chạy (đọc manifest).
pub fn list(hkcu: &Key, location_prefix: &str) -> Vec<PackagedTask> {
    states(hkcu)
        .into_iter()
        .map(|(family, task_id, state)| {
            let package_name = family.rsplit_once('_').map_or(family.as_str(), |(n, _)| n).to_owned();
            let mut name = None;
            let mut exe = None;
            if let Some((full, root, pkg_display)) = package_of(hkcu, &family) {
                let manifest = std::fs::read_to_string(root.join("AppxManifest.xml")).unwrap_or_default();
                if let Some((display, executable)) = manifest_task(&manifest, &task_id) {
                    name = display.and_then(|d| resolve_name(&d, &full, &package_name));
                    exe = executable.map(|e| root.join(e));
                }
                if name.is_none() {
                    name = pkg_display.and_then(|d| resolve_name(&d, &full, &package_name));
                }
            }
            PackagedTask {
                location: format!(r"{location_prefix}\{APP_MODEL}\SystemAppData\{family}\{task_id}"),
                name: name.unwrap_or_else(|| package_name.clone()),
                enabled: state_enabled(state),
                family,
                task_id,
                exe,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"<Package><Applications><Application Id="App">
      <Extensions>
        <desktop:Extension Category="windows.startupTask" Executable="app\autostart.exe" EntryPoint="Windows.FullTrustApplication">
          <desktop:StartupTask TaskId="MyStartup" Enabled="true" DisplayName="My App" />
        </desktop:Extension>
        <uap5:Extension Category="windows.startupTask" Executable="other.exe" EntryPoint="X">
          <uap5:StartupTask TaskId="Second" Enabled="false" DisplayName="ms-resource:TaskName" />
        </uap5:Extension>
      </Extensions></Application></Applications></Package>"#;

    #[test]
    fn reads_startup_task_from_manifest() {
        assert_eq!(manifest_task(MANIFEST, "MyStartup"), Some((Some("My App".into()), Some(r"app\autostart.exe".into()))));
        assert_eq!(manifest_task(MANIFEST, "Second"), Some((Some("ms-resource:TaskName".into()), Some("other.exe".into()))));
        assert_eq!(manifest_task(MANIFEST, "Missing"), None);
    }

    #[test]
    fn state_and_package_name_rules() {
        assert!(state_enabled(2) && state_enabled(4));
        assert!(!state_enabled(0) && !state_enabled(1) && !state_enabled(3));
        assert!(full_name_matches("MSTeams_26288.2004.5269.3420_x64__8wekyb3d8bbwe", "MSTeams", "8wekyb3d8bbwe"));
        assert!(!full_name_matches("MSTeamsX_1.0_x64__8wekyb3d8bbwe", "MSTeams", "8wekyb3d8bbwe"));
        assert!(!full_name_matches("MSTeams_1.0_x64__other", "MSTeams", "8wekyb3d8bbwe"));
    }

    #[test]
    fn real_list_does_not_fail() {
        let hkcu = windows_registry::CURRENT_USER.open("").unwrap();
        for t in list(&hkcu, "HKCU") {
            assert!(!t.name.is_empty(), "{t:?}");
        }
    }
}
