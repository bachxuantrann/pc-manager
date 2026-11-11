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

use serde::{Deserialize, Serialize};

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
    /// DISM có khuyên dọn không (nguyên văn DISM — xem `verdict` cho đánh giá thật).
    pub recommended: Option<bool>,
    /// Dọn bây giờ có tác dụng không (DISM + lịch sử dọn của PC Manager).
    pub verdict: WinSxsVerdict,
    /// Dòng kết quả gốc của DISM (để hiện khi không phân tích được).
    pub raw: String,
    pub duration_ms: u64,
}

/// DISM hay báo "Cleanup Recommended: Yes" mãi kể cả khi `/StartComponentCleanup` không lấy lại được
/// gì (gói Windows còn giữ, vd. bản cập nhật gốc của Windows 11 24H2; hoặc phần còn lại chỉ dọn
/// được sau khi khởi động lại). Chỉ đề xuất dọn khi lần dọn này có khả năng thật sự lấy lại dung lượng.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WinSxsVerdict {
    /// DISM không khuyên dọn.
    NotNeeded,
    /// Nên dọn.
    Recommended,
    /// Windows đang chờ khởi động lại để hoàn tất cập nhật (Component Based Servicing) — trong lúc
    /// đó DISM gần như không gỡ được gì. Khởi động lại trước rồi mới dọn.
    PendingRestart,
    /// PC Manager đã dọn sau lần khởi động gần nhất — phần còn lại chỉ dọn được sau khi khởi động lại.
    AfterRestart,
    /// Lần dọn trước không lấy lại được gì và từ đó không có gói mới => dọn lại vô ích.
    Stuck,
}

/// Dưới mức này coi như lần dọn không lấy lại được gì (app khác vẫn ghi/xoá file trong lúc dọn).
pub const NOTHING_FREED: i64 = 16 * 1024 * 1024;
/// Sau thời gian này đánh giá "dọn vô ích" hết hạn, đề xuất lại nếu DISM vẫn khuyên.
const STUCK_TTL_MS: i64 = 30 * 86_400_000;
const HISTORY_KEY: &str = "winsxs.history";

/// Lần dọn không lấy lại được gì.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WinSxsStuck {
    pub at_ms: i64,
    /// Số gói "thu hồi được" theo lần phân tích ngay trước đó (None = chưa phân tích).
    pub packages: Option<u32>,
}

/// Lịch sử phân tích/dọn WinSxS, lưu trong SQLite (bảng settings).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WinSxsHistory {
    /// Số gói thu hồi được ở lần phân tích gần nhất.
    pub last_reclaimable: Option<u32>,
    /// Lần dọn thành công gần nhất của PC Manager (unix ms).
    pub last_clean_ms: Option<i64>,
    pub stuck: Option<WinSxsStuck>,
}

impl WinSxsHistory {
    fn load(store: &Store) -> Self {
        store.get_setting(HISTORY_KEY).ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    fn save(&self, store: &Store) {
        if let Ok(json) = serde_json::to_string(self) {
            if let Err(e) = store.set_setting(HISTORY_KEY, &json) {
                eprintln!("không lưu được lịch sử WinSxS: {e}");
            }
        }
    }

    /// Ghi kết quả một lần dọn thành công. Không lấy lại được gì => ghi nhận các gói hiện có là
    /// "kẹt" — trừ khi lý do là Windows đang/sẽ chờ khởi động lại (`reboot_pending`, DISM trả 3010):
    /// khi đó sau khi khởi động lại vẫn dọn được.
    fn record_clean(&mut self, now_ms: i64, freed: i64, reboot_pending: bool) {
        self.last_clean_ms = Some(now_ms);
        self.stuck = (freed < NOTHING_FREED && !reboot_pending).then_some(WinSxsStuck { at_ms: now_ms, packages: self.last_reclaimable });
    }
}

/// Windows đang chờ khởi động lại để hoàn tất thao tác của Component Based Servicing (phần quản lý
/// kho WinSxS). Chỉ dùng key này — key của Windows Update có thể bật vì lý do không liên quan WinSxS.
pub fn servicing_restart_pending() -> bool {
    windows_registry::LOCAL_MACHINE
        .open(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending")
        .is_ok()
}

/// Dọn bây giờ có tác dụng không. `boot_ms` = lần khởi động lại thật gần nhất (unix ms);
/// `restart_pending` = `servicing_restart_pending()`.
pub fn judge(dism_yes: bool, reclaimable: Option<u32>, h: &WinSxsHistory, now_ms: i64, boot_ms: i64, restart_pending: bool) -> WinSxsVerdict {
    if !dism_yes {
        return WinSxsVerdict::NotNeeded;
    }
    if restart_pending {
        return WinSxsVerdict::PendingRestart;
    }
    if h.last_clean_ms.is_some_and(|t| t >= boot_ms && t <= now_ms) {
        return WinSxsVerdict::AfterRestart;
    }
    if let Some(s) = h.stuck {
        let fresh = s.at_ms <= now_ms && now_ms - s.at_ms < STUCK_TTL_MS;
        // Có thêm gói mới (vừa cài bản cập nhật) => có thể dọn được phần mới.
        let nothing_new = match (s.packages, reclaimable) {
            (Some(before), Some(now)) => now <= before,
            _ => true,
        };
        if fresh && nothing_new {
            return WinSxsVerdict::Stuck;
        }
    }
    WinSxsVerdict::Recommended
}

/// Lần khởi động lại thật gần nhất (unix ms).
fn boot_ms(now_ms: i64) -> i64 {
    now_ms.saturating_sub(i64::try_from(sys::firmware::uptime_secs()).unwrap_or(0).saturating_mul(1000))
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
        verdict: WinSxsVerdict::NotNeeded,
        raw: String::new(),
        duration_ms: 0,
    }
}

/// % ước lượng khi xoá `target` byte: dung lượng trống tăng thêm / `target`, trong khoảng 0–99
/// (100% chỉ khi tác vụ thật sự xong).
pub fn freed_percent(free_before: u64, free_now: u64, target: u64) -> f32 {
    if target == 0 {
        return 0.0;
    }
    let gained = free_now.saturating_sub(free_before) as f64;
    ((gained / target as f64) * 100.0).clamp(0.0, 99.0) as f32
}

/// Chạy `Dism.exe args`, đọc output liên tục để báo % (thanh "[==== 42.5% ====]"), trả về
/// (mã thoát, toàn bộ output).
fn run_dism(args: &[&str], on_progress: &dyn Fn(f32)) -> AppResult<(i32, String)> {
    let mut child = Command::new(system32().join("Dism.exe"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    let mut text = String::new();
    if let Some(mut out) = child.stdout.take() {
        let mut buf = [0u8; 512];
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 {
                break;
            }
            let chunk = String::from_utf8_lossy(&buf[..n]);
            // Ghép với đuôi đoạn trước: "42." | "5%" có thể bị cắt giữa hai lần đọc.
            let tail_start = text.len().saturating_sub(8);
            let tail = text.get(tail_start..).unwrap_or_default().to_owned();
            if let Some(p) = last_percent(&format!("{tail}{chunk}")) {
                on_progress(p);
            }
            text.push_str(&chunk);
        }
    }
    let code = child.wait()?.code().unwrap_or(-1);
    Ok((code, text))
}

impl SystemCleaner {
    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, ()>> {
        self.running.try_lock_recover().ok_or_else(|| AppError::Busy("đang có một thao tác dọn hệ thống khác".into()))
    }

    /// `DISM /AnalyzeComponentStore` — chỉ đọc hệ thống, mất khoảng 1 phút. Tiến độ (%) qua
    /// `on_progress`. Ghi số gói thu hồi được vào lịch sử để đánh giá lần dọn sau.
    pub fn analyze_winsxs(&self, store: &Store, on_progress: &dyn Fn(f32)) -> AppResult<WinSxsAnalysis> {
        require_admin()?;
        let _g = self.lock()?;
        let started = Instant::now();
        let (code, text) = run_dism(&["/Online", "/Cleanup-Image", "/AnalyzeComponentStore", "/English"], on_progress)?;
        if code != 0 {
            return Err(AppError::Other(format!("DISM lỗi (mã {code}): {}", text.trim())));
        }
        let mut a = parse_analysis(&text);
        a.raw = text.lines().filter(|l| l.contains(" : ")).map(str::trim).collect::<Vec<_>>().join("\n");
        a.duration_ms = started.elapsed().as_millis() as u64;
        let now = crate::settings::now_ms();
        let mut h = WinSxsHistory::load(store);
        a.verdict = judge(a.recommended == Some(true), a.reclaimable_packages, &h, now, boot_ms(now), servicing_restart_pending());
        h.last_reclaimable = a.reclaimable_packages;
        h.save(store);
        Ok(a)
    }

    /// `DISM /StartComponentCleanup`. Tiến độ (%) gửi qua `on_progress`. Không huỷ được giữa chừng
    /// (dừng DISM đột ngột có thể làm hỏng kho thành phần).
    pub fn clean_winsxs(&self, store: &Store, on_progress: &dyn Fn(f32)) -> AppResult<SystemCleanReport> {
        require_admin()?;
        let _g = self.lock()?;
        let started = Instant::now();
        let before = system_free();
        // Đọc trước khi dọn: đang chờ khởi động lại thì dọn ra 0 byte không có nghĩa là gói bị kẹt.
        let pending_before = servicing_restart_pending();
        let (code, _) = run_dism(&["/Online", "/Cleanup-Image", "/StartComponentCleanup", "/English"], on_progress)?;
        let report = SystemCleanReport {
            freed_bytes: system_free() as i64 - before as i64,
            exit_code: code,
            // Giao diện báo "cần khởi động lại" thay vì "không còn gì để dọn".
            reboot_required: code == 3010 || pending_before || servicing_restart_pending(),
            duration_ms: started.elapsed().as_millis() as u64,
        };
        let ok = code == 0 || code == 3010;
        store.log_action_best_effort(
            "system.winsxs",
            "DISM /StartComponentCleanup",
            ok,
            &format!("mã {code}, giải phóng {} byte, chờ khởi động lại: {}", report.freed_bytes, report.reboot_required),
        );
        if !ok {
            return Err(AppError::Other(format!("DISM kết thúc với mã {code}")));
        }
        let mut h = WinSxsHistory::load(store);
        h.record_clean(crate::settings::now_ms(), report.freed_bytes, report.reboot_required);
        h.save(store);
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
    ///
    /// Disk Cleanup không báo tiến độ => ước lượng % từ dung lượng trống của ổ hệ thống tăng thêm
    /// so với kích thước Windows.old đo trước khi xoá (tối đa 99% tới khi xong).
    pub fn clean_windows_old(&self, store: &Store, on_progress: &dyn Fn(f32)) -> AppResult<SystemCleanReport> {
        require_admin()?;
        let _g = self.lock()?;
        let info = self.windows_old_info();
        if !info.exists {
            return Err(AppError::NotFound("không có thư mục Windows.old".into()));
        }
        let target = info.bytes.unwrap_or(0);
        let started = Instant::now();
        let before = system_free();
        let flag = format!("StateFlags{SAGE_SET:04}");
        let key = windows_registry::LOCAL_MACHINE
            .create(format!(r"{VOLUME_CACHES}\{PREVIOUS_INSTALLATIONS}"))
            .map_err(|e| AppError::Other(format!("không mở được cấu hình Disk Cleanup: {e}")))?;
        key.set_u32(&flag, 2).map_err(|e| AppError::Other(format!("không đặt được cờ Disk Cleanup: {e}")))?;
        let status = Command::new(system32().join("cleanmgr.exe")).arg(format!("/sagerun:{SAGE_SET}")).spawn().and_then(|mut child| {
            on_progress(0.0);
            loop {
                if let Some(s) = child.try_wait()? {
                    return Ok(s);
                }
                std::thread::sleep(std::time::Duration::from_millis(700));
                on_progress(freed_percent(before, system_free(), target));
            }
        });
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
    fn freed_percent_is_capped_below_100() {
        assert_eq!(freed_percent(100, 100, 0), 0.0, "không biết kích thước => 0");
        assert_eq!(freed_percent(100, 50, 1000), 0.0, "trống giảm (app khác ghi) => 0");
        assert_eq!(freed_percent(0, 250, 1000), 25.0);
        assert_eq!(freed_percent(0, 5000, 1000), 99.0, "chỉ 100% khi xong thật");
    }

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

    const DAY: i64 = 86_400_000;

    #[test]
    fn verdict_follows_dism_without_history() {
        let h = WinSxsHistory::default();
        assert_eq!(judge(false, Some(3), &h, 100 * DAY, 99 * DAY, false), WinSxsVerdict::NotNeeded);
        assert_eq!(judge(true, Some(3), &h, 100 * DAY, 99 * DAY, false), WinSxsVerdict::Recommended);
    }

    #[test]
    fn cleaned_since_boot_waits_for_restart() {
        let mut h = WinSxsHistory { last_reclaimable: Some(3), ..Default::default() };
        h.record_clean(100 * DAY, 2 * 1024 * 1024 * 1024, false);
        assert_eq!(h.stuck, None, "lấy lại được nhiều => không kẹt");
        // Phân tích ngay sau khi dọn, chưa khởi động lại: DISM vẫn "Yes".
        assert_eq!(judge(true, Some(1), &h, 100 * DAY + 60_000, 99 * DAY, false), WinSxsVerdict::AfterRestart);
        // Đã khởi động lại sau lần dọn => đề xuất lại được.
        assert_eq!(judge(true, Some(1), &h, 101 * DAY, 100 * DAY + 3_600_000, false), WinSxsVerdict::Recommended);
    }

    #[test]
    fn futile_clean_stops_recommending_until_new_packages() {
        let mut h = WinSxsHistory { last_reclaimable: Some(3), ..Default::default() };
        h.record_clean(100 * DAY, 0, false);
        assert_eq!(h.stuck, Some(WinSxsStuck { at_ms: 100 * DAY, packages: Some(3) }));
        let after_restart = 101 * DAY - 3_600_000;
        assert_eq!(judge(true, Some(3), &h, 101 * DAY, after_restart, false), WinSxsVerdict::Stuck, "cùng số gói => dọn lại vô ích");
        assert_eq!(judge(true, Some(2), &h, 101 * DAY, after_restart, false), WinSxsVerdict::Stuck);
        assert_eq!(judge(true, Some(5), &h, 101 * DAY, after_restart, false), WinSxsVerdict::Recommended, "có bản cập nhật mới");
        assert_eq!(judge(true, Some(3), &h, 131 * DAY, 130 * DAY, false), WinSxsVerdict::Recommended, "hết hạn sau 30 ngày");
        assert_eq!(judge(false, Some(3), &h, 101 * DAY, after_restart, false), WinSxsVerdict::NotNeeded);
    }

    #[test]
    fn futile_clean_needing_reboot_is_not_stuck() {
        let mut h = WinSxsHistory { last_reclaimable: Some(3), ..Default::default() };
        h.record_clean(100 * DAY, 0, true);
        assert_eq!(h.stuck, None, "đang chờ khởi động lại => không coi là kẹt");
        // Sau khi khởi động lại, DISM vẫn khuyên => đề xuất lại.
        assert_eq!(judge(true, Some(3), &h, 101 * DAY, 100 * DAY + 3_600_000, false), WinSxsVerdict::Recommended);
    }

    #[test]
    fn restart_pending_comes_first() {
        let h = WinSxsHistory::default();
        assert_eq!(judge(true, Some(3), &h, 100 * DAY, 99 * DAY, true), WinSxsVerdict::PendingRestart);
        assert_eq!(judge(false, Some(3), &h, 100 * DAY, 99 * DAY, true), WinSxsVerdict::NotNeeded, "DISM không khuyên => không nói gì");
    }

    #[test]
    fn history_round_trips_and_survives_bad_data() {
        let store = Store::in_memory().unwrap();
        assert_eq!(WinSxsHistory::load(&store), WinSxsHistory::default());
        let h = WinSxsHistory { last_reclaimable: Some(4), last_clean_ms: Some(7), stuck: Some(WinSxsStuck { at_ms: 7, packages: None }) };
        h.save(&store);
        assert_eq!(WinSxsHistory::load(&store), h);
        store.set_setting(HISTORY_KEY, "hỏng").unwrap();
        assert_eq!(WinSxsHistory::load(&store), WinSxsHistory::default());
    }

    #[test]
    fn non_admin_is_refused() {
        if !sys::token::is_elevated().unwrap_or(false) {
            let store = Store::in_memory().unwrap();
            let s = SystemCleaner::default();
            assert!(matches!(s.clean_winsxs(&store, &|_| {}), Err(AppError::Forbidden(_))));
            assert!(matches!(s.clean_windows_old(&store, &|_| {}), Err(AppError::Forbidden(_))));
        }
    }

    /// Chỉ đọc (DISM /AnalyzeComponentStore, ~1 phút).
    #[test]
    #[ignore = "cần quyền Administrator (DISM)"]
    fn analyze_winsxs_real() {
        let a = SystemCleaner::default().analyze_winsxs(&Store::in_memory().unwrap(), &|_| {}).unwrap();
        println!("{a:#?}");
        assert!(a.actual_size.is_some() && a.shared_with_windows.is_some());
    }
}
