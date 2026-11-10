//! Module bộ nhớ: ảnh chụp RAM, Boost (purge standby + flush modified), Smart Boost.

pub mod calc;

use std::sync::{Mutex, TryLockError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub use calc::{BoostGain, MemoryStatus, SmartThreshold};

use crate::error::{AppError, AppResult};
use crate::store::Store;
use crate::sys::memory::{self as sysmem, MemoryListCommand};
use crate::sys::token;

const KEY_THRESHOLD: &str = "memory.smart_threshold_pct";
const KEY_AUTO: &str = "memory.smart_auto";

/// Ai kích hoạt Boost — ghi vào log.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BoostTrigger {
    Manual,
    Smart,
    Tray,
    Auto,
}

/// Mức độ Boost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoostMode {
    /// Flush modified + chỉ purge standby ưu tiên thấp (priority 0).
    /// Giữ cache của app hay dùng → không làm chậm lần mở app sau.
    Light,
    /// Flush modified + purge toàn bộ standby. Free tăng mạnh nhưng mất cache.
    Deep,
}

impl BoostMode {
    fn purge_command(self) -> MemoryListCommand {
        match self {
            BoostMode::Light => MemoryListCommand::PurgeLowPriorityStandbyList,
            BoostMode::Deep => MemoryListCommand::PurgeStandbyList,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoostReport {
    pub trigger: BoostTrigger,
    pub mode: BoostMode,
    pub before: MemoryStatus,
    pub after: MemoryStatus,
    pub gain: BoostGain,
    pub flushed_modified: bool,
    pub purged_standby: bool,
    /// Lỗi từng bước (nếu có) — Boost vẫn có thể thành công một phần.
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SmartBoostOutcome {
    /// RAM còn đủ — không làm gì.
    Skipped { available_pct: f64, threshold_pct: u8 },
    Boosted { report: Box<BoostReport> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySettings {
    pub smart_threshold_pct: u8,
    pub smart_auto: bool,
}

impl MemorySettings {
    pub fn load(store: &Store) -> Self {
        let pct = store.setting_or(KEY_THRESHOLD, SmartThreshold::DEFAULT.get());
        Self {
            smart_threshold_pct: SmartThreshold::new(pct).unwrap_or(SmartThreshold::DEFAULT).get(),
            smart_auto: store.setting_or(KEY_AUTO, false),
        }
    }

    pub fn save(self, store: &Store) -> AppResult<()> {
        let t = SmartThreshold::new(self.smart_threshold_pct)?;
        store.set_setting(KEY_THRESHOLD, &t.get().to_string())?;
        store.set_setting(KEY_AUTO, &self.smart_auto.to_string())?;
        store.log_action_best_effort(
            "settings.memory",
            "smart_boost",
            true,
            &format!("threshold={}%, auto={}", t.get(), self.smart_auto),
        );
        Ok(())
    }

    pub fn threshold(self) -> SmartThreshold {
        SmartThreshold::new(self.smart_threshold_pct).unwrap_or(SmartThreshold::DEFAULT)
    }
}

/// Giữ trạng thái privilege và khoá chống chạy Boost song song (tray + UI).
pub struct MemoryManager {
    privileged: bool,
    boost_lock: Mutex<Option<Instant>>,
}

impl MemoryManager {
    /// Bật `SeProfileSingleProcessPrivilege` một lần khi khởi động.
    pub fn init() -> Self {
        let privileged = match token::enable_privilege(token::SE_PROFILE_SINGLE_PROCESS) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[memory] không bật được privilege: {e}");
                false
            }
        };
        Self { privileged, boost_lock: Mutex::new(None) }
    }

    pub fn privileged(&self) -> bool {
        self.privileged
    }

    pub fn snapshot(&self) -> AppResult<MemoryStatus> {
        let basic = sysmem::basic_memory()?;
        // Không có quyền => thiếu standby/modified, nhưng vẫn trả số cơ bản.
        let lists = if self.privileged { sysmem::memory_lists().ok() } else { None };
        Ok(calc::status_from(basic, lists))
    }

    /// Thời điểm Boost gần nhất (để auto Smart Boost có cooldown).
    pub fn last_boost(&self) -> Option<Instant> {
        self.boost_lock.try_lock().ok().and_then(|g| *g)
    }

    /// Flush modified list rồi purge standby (toàn bộ hoặc ưu tiên thấp tuỳ `mode`).
    /// Ghi log vào SQLite.
    pub fn boost(&self, store: &Store, trigger: BoostTrigger, mode: BoostMode) -> AppResult<BoostReport> {
        if !self.privileged {
            let err = AppError::Forbidden("cần chạy PC Manager với quyền Administrator để Boost".into());
            store.log_action_best_effort("memory.boost", "system", false, &err.to_string());
            return Err(err);
        }
        let mut last = match self.boost_lock.try_lock() {
            Ok(g) => g,
            Err(TryLockError::WouldBlock) => return Err(AppError::Busy("Boost đang chạy".into())),
            Err(TryLockError::Poisoned(p)) => p.into_inner(),
        };

        let started = Instant::now();
        let before = self.snapshot()?;
        let mut warnings = Vec::new();
        // Flush trước: trang modified được ghi ra đĩa → thành standby → purge luôn.
        let flushed_modified = run_step(MemoryListCommand::FlushModifiedList, &mut warnings);
        // Ảnh chụp giữa 2 bước để đo riêng flush và purge (xem calc::boost_gain).
        let mid = self.snapshot()?;
        let purged_standby = run_step(mode.purge_command(), &mut warnings);
        let after = self.snapshot()?;
        *last = Some(Instant::now());

        let report = BoostReport {
            trigger,
            mode,
            before,
            after,
            gain: calc::boost_gain(&before, &mid, &after),
            flushed_modified,
            purged_standby,
            warnings,
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        };
        let ok = flushed_modified || purged_standby;
        store.log_action_best_effort(
            "memory.boost",
            "system",
            ok,
            &format!(
                "trigger={:?} mode={:?} available+={}B standby_cleared={:?}B modified_flushed={:?}B flush={} purge={} {}",
                trigger,
                mode,
                report.gain.available_gain,
                report.gain.standby_cleared,
                report.gain.modified_flushed,
                flushed_modified,
                purged_standby,
                report.warnings.join("; ")
            ),
        );
        if ok {
            Ok(report)
        } else {
            Err(AppError::Other(format!("Boost thất bại: {}", report.warnings.join("; "))))
        }
    }

    /// Chỉ Boost khi available < ngưỡng. Lúc đó RAM thực sự thiếu nên dùng
    /// `Deep`: standby khi ấy vốn đã nhỏ, xoá hết để app đang cần có trang free ngay.
    pub fn smart_boost(
        &self,
        store: &Store,
        threshold: SmartThreshold,
        trigger: BoostTrigger,
    ) -> AppResult<SmartBoostOutcome> {
        let now = self.snapshot()?;
        if !calc::should_smart_boost(&now, threshold) {
            return Ok(SmartBoostOutcome::Skipped {
                available_pct: calc::available_pct(&now),
                threshold_pct: threshold.get(),
            });
        }
        let report = self.boost(store, trigger, BoostMode::Deep)?;
        Ok(SmartBoostOutcome::Boosted { report: Box::new(report) })
    }

    /// Một vòng kiểm tra cho auto Smart Boost (gọi định kỳ từ thread nền).
    /// Có cooldown để không purge liên tục khi RAM luôn thấp.
    pub fn auto_tick(&self, store: &Store, cooldown: Duration) -> Option<BoostReport> {
        let settings = MemorySettings::load(store);
        if !settings.smart_auto || !self.privileged {
            return None;
        }
        if self.last_boost().is_some_and(|t| t.elapsed() < cooldown) {
            return None;
        }
        match self.smart_boost(store, settings.threshold(), BoostTrigger::Auto) {
            Ok(SmartBoostOutcome::Boosted { report }) => Some(*report),
            Ok(SmartBoostOutcome::Skipped { .. }) => None,
            Err(e) => {
                eprintln!("[memory] auto smart boost lỗi: {e}");
                None
            }
        }
    }
}

fn run_step(cmd: MemoryListCommand, warnings: &mut Vec<String>) -> bool {
    match sysmem::memory_list_command(cmd) {
        Ok(()) => true,
        Err(e) => {
            warnings.push(format!("{cmd:?}: {e}"));
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_and_validation() {
        let store = Store::in_memory().unwrap();
        let s = MemorySettings::load(&store);
        assert_eq!(s.smart_threshold_pct, 15);
        assert!(!s.smart_auto);

        MemorySettings { smart_threshold_pct: 25, smart_auto: true }.save(&store).unwrap();
        let s = MemorySettings::load(&store);
        assert_eq!((s.smart_threshold_pct, s.smart_auto), (25, true));

        assert!(MemorySettings { smart_threshold_pct: 90, smart_auto: false }.save(&store).is_err());
        // Thay đổi cài đặt cũng được ghi log.
        assert_eq!(store.recent_actions(10).unwrap().len(), 1);
    }

    #[test]
    fn unprivileged_boost_is_refused_and_logged() {
        let store = Store::in_memory().unwrap();
        let mm = MemoryManager { privileged: false, boost_lock: Mutex::new(None) };
        let err = mm.boost(&store, BoostTrigger::Manual, BoostMode::Light).unwrap_err();
        assert_eq!(err.code(), "forbidden");
        let log = store.recent_actions(1).unwrap();
        assert_eq!(log[0].action, "memory.boost");
        assert!(!log[0].ok);
    }

    /// Boost thật — chạy trong terminal Administrator: `cargo test -- --ignored`.
    #[test]
    #[ignore = "cần quyền Administrator, thay đổi trạng thái RAM của máy"]
    fn real_boost_when_elevated() {
        assert!(crate::sys::token::is_elevated().unwrap(), "hãy chạy trong terminal Administrator");
        let mm = MemoryManager::init();
        assert!(mm.privileged());
        let s = mm.snapshot().unwrap();
        assert!(s.standby.is_some() && s.modified.is_some() && s.free.is_some());
        let store = Store::in_memory().unwrap();
        let mb = |v: Option<u64>| v.map_or(-1, |b| (b / (1024 * 1024)) as i64);
        for mode in [BoostMode::Light, BoostMode::Deep] {
            let r = mm.boost(&store, BoostTrigger::Manual, mode).unwrap();
            assert!(r.flushed_modified && r.purged_standby, "{:?}", r.warnings);
            println!(
                "{mode:?}: available +{} MB | standby xoá {} MB | modified ghi {} MB | free +{} MB | {} ms",
                r.gain.available_gain / (1024 * 1024),
                mb(r.gain.standby_cleared),
                mb(r.gain.modified_flushed),
                mb(r.gain.free_gain),
                r.duration_ms
            );
        }
    }

    #[test]
    fn snapshot_works_without_privilege() {
        let mm = MemoryManager { privileged: false, boost_lock: Mutex::new(None) };
        let s = mm.snapshot().unwrap();
        assert!(s.total > 0);
        assert_eq!(s.standby, None);
    }
}
