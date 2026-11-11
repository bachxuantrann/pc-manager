//! Smart Diagnosis: Scan (thu thập) → Analyze/Recommend (`rules`) → Action (giao diện, qua các trang
//! sẵn có với bước xem trước/xác nhận) → Verify (quét lại, giao diện so trước/sau theo `Finding::id`).
//!
//! Quét nhanh (vài giây), chỉ đọc: KHÔNG kiểm tra chữ ký app khởi động, KHÔNG đo thư mục app, KHÔNG
//! chạy DISM — phân tích WinSxS chỉ có ở "Quét sâu". Không chạy nền, không lưu gì.

pub mod rules;

use std::time::Instant;

use serde::Serialize;

use crate::error::AppResult;
use crate::state::AppState;
use crate::sys;
use rules::{analyze, Category, DriveSpace, Finding, Severity, Snapshot};

const RECYCLE_RULE: &str = "recycle-bin";
/// "Không dùng" = hơn 6 tháng.
const UNUSED_MS: i64 = 180 * 86_400_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagProgress {
    pub step: &'static str,
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStatus {
    pub category: Category,
    /// Mức nặng nhất trong nhóm.
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub findings: Vec<Finding>,
    pub categories: Vec<CategoryStatus>,
    /// Ghi chú về phần không quét được (thiếu quyền, đang bận…).
    pub notes: Vec<String>,
    pub deep: bool,
    pub duration_ms: u64,
}

fn summarize(findings: &[Finding]) -> Vec<CategoryStatus> {
    [Category::Memory, Category::Storage, Category::Startup, Category::Apps, Category::Windows, Category::Hardware]
        .into_iter()
        .filter_map(|c| {
            let severity = findings.iter().filter(|f| f.category == c).map(|f| f.severity).max()?;
            Some(CategoryStatus { category: c, severity })
        })
        .collect()
}

fn reg_key_exists(path: &str) -> bool {
    windows_registry::LOCAL_MACHINE.open(path).is_ok()
}

/// Chạy chẩn đoán. `report` báo bước đang làm (để hiện %).
pub fn run(state: &AppState, deep: bool, report: &dyn Fn(DiagProgress)) -> AppResult<Report> {
    let started = Instant::now();
    let steps: &[&'static str] = if deep {
        &["Bộ nhớ", "Ổ đĩa", "Rác & Thùng rác", "Khởi động", "Ứng dụng", "Windows", "Phần cứng", "Kho WinSxS (DISM, ~1 phút)"]
    } else {
        &["Bộ nhớ", "Ổ đĩa", "Rác & Thùng rác", "Khởi động", "Ứng dụng", "Windows", "Phần cứng"]
    };
    let total = u32::try_from(steps.len()).unwrap_or(u32::MAX);
    let mut done = 0u32;
    let mut step = |i: usize| {
        report(DiagProgress { step: steps.get(i).copied().unwrap_or(""), done, total });
        done = done.saturating_add(1);
    };
    let mut s = Snapshot::default();
    let mut notes = Vec::new();

    step(0);
    if let Ok(m) = sys::memory::basic_memory() {
        s.mem_total = m.total_phys;
        s.mem_avail = m.avail_phys;
        s.commit_total = m.commit_total;
        s.commit_limit = m.commit_limit;
    }

    step(1);
    let sys_letter = std::env::var("SystemDrive").ok().and_then(|d| d.chars().next()).unwrap_or('C').to_ascii_uppercase();
    s.drives = sys::disk::letters()
        .into_iter()
        .map(sys::disk::drive)
        .filter(|d| matches!(d.kind, sys::disk::DriveType::Fixed) && d.total > 0)
        .filter_map(|d| {
            let letter = d.root.chars().next()?.to_ascii_uppercase();
            Some(DriveSpace { letter, total: d.total, free: d.free, system: letter == sys_letter })
        })
        .collect();

    step(2);
    match state.cleaner.estimate() {
        Ok(scan) => {
            let ready = |r: &&crate::cleaner::RuleScan| matches!(r.status, crate::cleaner::RuleStatus::Ready);
            s.recycle_bytes = Some(scan.rules.iter().filter(ready).filter(|r| r.id == RECYCLE_RULE).map(|r| r.bytes).sum());
            s.junk_bytes = Some(scan.rules.iter().filter(ready).filter(|r| r.id != RECYCLE_RULE).map(|r| r.bytes).sum());
            if !sys::token::is_elevated().unwrap_or(false) {
                notes.push("Một số mục rác (Windows Temp, cache Windows Update…) cần quyền Administrator nên chưa được tính.".into());
            }
        }
        Err(e) => notes.push(format!("Chưa ước lượng được rác: {e}")),
    }

    step(3);
    s.startup_enabled = Some(crate::startup::enabled_app_count());

    step(4);
    s.unused = Some(crate::usage::unused_summary(&state.store, UNUSED_MS));

    step(5);
    s.pending_reboot = reg_key_exists(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending")
        || reg_key_exists(r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired");
    s.windows_old = std::fs::symlink_metadata(format!(r"{sys_letter}:\Windows.old")).is_ok_and(|m| m.is_dir());
    s.since_restart_secs = Some(sys::firmware::uptime_secs());
    s.fast_startup = windows_registry::LOCAL_MACHINE
        .open(r"SYSTEM\CurrentControlSet\Control\Session Manager\Power")
        .and_then(|k| k.get_u32("HiberbootEnabled"))
        .is_ok_and(|v| v != 0);

    step(6);
    if let Some(t) = sys::firmware::smbios_table() {
        let smb = crate::hwinfo::smbios::parse(&t);
        let filled: Vec<_> = smb.slots.iter().filter(|m| m.size_mb.is_some()).collect();
        s.ram_slow = filled
            .iter()
            .filter_map(|m| Some((m.configured_mts?, m.speed_mts?)))
            .find(|(now, max)| now < max && max - now >= 200);
        s.ram_slots_free = smb.ram_slots_total.map(|t| t.saturating_sub(u32::try_from(filled.len()).unwrap_or(u32::MAX)));
    }
    let disks = sys::storage::physical_disks();
    s.all_disks_hdd = !disks.is_empty() && disks.iter().all(|d| d.seek_penalty == Some(true));

    if deep {
        step(7);
        match state.system.analyze_winsxs(&|_| {}) {
            Ok(a) => s.winsxs_recommended = a.recommended,
            Err(e) => notes.push(format!("Không phân tích được WinSxS: {e}")),
        }
    }
    report(DiagProgress { step: "Phân tích kết quả", done: total, total });

    let findings = analyze(&s);
    Ok(Report {
        categories: summarize(&findings),
        findings,
        notes,
        deep,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn quick_scan_on_this_machine_is_fast_and_complete() {
        let dir = std::env::temp_dir().join(format!("pcm-diag-{}", std::process::id()));
        let state = crate::state::AppState::init(Some(&dir));
        let steps = std::sync::Mutex::new(Vec::new());
        let r = super::run(&state, false, &|p| steps.lock().unwrap().push(p.step)).unwrap();
        println!("{} ms\n{}", r.duration_ms, serde_json::to_string_pretty(&r).unwrap_or_default());
        assert_eq!(r.categories.len(), 6, "đủ 6 nhóm");
        assert!(steps.lock().unwrap().len() >= 7, "báo tiến độ từng bước");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
