//! Ứng dụng đã cài + lần dùng cuối + gỡ cài đặt.
//!
//! Lần dùng cuối = max(UserAssist, Prefetch, mẫu của PC Manager) theo
//! CLAUDE.md. Mỗi nguồn đều có điểm mù nên UI luôn cho biết số liệu lấy từ đâu.

pub mod installed;
pub mod leftovers;
pub mod prefetch;
pub mod sampler;
pub mod userassist;

use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rayon::prelude::*;
use serde::Serialize;

use crate::cleaner::safety::is_link_or_cloud;
use crate::error::{AppError, AppResult};
use crate::store::Store;
use crate::sys::com::ComGuard;
use crate::sys::shell;
use installed::Installed;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageSource {
    UserAssist,
    Prefetch,
    PcManager,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub install_date_ms: Option<i64>,
    pub size_bytes: Option<u64>,
    /// true = đo thực tế thư mục cài; false = số app tự khai báo trong registry.
    pub size_measured: bool,
    pub install_location: Option<String>,
    pub last_used_ms: Option<i64>,
    pub last_used_source: Option<UsageSource>,
    pub can_uninstall: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsOverview {
    pub apps: Vec<AppInfo>,
    pub userassist_entries: usize,
    /// None = không đọc được Prefetch (thiếu quyền admin hoặc bị tắt).
    pub prefetch_entries: Option<usize>,
    /// PC Manager bắt đầu lấy mẫu từ lúc này.
    pub tracking_since_ms: Option<i64>,
}

/// Tên exe chung chung (trình cài/gỡ/cập nhật) — không dùng để suy ra app đã được dùng.
fn is_generic_exe(name: &str) -> bool {
    const WORDS: &[&str] = &["unins", "uninst", "setup", "install", "update", "crash", "elevat", "helper", "squirrel", "maintenance"];
    let n = name.to_lowercase();
    WORDS.iter().any(|w| n.contains(w))
}

/// Thư mục cài đủ "hẹp" để so tiền tố đường dẫn (không phải C:\, Program Files, Windows…).
fn usable_location(loc: &Path) -> bool {
    let normals = loc.components().filter(|c| matches!(c, std::path::Component::Normal(_))).count();
    let lower = loc.display().to_string().to_lowercase();
    let windir = std::env::var("SystemRoot").unwrap_or_default().to_lowercase();
    normals >= 2 && !(!windir.is_empty() && lower.starts_with(&windir)) && loc.is_dir()
}

/// File exe thuộc app: DisplayIcon + exe ở thư mục cài (sâu tối đa 2 cấp, tối đa 300 file).
fn app_exes(app: &Installed) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(icon) = &app.display_icon {
        if icon.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
            out.push(icon.display().to_string().to_lowercase());
        }
    }
    if let Some(loc) = app.install_location.as_deref().filter(|l| usable_location(l)) {
        let mut stack = vec![(loc.to_path_buf(), 0u8)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                if out.len() >= 300 {
                    return out;
                }
                let Ok(m) = e.metadata() else { continue };
                if is_link_or_cloud(&m) {
                    continue;
                }
                let p = e.path();
                if m.is_dir() && depth < 1 {
                    stack.push((p, depth + 1));
                } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")) {
                    out.push(p.display().to_string().to_lowercase());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Đo dung lượng thư mục (không theo link), dừng khi quá hạn → None.
fn dir_size(root: &Path, deadline: Instant) -> Option<u64> {
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if Instant::now() > deadline {
            return None;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(m) = e.metadata() else { continue };
            if is_link_or_cloud(&m) {
                continue;
            }
            if m.is_dir() {
                stack.push(e.path());
            } else {
                total += m.len();
            }
        }
    }
    Some(total)
}

/// Nguồn lần dùng: danh sách (đường dẫn viết thường, ms, nguồn).
struct UsageIndex {
    by_path: Vec<(String, i64, UsageSource)>,
    prefetch: HashMap<String, i64>,
}

impl UsageIndex {
    fn build(store: &Store, ua: &[userassist::Entry], prefetch: Option<HashMap<String, i64>>) -> Self {
        let mut by_path = Vec::new();
        // .lnk trong UserAssist (mở từ Start menu) → giải ra exe đích.
        let com = ComGuard::init().ok();
        for e in ua {
            let (Some(path), Some(ms)) = (userassist::as_path(e), e.last_run_ms) else { continue };
            let is_lnk = path.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk"));
            let target = match (&com, is_lnk) {
                (Some(c), true) => shell::resolve_lnk(c, &path).ok().and_then(|t| t.path),
                (_, true) => None,
                (_, false) => Some(path),
            };
            if let Some(t) = target {
                by_path.push((t.display().to_string().to_lowercase(), ms, UsageSource::UserAssist));
            }
        }
        for (p, ms) in store.usage().unwrap_or_default() {
            by_path.push((p, ms, UsageSource::PcManager));
        }
        Self { by_path, prefetch: prefetch.unwrap_or_default() }
    }

    fn last_used(&self, app: &Installed, exes: &[String]) -> Option<(i64, UsageSource)> {
        let loc = app
            .install_location
            .as_deref()
            .filter(|l| usable_location(l))
            .map(|l| format!("{}\\", l.display().to_string().to_lowercase()));
        let mut best: Option<(i64, UsageSource)> = None;
        let mut consider = |ms: i64, src: UsageSource| {
            if best.is_none_or(|(b, _)| ms > b) {
                best = Some((ms, src));
            }
        };
        for (p, ms, src) in &self.by_path {
            let in_loc = loc.as_deref().is_some_and(|l| p.starts_with(l));
            let named = exes.binary_search(p).is_ok();
            let file = p.rsplit('\\').next().unwrap_or("");
            if (in_loc || named) && !is_generic_exe(file) {
                consider(*ms, *src);
            }
        }
        for exe in exes {
            let file = exe.rsplit('\\').next().unwrap_or("");
            if is_generic_exe(file) {
                continue;
            }
            if let Some(&ms) = self.prefetch.get(file) {
                consider(ms, UsageSource::Prefetch);
            }
        }
        best
    }
}

pub fn list(store: &Store) -> AppResult<AppsOverview> {
    let apps = installed::list();
    let ua = userassist::read();
    let prefetch = prefetch::read();
    let prefetch_entries = prefetch.as_ref().map(HashMap::len);
    let index = UsageIndex::build(store, &ua, prefetch);
    let deadline = Instant::now() + Duration::from_secs(8);

    let out: Vec<AppInfo> = apps
        .par_iter()
        .map(|a| {
            let exes = app_exes(a);
            let used = index.last_used(a, &exes);
            // Ưu tiên số registry; thiếu thì đo thư mục (có giới hạn thời gian).
            let measured = match (a.estimated_bytes, &a.install_location) {
                (None, Some(loc)) if usable_location(loc) => dir_size(loc, deadline),
                _ => None,
            };
            AppInfo {
                id: a.id.clone(),
                name: a.name.clone(),
                publisher: a.publisher.clone(),
                version: a.version.clone(),
                install_date_ms: a.install_date_ms,
                size_bytes: a.estimated_bytes.or(measured),
                size_measured: a.estimated_bytes.is_none() && measured.is_some(),
                install_location: a.install_location.as_ref().map(|p| p.display().to_string()),
                last_used_ms: used.map(|u| u.0),
                last_used_source: used.map(|u| u.1),
                can_uninstall: !a.no_remove && a.uninstall.is_some(),
            }
        })
        .collect();

    Ok(AppsOverview {
        apps: out,
        userassist_entries: ua.len(),
        prefetch_entries,
        tracking_since_ms: store.usage_since().unwrap_or(None),
    })
}

/// Lệnh gỡ cài đặt cần chạy: (chương trình, tham số thô).
#[derive(Debug, PartialEq, Eq)]
pub struct UninstallCommand {
    pub program: PathBuf,
    pub args: String,
}

/// Phân tích UninstallString. MSI `/I{GUID}` (mở trình sửa chữa) được đổi thành
/// `/X{GUID}` (gỡ bỏ) như Programs and Features vẫn làm.
pub fn parse_uninstall(cmd: &str, system32: &Path) -> Option<UninstallCommand> {
    let cmd = cmd.trim();
    let lower = cmd.to_lowercase();
    if lower.starts_with("msiexec") || lower.contains("\\msiexec.exe") {
        let start = cmd.find('{')?;
        let end = start + cmd[start..].find('}')?;
        let guid = &cmd[start..=end];
        let valid = guid.len() == 38 && guid[1..37].chars().all(|c| c.is_ascii_hexdigit() || c == '-');
        return valid.then(|| UninstallCommand { program: system32.join("msiexec.exe"), args: format!("/X{guid}") });
    }
    let (program, rest) = if let Some(s) = cmd.strip_prefix('"') {
        let end = s.find('"')?;
        (&s[..end], &s[end + 1..])
    } else if let Some(i) = lower.find(".exe") {
        (&cmd[..i + 4], &cmd[i + 4..])
    } else {
        cmd.split_once(' ').unwrap_or((cmd, ""))
    };
    let program = PathBuf::from(program.trim());
    // Chỉ chạy đường dẫn tuyệt đối — không để PATH quyết định chương trình nào chạy.
    program.is_absolute().then(|| UninstallCommand { program, args: rest.trim().to_owned() })
}

/// Mở trình gỡ cài đặt của app `id`. Lệnh luôn đọc lại từ registry, không nhận từ UI.
pub fn uninstall(store: &Store, id: &str) -> AppResult<String> {
    let app = installed::find(id).ok_or_else(|| AppError::NotFound("ứng dụng không còn trong danh sách".into()))?;
    let result = (|| {
        if app.no_remove {
            return Err(AppError::Forbidden(format!("{} không cho phép gỡ cài đặt", app.name)));
        }
        let raw = app.uninstall.as_deref().ok_or_else(|| AppError::NotFound("không có lệnh gỡ cài đặt".into()))?;
        let sys32 = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32");
        let cmd = parse_uninstall(raw, &sys32).ok_or_else(|| AppError::InvalidInput(format!("không hiểu lệnh gỡ: {raw}")))?;
        if !cmd.program.is_file() {
            return Err(AppError::NotFound(format!("không tìm thấy {}", cmd.program.display())));
        }
        std::process::Command::new(&cmd.program).raw_arg(&cmd.args).spawn()?;
        Ok(format!("{} {}", cmd.program.display(), cmd.args))
    })();
    let detail = match &result {
        Ok(c) => c.clone(),
        Err(e) => e.to_string(),
    };
    store.log_action_best_effort("app.uninstall", &app.name, result.is_ok(), &detail);
    result.map(|_| app.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys32() -> PathBuf {
        PathBuf::from(r"C:\Windows\System32")
    }

    #[test]
    fn msi_install_switch_becomes_uninstall() {
        let c = parse_uninstall("MsiExec.exe /I{23170F69-40C1-2702-2301-000001000000}", &sys32()).unwrap();
        assert_eq!(c.program, PathBuf::from(r"C:\Windows\System32\msiexec.exe"));
        assert_eq!(c.args, "/X{23170F69-40C1-2702-2301-000001000000}");
        assert!(parse_uninstall("MsiExec.exe /I{not-a-guid}", &sys32()).is_none());
    }

    #[test]
    fn quoted_and_unquoted_programs() {
        assert_eq!(
            parse_uninstall(r#""C:\Program Files\7-Zip\Uninstall.exe" /S"#, &sys32()),
            Some(UninstallCommand { program: r"C:\Program Files\7-Zip\Uninstall.exe".into(), args: "/S".into() })
        );
        assert_eq!(
            parse_uninstall(r"C:\Program Files\App X\unins000.exe /SILENT", &sys32()),
            Some(UninstallCommand { program: r"C:\Program Files\App X\unins000.exe".into(), args: "/SILENT".into() })
        );
    }

    #[test]
    fn relative_program_is_refused() {
        // Không cho PATH quyết định chạy chương trình nào.
        assert!(parse_uninstall("uninstall.exe /x", &sys32()).is_none());
        assert!(parse_uninstall("", &sys32()).is_none());
    }

    #[test]
    fn generic_exe_names() {
        assert!(is_generic_exe("unins000.exe"));
        assert!(is_generic_exe("Update.exe"));
        assert!(is_generic_exe("setup.exe"));
        assert!(!is_generic_exe("code.exe"));
    }

    #[test]
    fn usage_matching_by_location_and_prefetch() {
        let dir = std::env::temp_dir().join(format!("pcm-usage-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin").join("myapp.exe"), b"x").unwrap();
        std::fs::write(dir.join("unins000.exe"), b"x").unwrap();
        let app = Installed {
            id: "HKCU\\x".into(),
            name: "My App".into(),
            publisher: None,
            version: None,
            install_date_ms: None,
            estimated_bytes: None,
            install_location: Some(dir.clone()),
            display_icon: None,
            uninstall: None,
            no_remove: false,
        };
        let exes = app_exes(&app);
        assert_eq!(exes.len(), 2);
        let loc = dir.display().to_string().to_lowercase();
        let index = UsageIndex {
            by_path: vec![
                (format!(r"{loc}\bin\myapp.exe"), 1000, UsageSource::UserAssist),
                (format!(r"{loc}\unins000.exe"), 9000, UsageSource::PcManager), // trình gỡ: bỏ qua
                (r"c:\khac\other.exe".into(), 8000, UsageSource::PcManager),
            ],
            prefetch: HashMap::from([("myapp.exe".into(), 2000), ("unins000.exe".into(), 9999)]),
        };
        assert_eq!(index.last_used(&app, &exes), Some((2000, UsageSource::Prefetch)));
        assert_eq!(dir_size(&dir, Instant::now() + Duration::from_secs(5)), Some(2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Chẩn đoán: `cargo test print_apps -- --ignored --nocapture`.
    #[test]
    #[ignore = "chỉ để xem dữ liệu thật"]
    fn print_apps() {
        let store = Store::in_memory().unwrap();
        let o = list(&store).unwrap();
        println!("UserAssist={} Prefetch={:?}", o.userassist_entries, o.prefetch_entries);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
        for a in o.apps.iter().take(25) {
            let days = a.last_used_ms.map(|ms| (now - ms) / 86_400_000);
            println!(
                "{:<40} size={:>8}MB last={:?}d src={:?}",
                a.name.chars().take(40).collect::<String>(),
                a.size_bytes.unwrap_or(0) / 1_048_576,
                days,
                a.last_used_source
            );
        }
    }

    #[test]
    fn real_list_runs() {
        let store = Store::in_memory().unwrap();
        let o = list(&store).unwrap();
        assert!(o.apps.iter().all(|a| !a.name.is_empty()));
    }
}
