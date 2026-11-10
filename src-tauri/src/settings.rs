//! Cài đặt chung: danh sách bảo vệ (whitelist), lịch dọn tự động, khởi động cùng Windows.

use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::cleaner::safety::norm;
use crate::error::{AppError, AppResult};
use crate::store::Store;

// ---- Danh sách bảo vệ ---------------------------------------------------------------

const KEY_WHITELIST: &str = "whitelist";
const MAX_WHITELIST: usize = 100;

/// Bản đã chuẩn hoá (`norm`) — đọc ở mọi chỗ kiểm tra trước khi xoá.
static WHITELIST: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// Đường dẫn (đã `norm`) nằm trong một thư mục người dùng bảo vệ.
/// Cleaner, Disk và Dọn phần còn sót đều không bao giờ xoá những đường dẫn này.
pub fn is_protected(normed: &str) -> bool {
    let list = WHITELIST.read().unwrap_or_else(|e| e.into_inner());
    list.iter().any(|w| normed == w || normed.strip_prefix(w.as_str()).is_some_and(|r| r.starts_with('\\')))
}

fn validate_whitelist_path(p: &str) -> Result<String, String> {
    let p = p.trim().trim_matches('"').trim_end_matches(['\\', '/']);
    let path = Path::new(p);
    let disk = matches!(path.components().next(), Some(Component::Prefix(x)) if matches!(x.kind(), Prefix::Disk(_)));
    if p.is_empty() || p.len() > 260 || !disk || !path.is_absolute() {
        return Err(format!("'{p}' không phải đường dẫn tuyệt đối trên ổ đĩa"));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return Err(format!("'{p}' chứa '.' hoặc '..'"));
    }
    Ok(p.to_owned())
}

pub fn load_whitelist(store: &Store) -> Vec<String> {
    store
        .get_setting(KEY_WHITELIST)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
}

/// Nạp whitelist từ DB vào bộ nhớ (lúc khởi động).
pub fn init(store: &Store) {
    apply_whitelist(&load_whitelist(store));
}

fn apply_whitelist(list: &[String]) {
    let mut normed: Vec<String> = list.iter().map(|p| norm(p)).collect();
    normed.sort();
    normed.dedup();
    *WHITELIST.write().unwrap_or_else(|e| e.into_inner()) = normed;
}

/// Lưu whitelist (thư mục có thể chưa tồn tại — vẫn bảo vệ).
pub fn save_whitelist(store: &Store, list: &[String]) -> AppResult<Vec<String>> {
    if list.len() > MAX_WHITELIST {
        return Err(AppError::InvalidInput(format!("tối đa {MAX_WHITELIST} thư mục")));
    }
    let mut clean = Vec::new();
    for p in list {
        let v = validate_whitelist_path(p).map_err(AppError::InvalidInput)?;
        if !clean.iter().any(|c: &String| c.eq_ignore_ascii_case(&v)) {
            clean.push(v);
        }
    }
    let json = serde_json::to_string(&clean).map_err(|e| AppError::Other(e.to_string()))?;
    store.set_setting(KEY_WHITELIST, &json)?;
    apply_whitelist(&clean);
    store.log_action_best_effort("settings.whitelist", "whitelist", true, &format!("{} thư mục", clean.len()));
    Ok(clean)
}

// ---- Dọn tự động ------------------------------------------------------------------------

const KEY_AUTO_CLEAN: &str = "auto_clean";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoClean {
    pub enabled: bool,
    /// 1 = hằng ngày, 7 = hằng tuần, 30 = hằng tháng.
    pub interval_days: u32,
    /// Rule được phép dọn tự động — chỉ rule rủi ro thấp, không đụng service.
    pub rule_ids: Vec<String>,
    pub last_run_ms: Option<i64>,
}

impl Default for AutoClean {
    fn default() -> Self {
        Self { enabled: false, interval_days: 7, rule_ids: Vec::new(), last_run_ms: None }
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

impl AutoClean {
    pub fn load(store: &Store) -> Self {
        store
            .get_setting(KEY_AUTO_CLEAN)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, store: &Store) -> AppResult<()> {
        let json = serde_json::to_string(self).map_err(|e| AppError::Other(e.to_string()))?;
        store.set_setting(KEY_AUTO_CLEAN, &json)
    }

    /// Đến hạn chạy chưa (lần đầu bật: chạy ở lần kiểm tra kế tiếp).
    pub fn due(&self, now_ms: i64) -> bool {
        let interval = i64::from(self.interval_days.clamp(1, 30)) * 86_400_000;
        self.enabled && !self.rule_ids.is_empty() && self.last_run_ms.is_none_or(|t| now_ms - t >= interval || t > now_ms)
    }
}

// ---- Khởi động cùng Windows (Task Scheduler) -------------------------------------------

/// Tên task. App cần quyền admin nên Run key sẽ bị UAC chặn — dùng task "Run with highest privileges".
const TASK_NAME: &str = "PC Manager (khởi động cùng Windows)";
/// Tham số dòng lệnh: khởi động ẩn xuống tray.
pub const TRAY_ARG: &str = "--tray";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn schtasks() -> PathBuf {
    PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join(r"System32\schtasks.exe")
}

fn run_schtasks(args: &[&str]) -> AppResult<std::process::Output> {
    let out = std::process::Command::new(schtasks()).args(args).creation_flags(CREATE_NO_WINDOW).output()?;
    Ok(out)
}

pub fn autostart_enabled() -> bool {
    run_schtasks(&["/Query", "/TN", TASK_NAME]).is_ok_and(|o| o.status.success())
}

/// Bỏ tiền tố `\\?\` mà `canonicalize` thêm vào, giữ nguyên hoa/thường.
fn norm_display(p: &Path) -> String {
    let s = p.to_string_lossy();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_owned()
}

/// `exe` (đã chuẩn hoá) nằm BÊN TRONG một thư mục Program Files.
fn inside_program_files(exe_normed: &str, program_files: &[String]) -> bool {
    program_files
        .iter()
        .filter(|p| !p.trim().is_empty())
        .map(|p| norm(p))
        .any(|pf| exe_normed.strip_prefix(pf.as_str()).is_some_and(|rest| rest.starts_with('\\') && rest.len() > 1))
}

/// Task chạy exe với quyền admin mỗi lần đăng nhập, không hỏi UAC. Nếu exe nằm ở nơi
/// người dùng thường ghi được (Desktop, Downloads, bản dev…), phần mềm độc hại có thể
/// thay file exe để được chạy với quyền admin. Chỉ Program Files mới được bảo vệ
/// (chỉ admin ghi được) — nên chỉ cho phép khi app được cài ở đó.
/// `Err(lý do)` nếu không được phép.
pub fn autostart_allowed() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("không xác định được vị trí app: {e}"))?;
    // canonicalize: giải symlink/junction và tên 8.3 trước khi so đường dẫn.
    let real = std::fs::canonicalize(&exe).map_err(|e| format!("không đọc được vị trí app: {e}"))?;
    let pf: Vec<String> = ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"].iter().filter_map(|k| std::env::var(k).ok()).collect();
    if inside_program_files(&norm(&real.to_string_lossy()), &pf) {
        Ok(real)
    } else {
        Err(format!(
            "PC Manager đang chạy từ {} — chỉ bật được khi app được cài vào Program Files (bằng bộ cài), \
             vì task khởi động chạy với quyền Administrator mà không hỏi UAC.",
            exe.parent().unwrap_or(&exe).display()
        ))
    }
}

pub fn set_autostart(store: &Store, enabled: bool) -> AppResult<bool> {
    // Tạo / xoá task "chạy với quyền cao nhất" đều cần quyền Administrator.
    if !crate::sys::token::is_elevated().unwrap_or(false) {
        return Err(AppError::Forbidden("cần chạy PC Manager với quyền Administrator để đổi tuỳ chọn này".into()));
    }
    let result = if enabled {
        let exe = match autostart_allowed() {
            Ok(p) => p,
            Err(why) => {
                store.log_action_best_effort("settings.autostart", TASK_NAME, false, &why);
                return Err(AppError::Forbidden(why));
            }
        };
        // Dùng đường dẫn thật (không còn tiền tố \\?\) cho Task Scheduler.
        let exe = PathBuf::from(norm_display(&exe));
        let tr = format!("\"{}\" {TRAY_ARG}", exe.display());
        run_schtasks(&["/Create", "/TN", TASK_NAME, "/TR", &tr, "/SC", "ONLOGON", "/RL", "HIGHEST", "/IT", "/F"])
    } else {
        run_schtasks(&["/Delete", "/TN", TASK_NAME, "/F"])
    };
    let out = result?;
    let ok = out.status.success();
    let detail = String::from_utf8_lossy(if ok { &out.stdout } else { &out.stderr }).trim().to_owned();
    store.log_action_best_effort("settings.autostart", TASK_NAME, ok, &format!("{}: {detail}", if enabled { "bật" } else { "tắt" }));
    if !ok {
        return Err(AppError::Other(format!("schtasks lỗi: {detail}")));
    }
    Ok(autostart_enabled())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_only_from_program_files() {
        let pf = vec![r"C:\Program Files".to_owned(), r"C:\Program Files (x86)".to_owned(), String::new()];
        let ok = |p: &str| inside_program_files(&norm(p), &pf);
        assert!(ok(r"C:\Program Files\PC Manager\pc-manager.exe"));
        assert!(ok(r"\\?\C:\PROGRAM FILES (X86)\PC Manager\pc-manager.exe"));
        assert!(!ok(r"C:\Users\An\Desktop\pc-manager\src-tauri\target\release\pc-manager.exe"));
        assert!(!ok(r"C:\Program FilesEvil\pc-manager.exe"), "phải đúng ranh giới thư mục");
        assert!(!ok(r"C:\Program Files"), "không phải chính thư mục gốc");
        assert!(!ok(r"D:\Program Files\PC Manager\pc-manager.exe"));
        assert_eq!(norm_display(Path::new(r"\\?\C:\Program Files\PC Manager\a.exe")), r"C:\Program Files\PC Manager\a.exe");
        // Bản test đang chạy từ target\ => không được phép.
        assert!(autostart_allowed().is_err());
    }

    #[test]
    fn whitelist_validation() {
        assert_eq!(validate_whitelist_path(r#""D:\Projects\""#), Ok(r"D:\Projects".to_owned()));
        assert!(validate_whitelist_path(r"Projects").is_err());
        assert!(validate_whitelist_path(r"\\server\share").is_err());
        assert!(validate_whitelist_path(r"C:\a\..\b").is_err());
        assert!(validate_whitelist_path("").is_err());
    }

    #[test]
    fn whitelist_roundtrip_protects_subpaths() {
        let store = Store::in_memory().unwrap();
        let saved = save_whitelist(&store, &[r"C:\PcmTestKeep\".into(), r"c:\pcmtestkeep".into()]).unwrap();
        assert_eq!(saved, [r"C:\PcmTestKeep"]);
        assert!(is_protected(&norm(r"C:\PcmTestKeep\sub\file.tmp")));
        assert!(is_protected(&norm(r"C:\PCMTESTKEEP")));
        assert!(!is_protected(&norm(r"C:\PcmTestKeepOther\x")));
        assert_eq!(load_whitelist(&store), saved);
        save_whitelist(&store, &[]).unwrap();
        assert!(!is_protected(&norm(r"C:\PcmTestKeep\sub\file.tmp")));
    }

    #[test]
    fn auto_clean_due() {
        let mut a = AutoClean { enabled: true, interval_days: 1, rule_ids: vec!["user-temp".into()], last_run_ms: None };
        assert!(a.due(1_000));
        a.last_run_ms = Some(1_000);
        assert!(!a.due(1_000 + 3_600_000));
        assert!(a.due(1_000 + 86_400_000));
        assert!(a.due(500), "đồng hồ lùi => chạy lại");
        a.enabled = false;
        assert!(!a.due(i64::MAX));
        assert!(!AutoClean { rule_ids: vec![], ..AutoClean { enabled: true, ..AutoClean::default() } }.due(0));
    }
}
