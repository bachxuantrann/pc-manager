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

pub fn set_autostart(store: &Store, enabled: bool) -> AppResult<bool> {
    let result = if enabled {
        let exe = std::env::current_exe()?;
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
