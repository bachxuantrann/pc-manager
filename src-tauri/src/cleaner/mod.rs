//! Cleaner: rule engine dọn rác (Scan → Preview → xác nhận → Clean → Report).
//!
//! Bất biến an toàn (CLAUDE.md, mục AN TOÀN):
//! - Không command nào nhận đường dẫn từ UI. UI chỉ gửi `scan_id` + danh sách
//!   `rule id`; file bị xoá là đúng những file engine đã tự tìm lúc quét.
//! - Mỗi file được kiểm tra lại (ban list, link, tuổi, kích thước/ngày sửa) ngay
//!   trước khi xoá. Có chế độ dry-run.
//! - Mọi lần dọn (kể cả dry-run) ghi log SQLite.

pub mod engine;
pub mod glob;
pub mod rules;
pub mod safety;
pub mod system;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::{Duration, Instant, SystemTime};

use rayon::prelude::*;
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::store::Store;
use crate::sys;
use engine::{Counters, DeleteStats, RuleScanData};
use rules::{Risk, Rule, RuleKind};
use safety::BanList;
use crate::sync::LockExt;

/// Kết quả quét hết hạn sau 30 phút — phải quét lại trước khi dọn.
const SCAN_TTL: Duration = Duration::from_secs(30 * 60);
const SERVICE_STOP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RuleStatus {
    Ready,
    Empty,
    NeedsAdmin,
    Blocked { reason: String },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleScan {
    pub id: String,
    pub name: String,
    pub group: String,
    pub description: String,
    pub risk: Risk,
    pub status: RuleStatus,
    pub bytes: u64,
    pub files: u64,
    pub skipped_young: u64,
    pub skipped_links: u64,
    pub default_selected: bool,
    pub uses_service: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub scan_id: u64,
    pub rules: Vec<RuleScan>,
    pub total_bytes: u64,
    pub duration_ms: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleClean {
    pub id: String,
    pub name: String,
    #[serde(flatten)]
    pub stats: DeleteStats,
    /// Lý do bỏ qua cả rule (trình duyệt đang chạy, service không dừng được…).
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanReport {
    pub dry_run: bool,
    pub rules: Vec<RuleClean>,
    pub freed_bytes: u64,
    pub deleted_files: u64,
    pub skipped_locked: u64,
    pub skipped_changed: u64,
    pub errors: u64,
    pub duration_ms: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// "scan" | "clean"
    pub phase: &'static str,
    pub done_rules: u32,
    pub total_rules: u32,
    pub current: String,
    pub files: u64,
    pub bytes: u64,
}

enum Prepared {
    Files(RuleScanData),
    Recycle(sys::recycle::RecycleInfo),
}

struct StoredScan {
    id: u64,
    at: Instant,
    data: HashMap<String, Prepared>,
}

pub struct CleanerManager {
    rules: Vec<Rule>,
    bans: BanList,
    elevated: bool,
    last: Mutex<Option<StoredScan>>,
    cancel: Arc<AtomicBool>,
    running: Mutex<()>,
    next_id: AtomicU64,
    env: fn(&str) -> Option<String>,
}

/// Biến môi trường của user đang đăng nhập (`%TEMP%`, `%LOCALAPPDATA%`…), kể cả khi app được
/// nâng quyền bằng tài khoản admin khác.
fn real_env(k: &str) -> Option<String> {
    crate::user::env(k)
}

impl CleanerManager {
    pub fn new(rules: Vec<Rule>, bans: BanList, elevated: bool) -> Self {
        Self {
            rules,
            bans,
            elevated,
            last: Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            running: Mutex::new(()),
            next_id: AtomicU64::new(1),
            env: real_env,
        }
    }

    pub fn from_system() -> Self {
        let rules = rules::embedded().unwrap_or_else(|e| {
            eprintln!("[cleaner] {e}");
            Vec::new()
        });
        Self::new(rules, BanList::from_env(), sys::token::is_elevated().unwrap_or(false))
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Rule đủ an toàn để dọn tự động theo lịch: rủi ro thấp, không dừng service,
    /// không phải Thùng rác. Trả về (id, tên, mô tả).
    pub fn auto_eligible(&self) -> Vec<(String, String, String)> {
        self.rules
            .iter()
            .filter(|r| r.risk == Risk::Low && r.service.is_none() && r.kind == RuleKind::Files)
            .map(|r| (r.id.clone(), r.name.clone(), r.description.clone()))
            .collect()
    }

    /// Dọn tự động: quét rồi dọn ngay các rule `rule_ids` (chỉ rule hợp lệ cho auto,
    /// đã bật trong Cài đặt — đó là bước người dùng xác nhận). Mọi lớp an toàn
    /// (tuổi file, file bị khoá, danh sách cấm/bảo vệ, link) vẫn áp dụng.
    pub fn auto_clean(&self, store: &Store, rule_ids: &[String]) -> AppResult<CleanReport> {
        let eligible: Vec<String> = self.auto_eligible().into_iter().map(|(id, _, _)| id).collect();
        let scan = self.scan(&|_| {})?;
        let ids: Vec<String> = scan
            .rules
            .iter()
            .filter(|r| matches!(r.status, RuleStatus::Ready) && rule_ids.contains(&r.id) && eligible.contains(&r.id))
            .map(|r| r.id.clone())
            .collect();
        if ids.is_empty() {
            // Không có gì để dọn — bỏ kết quả quét.
            {
                let mut l = self.last.lock_recover();
                l.take();
            }
            return Ok(CleanReport {
                dry_run: false,
                rules: Vec::new(),
                freed_bytes: 0,
                deleted_files: 0,
                skipped_locked: 0,
                skipped_changed: 0,
                errors: 0,
                duration_ms: scan.duration_ms,
                cancelled: false,
            });
        }
        self.clean(store, scan.scan_id, &ids, false, &|_| {})
    }

    /// Pool nền dùng chung (max(2, cpus/2) thread ưu tiên thấp).
    fn pool(&self) -> AppResult<&'static rayon::ThreadPool> {
        crate::bg::pool()
    }

    fn lock_running(&self) -> AppResult<std::sync::MutexGuard<'_, ()>> {
        match self.running.try_lock() {
            Ok(g) => Ok(g),
            Err(TryLockError::WouldBlock) => Err(AppError::Busy("đang quét hoặc dọn".into())),
            Err(TryLockError::Poisoned(p)) => Ok(p.into_inner()),
        }
    }

    fn running_processes() -> HashSet<String> {
        sys::process::enumerate().map(|v| v.into_iter().map(|p| p.name.to_lowercase()).collect()).unwrap_or_default()
    }

    /// Trạng thái trước khi quét: thiếu quyền / ứng dụng đang chạy.
    fn precheck(&self, rule: &Rule, running: &HashSet<String>) -> Option<RuleStatus> {
        if rule.requires_admin && !self.elevated {
            return Some(RuleStatus::NeedsAdmin);
        }
        let open: Vec<&String> = rule.skip_if_running.iter().filter(|p| running.contains(&p.to_lowercase())).collect();
        if let Some(p) = open.first() {
            return Some(RuleStatus::Blocked { reason: format!("{p} đang chạy — hãy đóng ứng dụng rồi quét lại") });
        }
        None
    }

    pub fn scan(&self, on_progress: &(dyn Fn(Progress) + Sync)) -> AppResult<ScanResult> {
        self.scan_impl(on_progress, true)
    }

    /// Quét như `scan` nhưng KHÔNG thay kết quả quét đang dùng ở trang Dọn rác (Smart Diagnosis chỉ
    /// cần ước lượng dung lượng; thay kết quả sẽ làm hỏng bản xem trước người dùng đang mở).
    pub fn estimate(&self) -> AppResult<ScanResult> {
        self.scan_impl(&|_| {}, false)
    }

    fn scan_impl(&self, on_progress: &(dyn Fn(Progress) + Sync), keep: bool) -> AppResult<ScanResult> {
        let _g = self.lock_running()?;
        self.cancel.store(false, Ordering::Relaxed);
        let started = Instant::now();
        let now = SystemTime::now();
        let running = Self::running_processes();
        let counters = Counters::default();
        let done = AtomicU64::new(0);
        let total = self.rules.len() as u32;
        let last_emit = Mutex::new(Instant::now() - Duration::from_secs(1));
        let emit = |current: &str| {
            // Tối đa ~10 lần/giây để không làm nghẽn IPC.
            {
                let mut t = last_emit.lock_recover();
                if t.elapsed() < Duration::from_millis(100) {
                    return;
                }
                *t = Instant::now();
            }
            on_progress(Progress {
                phase: "scan",
                done_rules: done.load(Ordering::Relaxed) as u32,
                total_rules: total,
                current: current.to_owned(),
                files: counters.files.load(Ordering::Relaxed),
                bytes: counters.bytes.load(Ordering::Relaxed),
            });
        };

        let results: Vec<(RuleScan, Option<Prepared>)> = self.pool()?.install(|| {
            self.rules
                .par_iter()
                .map(|rule| {
                    emit(&rule.name);
                    let (status, prepared) = match self.precheck(rule, &running) {
                        Some(s) => (s, None),
                        None => self.scan_rule(rule, now, &counters),
                    };
                    done.fetch_add(1, Ordering::Relaxed);
                    emit(&rule.name);
                    (summarize(rule, status, prepared.as_ref()), prepared)
                })
                .collect()
        });

        let cancelled = self.cancel.load(Ordering::Relaxed);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut data = HashMap::new();
        let mut rules_out = Vec::with_capacity(results.len());
        for (summary, prepared) in results {
            if let (Some(p), RuleStatus::Ready) = (prepared, &summary.status) {
                data.insert(summary.id.clone(), p);
            }
            rules_out.push(summary);
        }
        let total_bytes = rules_out.iter().filter(|r| matches!(r.status, RuleStatus::Ready)).map(|r| r.bytes).sum();
        // Quét bị huỷ dở thì không được dùng để dọn. Ước lượng (keep = false) không đụng tới kết quả cũ.
        if keep {
            let mut last = self.last.lock_recover();
            *last = (!cancelled).then(|| StoredScan { id, at: Instant::now(), data });
        }
        Ok(ScanResult {
            scan_id: id,
            rules: rules_out,
            total_bytes,
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            cancelled,
        })
    }

    fn scan_rule(&self, rule: &Rule, now: SystemTime, c: &Counters) -> (RuleStatus, Option<Prepared>) {
        match rule.kind {
            RuleKind::RecycleBin => match crate::user::recycle::query() {
                Ok(info) if info.items > 0 => (RuleStatus::Ready, Some(Prepared::Recycle(info))),
                Ok(_) => (RuleStatus::Empty, None),
                Err(e) => (RuleStatus::Error { message: e }, None),
            },
            RuleKind::Files => {
                let roots: Vec<_> = rule.paths.iter().flat_map(|p| engine::expand_roots(p, &self.env)).collect();
                let data = engine::scan(rule, roots, &self.bans, now, &self.cancel, c);
                let status = if data.files.is_empty() { RuleStatus::Empty } else { RuleStatus::Ready };
                (status, Some(Prepared::Files(data)))
            }
        }
    }

    /// Dọn các rule đã chọn từ một lần quét. `rule_ids` chỉ là id — không có đường dẫn.
    pub fn clean(
        &self,
        store: &Store,
        scan_id: u64,
        rule_ids: &[String],
        dry_run: bool,
        on_progress: &(dyn Fn(Progress) + Sync),
    ) -> AppResult<CleanReport> {
        let _g = self.lock_running()?;
        self.cancel.store(false, Ordering::Relaxed);
        let started = Instant::now();
        let mut scan = self
            .last
            .lock_recover()
            .take()
            .filter(|s| s.id == scan_id && s.at.elapsed() < SCAN_TTL)
            .ok_or_else(|| AppError::InvalidInput("kết quả quét đã cũ hoặc không tồn tại — hãy quét lại".into()))?;

        let running = Self::running_processes();
        let now = SystemTime::now();
        let counters = Counters::default();
        let total = rule_ids.len() as u32;
        let mut out = Vec::new();

        self.pool()?.install(|| {
            for (i, id) in rule_ids.iter().enumerate() {
                if self.cancel.load(Ordering::Relaxed) {
                    break;
                }
                let Some(rule) = self.rules.iter().find(|r| &r.id == id) else { continue };
                on_progress(Progress {
                    phase: "clean",
                    done_rules: i as u32,
                    total_rules: total,
                    current: rule.name.clone(),
                    files: counters.files.load(Ordering::Relaxed),
                    bytes: counters.bytes.load(Ordering::Relaxed),
                });
                let mut rc = RuleClean { id: id.clone(), name: rule.name.clone(), stats: DeleteStats::default(), message: None };
                match (scan.data.get(id), self.precheck(rule, &running)) {
                    (None, _) => rc.message = Some("không có trong kết quả quét".into()),
                    (_, Some(RuleStatus::Blocked { reason })) => rc.message = Some(reason),
                    (_, Some(_)) => rc.message = Some("cần quyền Administrator".into()),
                    (Some(Prepared::Recycle(info)), None) => {
                        if dry_run {
                            rc.stats.deleted_files = info.items;
                            rc.stats.freed_bytes = info.bytes;
                        } else {
                            match crate::user::recycle::empty() {
                                Ok(()) => {
                                    rc.stats.deleted_files = info.items;
                                    rc.stats.freed_bytes = info.bytes;
                                }
                                Err(e) => rc.message = Some(format!("không làm rỗng được Thùng rác: {e}")),
                            }
                        }
                    }
                    (Some(Prepared::Files(data)), None) => {
                        self.clean_files(store, rule, data, now, dry_run, &counters, &mut rc);
                    }
                }
                log_rule(store, dry_run, &rc);
                out.push(rc);
            }
        });

        // Dry-run không xoá gì => giữ kết quả quét để có thể dọn thật ngay sau đó.
        if dry_run {
            scan.at = Instant::now();
            {
                let mut last = self.last.lock_recover();
                *last = Some(scan);
            }
        }

        let sum = |f: fn(&DeleteStats) -> u64| out.iter().map(|r| f(&r.stats)).sum();
        Ok(CleanReport {
            dry_run,
            freed_bytes: sum(|s| s.freed_bytes),
            deleted_files: sum(|s| s.deleted_files),
            skipped_locked: sum(|s| s.skipped_locked),
            skipped_changed: sum(|s| s.skipped_changed),
            errors: sum(|s| s.errors),
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            cancelled: self.cancel.load(Ordering::Relaxed),
            rules: out,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn clean_files(
        &self,
        store: &Store,
        rule: &Rule,
        data: &RuleScanData,
        now: SystemTime,
        dry_run: bool,
        counters: &Counters,
        rc: &mut RuleClean,
    ) {
        // Dừng service đang giữ file (vd Windows Update), chạy lại sau khi dọn.
        let mut restart = None;
        if let (Some(svc), false) = (&rule.service, dry_run) {
            match sys::service::stop(svc, SERVICE_STOP_TIMEOUT) {
                Ok(was_running) => {
                    store.log_action_best_effort("service.stop", svc, true, &format!("cleaner rule {}", rule.id));
                    restart = was_running.then(|| svc.clone());
                }
                Err(e) => {
                    store.log_action_best_effort("service.stop", svc, false, &e.to_string());
                    rc.message = Some(format!("không dừng được dịch vụ {svc}: {e}"));
                    return;
                }
            }
        }
        rc.stats = engine::delete(rule, data, &self.bans, now, dry_run, &self.cancel, counters);
        if let Some(svc) = restart {
            let r = sys::service::start(&svc);
            store.log_action_best_effort("service.start", &svc, r.is_ok(), &r.err().map(|e| e.to_string()).unwrap_or_default());
        }
    }
}

fn summarize(rule: &Rule, status: RuleStatus, prepared: Option<&Prepared>) -> RuleScan {
    let (bytes, files, young, links) = match prepared {
        Some(Prepared::Files(d)) => (d.bytes(), d.files.len() as u64, d.skipped_young, d.skipped_links),
        Some(Prepared::Recycle(i)) => (i.bytes, i.items, 0, 0),
        None => (0, 0, 0, 0),
    };
    RuleScan {
        id: rule.id.clone(),
        name: rule.name.clone(),
        group: rule.group.clone(),
        description: rule.description.clone(),
        risk: rule.risk,
        status,
        bytes,
        files,
        skipped_young: young,
        skipped_links: links,
        default_selected: rule.default_selected,
        uses_service: rule.service.is_some(),
    }
}

fn log_rule(store: &Store, dry_run: bool, rc: &RuleClean) {
    let s = &rc.stats;
    let detail = format!(
        "deleted={} freed={}B locked={} changed={} errors={}{}{}",
        s.deleted_files,
        s.freed_bytes,
        s.skipped_locked,
        s.skipped_changed,
        s.errors,
        rc.message.as_deref().map(|m| format!(" msg={m}")).unwrap_or_default(),
        s.first_error.as_deref().map(|m| format!(" err={m}")).unwrap_or_default(),
    );
    let action = if dry_run { "cleaner.dry_run" } else { "cleaner.clean" };
    store.log_action_best_effort(action, &rc.id, rc.message.is_none() && s.errors == 0, &detail);
}

#[cfg(test)]
mod tests;
