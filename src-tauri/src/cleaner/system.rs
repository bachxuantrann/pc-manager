//! Dọn sâu hệ thống — chỉ qua công cụ chính thức của Windows (CLAUDE.md AN TOÀN #4):
//! - WinSxS: `DISM /Online /Cleanup-Image /StartComponentCleanup` (KHÔNG /ResetBase —
//!   giữ khả năng gỡ bản cập nhật).
//! - Windows.old: Disk Cleanup (`cleanmgr /sagerun`) chỉ bật mục "Previous Installations".
//!   PC Manager không tự xoá file trong Windows.old (thuộc TrustedInstaller).

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::store::Store;
use crate::sys;
use crate::sync::LockExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const VOLUME_CACHES: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\VolumeCaches";
const PREVIOUS_INSTALLATIONS: &str = "Previous Installations";
/// Bộ cờ riêng của PC Manager cho `cleanmgr /sagerun:N` (0–9999).
const SAGE_SET: u32 = 4242;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinSxsAnalysis {
    /// Tổng kích thước thực của kho thành phần (None nếu không đọc được).
    pub actual_size: Option<String>,
    /// Phần dùng chung với Windows (hard link sang System32…) — KHÔNG dọn được.
    pub shared_with_windows: Option<String>,
    /// Bản sao lưu của thành phần cũ + tính năng đang tắt — dọn được một phần.
    pub backups: Option<String>,
    /// Cache / dữ liệu tạm của Windows Update — dọn được.
    pub cache: Option<String>,
    /// Số gói thay thế có thể thu hồi.
    pub reclaimable_packages: Option<u32>,
    pub last_cleanup: Option<String>,
    /// DISM có khuyên dọn không.
    pub recommended: Option<bool>,
    /// Dòng kết quả gốc của DISM (để hiện khi không phân tích được).
    pub raw: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemCleanReport {
    /// Dung lượng trống của ổ hệ thống tăng thêm (xấp xỉ — app khác cũng ghi đĩa cùng lúc).
    pub freed_bytes: i64,
    pub exit_code: i32,
    /// DISM trả 3010: cần khởi động lại để hoàn tất.
    pub reboot_required: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsOldInfo {
    pub path: String,
    pub exists: bool,
    /// Kích thước đọc được (thiếu phần không có quyền đọc).
    pub bytes: Option<u64>,
}

#[derive(Default)]
pub struct SystemCleaner {
    running: Mutex<()>,
}

fn system32() -> PathBuf {
    PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32")
}

fn system_drive_letter() -> char {
    std::env::var("SystemDrive").ok().and_then(|d| d.chars().next()).unwrap_or('C')
}

fn system_free() -> u64 {
    sys::disk::drive(system_drive_letter()).free
}

fn require_admin() -> AppResult<()> {
    if sys::token::is_elevated().unwrap_or(false) {
        Ok(())
    } else {
        Err(AppError::Forbidden("cần chạy PC Manager với quyền Administrator".into()))
    }
}

/// Phần trăm cuối cùng trong một đoạn output DISM ("[====  42.5%  ====]").
pub fn last_percent(s: &str) -> Option<f32> {
    let bytes = s.as_bytes();
    let mut best = None;
    for (i, _) in s.match_indices('%') {
        let start = bytes[..i].iter().rposition(|b| !(b.is_ascii_digit() || *b == b'.')).map_or(0, |p| p + 1);
        if let Ok(v) = s[start..i].parse::<f32>() {
            if (0.0..=100.0).contains(&v) {
                best = Some(v);
            }
        }
    }
    best
}

/// Đọc "Khoá : Giá trị" trong output DISM (tiếng Anh). None nếu không có dòng đó.
fn dism_field<'a>(out: &'a str, key: &str) -> Option<&'a str> {
    out.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(key).then(|| v.trim())
    })
}

/// Phân tích output `DISM /AnalyzeComponentStore /English`. `raw` và `duration_ms` để trống.
pub fn parse_analysis(out: &str) -> WinSxsAnalysis {
    let f = |k: &str| dism_field(out, k).filter(|v| !v.is_empty()).map(str::to_owned);
    WinSxsAnalysis {
        actual_size: f("Actual Size of Component Store"),
        shared_with_windows: f("Shared with Windows"),
        backups: f("Backups and Disabled Features"),
        cache: f("Cache and Temporary Data"),
        reclaimable_packages: f("Number of Reclaimable Packages").and_then(|v| v.parse().ok()),
        last_cleanup: f("Date of Last Cleanup"),
        recommended: f("Component Store Cleanup Recommended").map(|v| v.eq_ignore_ascii_case("yes")),
        raw: String::new(),
        duration_ms: 0,
    }
}

impl SystemCleaner {
    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, ()>> {
        self.running.try_lock_recover().ok_or_else(|| AppError::Busy("đang có một thao tác dọn hệ thống khác".into()))
    }

    /// `DISM /AnalyzeComponentStore` — chỉ đọc, mất khoảng 1 phút.
    pub fn analyze_winsxs(&self) -> AppResult<WinSxsAnalysis> {
        require_admin()?;
        let _g = self.lock()?;
        let started = Instant::now();
        let out = Command::new(system32().join("Dism.exe"))
            .args(["/Online", "/Cleanup-Image", "/AnalyzeComponentStore", "/English"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        if !out.status.success() {
            return Err(AppError::Other(format!("DISM lỗi (mã {}): {}", out.status.code().unwrap_or(-1), text.trim())));
        }
        let mut a = parse_analysis(&text);
        a.raw = text.lines().filter(|l| l.contains(" : ")).map(str::trim).collect::<Vec<_>>().join("\n");
        a.duration_ms = started.elapsed().as_millis() as u64;
        Ok(a)
    }

    /// `DISM /StartComponentCleanup`. Tiến độ (%) gửi qua `on_progress`. Không huỷ được giữa chừng
    /// (dừng DISM đột ngột có thể làm hỏng kho thành phần).
    pub fn clean_winsxs(&self, store: &Store, on_progress: &dyn Fn(f32)) -> AppResult<SystemCleanReport> {
        require_admin()?;
        let _g = self.lock()?;
        let started = Instant::now();
        let before = system_free();
        let mut child = Command::new(system32().join("Dism.exe"))
            .args(["/Online", "/Cleanup-Image", "/StartComponentCleanup", "/English"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
        if let Some(mut out) = child.stdout.take() {
            let mut buf = [0u8; 512];
            while let Ok(n) = out.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if let Some(p) = last_percent(&String::from_utf8_lossy(&buf[..n])) {
                    on_progress(p);
                }
            }
        }
        let code = child.wait()?.code().unwrap_or(-1);
        let report = SystemCleanReport {
            freed_bytes: system_free() as i64 - before as i64,
            exit_code: code,
            reboot_required: code == 3010,
            duration_ms: started.elapsed().as_millis() as u64,
        };
        let ok = code == 0 || code == 3010;
        store.log_action_best_effort("system.winsxs", "DISM /StartComponentCleanup", ok, &format!("mã {code}, giải phóng {} byte", report.freed_bytes));
        if !ok {
            return Err(AppError::Other(format!("DISM kết thúc với mã {code}")));
        }
        Ok(report)
    }

    pub fn windows_old_info(&self) -> WindowsOldInfo {
        let path = PathBuf::from(format!("{}:\\Windows.old", system_drive_letter()));
        let exists = std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir());
        let bytes = exists.then(|| {
            let mut total = 0u64;
            let _ = crate::disk::walk::walk(&path, &AtomicBool::new(false), |f| total += f.size);
            total
        });
        WindowsOldInfo { path: path.display().to_string(), exists, bytes }
    }

    /// Xoá bản cài Windows trước bằng Disk Cleanup (chỉ bật mục "Previous Installations").
    pub fn clean_windows_old(&self, store: &Store) -> AppResult<SystemCleanReport> {
        require_admin()?;
        let _g = self.lock()?;
        if !self.windows_old_info().exists {
            return Err(AppError::NotFound("không có thư mục Windows.old".into()));
        }
        let started = Instant::now();
        let before = system_free();
        let flag = format!("StateFlags{SAGE_SET:04}");
        let key = windows_registry::LOCAL_MACHINE
            .create(format!(r"{VOLUME_CACHES}\{PREVIOUS_INSTALLATIONS}"))
            .map_err(|e| AppError::Other(format!("không mở được cấu hình Disk Cleanup: {e}")))?;
        key.set_u32(&flag, 2).map_err(|e| AppError::Other(format!("không đặt được cờ Disk Cleanup: {e}")))?;
        let status = Command::new(system32().join("cleanmgr.exe")).arg(format!("/sagerun:{SAGE_SET}")).status();
        // Luôn gỡ cờ để lần chạy Disk Cleanup sau của người dùng không bị ảnh hưởng.
        let _ = key.remove_value(&flag);
        let code = status?.code().unwrap_or(-1);
        let still = self.windows_old_info().exists;
        let report = SystemCleanReport {
            freed_bytes: system_free() as i64 - before as i64,
            exit_code: code,
            reboot_required: false,
            duration_ms: started.elapsed().as_millis() as u64,
        };
        store.log_action_best_effort(
            "system.windows_old",
            "cleanmgr Previous Installations",
            !still,
            &format!("mã {code}, giải phóng {} byte, còn Windows.old: {still}", report.freed_bytes),
        );
        if still {
            return Err(AppError::Other("Disk Cleanup đã chạy nhưng Windows.old vẫn còn".into()));
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_from_dism_progress() {
        assert_eq!(last_percent("[==========                 20.0%                          ]"), Some(20.0));
        assert_eq!(last_percent("[=== 5.2% ===]\r[====== 10.9% ====]"), Some(10.9));
        assert_eq!(last_percent("[==========================100.0%==========================]"), Some(100.0));
        assert_eq!(last_percent("không có phần trăm"), None);
        assert_eq!(last_percent("150%"), None);
    }

    #[test]
    fn analysis_fields() {
        let out = "Component Store (WinSxS) information:\r\n\r\nWindows Explorer Reported Size of Component Store : 13.20 GB\r\n\r\nActual Size of Component Store : 12.67 GB\r\n\r\n    Shared with Windows : 9.84 GB\r\n    Backups and Disabled Features : 2.61 GB\r\n    Cache and Temporary Data : 220.41 MB\r\n\r\nDate of Last Cleanup : 2026-11-10 18:42:07\r\n\r\nNumber of Reclaimable Packages : 3\r\nComponent Store Cleanup Recommended : Yes\r\n";
        let a = parse_analysis(out);
        assert_eq!(a.actual_size.as_deref(), Some("12.67 GB"));
        assert_eq!(a.shared_with_windows.as_deref(), Some("9.84 GB"));
        assert_eq!(a.backups.as_deref(), Some("2.61 GB"));
        assert_eq!(a.cache.as_deref(), Some("220.41 MB"));
        assert_eq!(a.reclaimable_packages, Some(3));
        assert_eq!(a.last_cleanup.as_deref(), Some("2026-11-10 18:42:07"), "giờ có dấu ':' vẫn đọc đúng");
        assert_eq!(a.recommended, Some(true));
        let none = parse_analysis("Kích thước : 7 GB");
        assert!(none.actual_size.is_none() && none.recommended.is_none());
    }

    #[test]
    fn non_admin_is_refused() {
        if !sys::token::is_elevated().unwrap_or(false) {
            let store = Store::in_memory().unwrap();
            let s = SystemCleaner::default();
            assert!(matches!(s.clean_winsxs(&store, &|_| {}), Err(AppError::Forbidden(_))));
            assert!(matches!(s.clean_windows_old(&store), Err(AppError::Forbidden(_))));
        }
    }

    /// Chỉ đọc (DISM /AnalyzeComponentStore, ~1 phút).
    #[test]
    #[ignore = "cần quyền Administrator (DISM)"]
    fn analyze_winsxs_real() {
        let a = SystemCleaner::default().analyze_winsxs().unwrap();
        println!("{a:#?}");
        assert!(a.actual_size.is_some() && a.shared_with_windows.is_some());
    }
}
