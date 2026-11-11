//! Quản lý tiến trình: danh sách (CPU%, RAM, đường dẫn, publisher) và các
//! thao tác kill / suspend / resume / priority / trim / mở vị trí file.

pub mod calc;
pub mod guard;
pub mod publisher;

use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::store::Store;
use crate::sys::process::{self as sysproc, Access, PriorityClass, ProcessHandle, RawProcess};
use guard::Op;
use publisher::{Publisher, PublisherCache};
use crate::sync::LockExt;

/// Danh tính tiến trình = pid + thời điểm tạo (chống PID bị tái sử dụng).
/// `create_time` là chuỗi vì FILETIME vượt 2^53 (JS number mất chính xác).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRef {
    pub pid: u32,
    pub create_time: String,
}

impl ProcessRef {
    fn create_time(&self) -> AppResult<u64> {
        self.create_time
            .parse()
            .map_err(|_| AppError::InvalidInput(format!("createTime không hợp lệ: {}", self.create_time)))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub create_time: String,
    pub cpu_pct: f64,
    /// Private working set — giống cột "Memory" của Task Manager.
    pub private_bytes: u64,
    pub working_set: u64,
    pub threads: u32,
    pub handles: u32,
    pub session_id: u32,
    pub priority: Option<PriorityClass>,
    pub suspended: bool,
    pub path: Option<String>,
    pub publisher: Publisher,
    /// Có giá trị => UI khoá các thao tác nguy hiểm.
    pub protected_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshot {
    pub processes: Vec<ProcessInfo>,
    pub cpu_count: u32,
    pub total_cpu_pct: f64,
}

type Key = (u32, u64);

/// Thông tin đắt (phải mở handle) — chỉ lấy 1 lần cho mỗi tiến trình.
#[derive(Clone)]
struct Details {
    path: Option<PathBuf>,
    critical: bool,
}

#[derive(Default)]
struct Sampler {
    last: Option<Instant>,
    prev_cpu: HashMap<Key, u64>,
    details: HashMap<Key, Details>,
}

pub struct ProcessManager {
    sampler: Mutex<Sampler>,
    pub publishers: PublisherCache,
    cpus: u32,
    self_pid: u32,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self {
            sampler: Mutex::new(Sampler::default()),
            publishers: PublisherCache::default(),
            cpus: std::thread::available_parallelism().map_or(1, |n| u32::try_from(n.get()).unwrap_or(1)),
            self_pid: std::process::id(),
        }
    }
}

impl ProcessManager {
    pub fn snapshot(&self) -> AppResult<ProcessSnapshot> {
        let raw = sysproc::enumerate()?;
        let now = Instant::now();
        let mut s = self.sampler.lock_recover();
        let elapsed = s.last.map_or(0, |t| u64::try_from(now.duration_since(t).as_nanos() / 100).unwrap_or(u64::MAX));

        let mut prev_cpu = HashMap::with_capacity(raw.len());
        let mut details = HashMap::with_capacity(raw.len());
        let mut processes = Vec::with_capacity(raw.len());
        let mut total = 0.0;

        for p in raw {
            let key = (p.pid, p.create_time);
            let cpu_pct = match s.prev_cpu.get(&key) {
                Some(&prev) => calc::cpu_pct(prev, p.cpu_time, elapsed, self.cpus),
                None => 0.0,
            };
            if p.pid != 0 {
                total += cpu_pct;
            }
            prev_cpu.insert(key, p.cpu_time);

            let d = s.details.remove(&key).unwrap_or_else(|| load_details(&p));
            let publisher = d.path.as_deref().map_or(Publisher::Unknown, |path| self.publishers.get(path));
            let protected_reason = guard::static_protection(p.pid, &p.name, self.self_pid)
                .map(str::to_owned)
                .or_else(|| d.critical.then(|| "tiến trình critical của Windows".to_owned()));

            processes.push(ProcessInfo {
                pid: p.pid,
                ppid: p.ppid,
                create_time: p.create_time.to_string(),
                cpu_pct,
                private_bytes: p.private_ws,
                working_set: p.working_set,
                threads: p.threads,
                handles: p.handles,
                session_id: p.session_id,
                priority: PriorityClass::from_base_priority(p.base_priority),
                suspended: p.suspended,
                path: d.path.as_ref().map(|x| x.display().to_string()),
                publisher,
                protected_reason,
                name: p.name,
            });
            details.insert(key, d);
        }
        // Thay map cũ bằng map mới => tự dọn tiến trình đã thoát.
        s.prev_cpu = prev_cpu;
        s.details = details;
        s.last = Some(now);

        Ok(ProcessSnapshot { processes, cpu_count: self.cpus, total_cpu_pct: total.min(100.0) })
    }

    /// Thực hiện thao tác sau khi: xác minh danh tính, kiểm tra guard, ghi log.
    pub fn act(&self, store: &Store, target: &ProcessRef, op: ProcessOp) -> AppResult<ActionOutcome> {
        let create_time = target.create_time()?;
        let raw = find(target.pid, create_time)?;
        let label = format!("{} ({})", raw.name, raw.pid);

        let access = match op {
            ProcessOp::Kill => Access::Terminate,
            ProcessOp::Suspend | ProcessOp::Resume => Access::SuspendResume,
            ProcessOp::SetPriority(_) => Access::SetPriority,
            ProcessOp::Trim => Access::SetQuota,
            ProcessOp::OpenLocation => Access::Query,
        };

        // Guard tĩnh trước khi mở handle (pid 0/4 không mở được).
        if let Err(msg) = guard::check(op.guard_op(), raw.pid, &raw.name, self.self_pid, false) {
            store.log_action_best_effort(op.action(), &label, false, &msg);
            return Err(AppError::Forbidden(msg));
        }

        let result = (|| -> AppResult<ActionOutcome> {
            let h = ProcessHandle::open(raw.pid, create_time, access)?
                .ok_or_else(|| AppError::NotFound(format!("{label} đã thoát")))?;
            let critical = h.is_critical().unwrap_or(true); // không biết => coi như critical
            guard::check(op.guard_op(), raw.pid, &raw.name, self.self_pid, critical).map_err(AppError::Forbidden)?;

            match op {
                ProcessOp::Kill => h.terminate()?,
                ProcessOp::Suspend => h.suspend()?,
                ProcessOp::Resume => h.resume()?,
                ProcessOp::SetPriority(p) => h.set_priority(p)?,
                ProcessOp::Trim => {
                    h.empty_working_set()?;
                    let after = find(raw.pid, create_time).map_or(0, |p| p.working_set);
                    return Ok(ActionOutcome { trimmed_bytes: Some(raw.working_set.saturating_sub(after)) });
                }
                ProcessOp::OpenLocation => {
                    let path = h.image_path().ok_or_else(|| AppError::NotFound("không đọc được đường dẫn".into()))?;
                    open_in_explorer(&path)?;
                }
            }
            Ok(ActionOutcome { trimmed_bytes: None })
        })();

        // Mở vị trí file không thay đổi hệ thống => không cần log.
        if op != ProcessOp::OpenLocation {
            let detail = match (&result, op) {
                (Ok(o), _) if o.trimmed_bytes.is_some() => format!("trimmed={}B", o.trimmed_bytes.unwrap_or(0)),
                (Ok(_), ProcessOp::SetPriority(p)) => format!("priority={p:?}"),
                (Ok(_), _) => "ok".to_owned(),
                (Err(e), _) => e.to_string(),
            };
            store.log_action_best_effort(op.action(), &label, result.is_ok(), &detail);
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessOp {
    Kill,
    Suspend,
    Resume,
    SetPriority(PriorityClass),
    Trim,
    OpenLocation,
}

impl ProcessOp {
    fn guard_op(self) -> Op {
        match self {
            ProcessOp::Kill => Op::Kill,
            ProcessOp::Suspend => Op::Suspend,
            ProcessOp::Resume => Op::Resume,
            ProcessOp::SetPriority(_) => Op::SetPriority,
            ProcessOp::Trim => Op::Trim,
            ProcessOp::OpenLocation => Op::OpenLocation,
        }
    }

    fn action(self) -> &'static str {
        match self {
            ProcessOp::Kill => "process.kill",
            ProcessOp::Suspend => "process.suspend",
            ProcessOp::Resume => "process.resume",
            ProcessOp::SetPriority(_) => "process.priority",
            ProcessOp::Trim => "process.trim",
            ProcessOp::OpenLocation => "process.open_location",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOutcome {
    pub trimmed_bytes: Option<u64>,
}

fn find(pid: u32, create_time: u64) -> AppResult<RawProcess> {
    sysproc::enumerate()?
        .into_iter()
        .find(|p| p.pid == pid && p.create_time == create_time)
        .ok_or_else(|| AppError::NotFound(format!("tiến trình {pid} đã thoát")))
}

fn load_details(p: &RawProcess) -> Details {
    match ProcessHandle::open(p.pid, p.create_time, Access::Query) {
        Ok(Some(h)) => Details { path: h.image_path(), critical: h.is_critical().unwrap_or(false) },
        // Không mở được (protected process): vẫn thử lấy đường dẫn.
        _ => Details { path: sysproc::image_path(p.pid), critical: false },
    }
}

fn open_in_explorer(path: &std::path::Path) -> AppResult<()> {
    // Đường dẫn lấy từ OS (không chứa dấu "), nên bọc ngoặc kép là an toàn.
    // Đường dẫn tuyệt đối: không để PATH / thư mục hiện tại quyết định chương trình nào chạy
    // (app có thể đang có quyền Administrator).
    let explorer = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("explorer.exe");
    std::process::Command::new(explorer)
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_twice_gives_cpu_and_self_is_protected() {
        let pm = ProcessManager::default();
        pm.snapshot().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let snap = pm.snapshot().unwrap();
        let me = snap.processes.iter().find(|p| p.pid == std::process::id()).unwrap();
        assert!(me.path.as_deref().is_some_and(|p| p.ends_with(".exe")));
        assert_eq!(me.protected_reason.as_deref(), Some("chính PC Manager"));
        assert!(snap.processes.iter().all(|p| (0.0..=100.0).contains(&p.cpu_pct)));
        let system = snap.processes.iter().find(|p| p.pid == 4).unwrap();
        assert!(system.protected_reason.is_some());
    }

    #[test]
    fn killing_self_and_system_is_refused_and_logged() {
        let pm = ProcessManager::default();
        let store = Store::in_memory().unwrap();
        let snap = pm.snapshot().unwrap();
        for pid in [std::process::id(), 4] {
            let p = snap.processes.iter().find(|p| p.pid == pid).unwrap();
            let target = ProcessRef { pid, create_time: p.create_time.clone() };
            let err = pm.act(&store, &target, ProcessOp::Kill).unwrap_err();
            assert_eq!(err.code(), "forbidden");
        }
        let log = store.recent_actions(10).unwrap();
        assert_eq!(log.len(), 2);
        assert!(log.iter().all(|e| e.action == "process.kill" && !e.ok));
    }

    #[test]
    fn stale_identity_is_not_found() {
        let pm = ProcessManager::default();
        let store = Store::in_memory().unwrap();
        let target = ProcessRef { pid: std::process::id(), create_time: "1".into() };
        assert_eq!(pm.act(&store, &target, ProcessOp::Trim).unwrap_err().code(), "not_found");
        let bad = ProcessRef { pid: 1, create_time: "abc".into() };
        assert_eq!(pm.act(&store, &bad, ProcessOp::Trim).unwrap_err().code(), "invalid_input");
    }

    /// Chạy thật: tạo tiến trình con, suspend → resume → trim → kill.
    #[test]
    fn full_lifecycle_on_child_process() {
        let mut child = std::process::Command::new("ping.exe")
            .args(["-n", "30", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        let pm = ProcessManager::default();
        let store = Store::in_memory().unwrap();
        let snap = pm.snapshot().unwrap();
        let p = snap.processes.iter().find(|p| p.pid == pid).unwrap();
        assert!(p.protected_reason.is_none());
        let target = ProcessRef { pid, create_time: p.create_time.clone() };

        // Windows coi tiến trình là "đã tạm dừng" khi mọi luồng đã chuyển sang trạng thái chờ;
        // trên máy chậm (vd. GitHub Actions) việc này cần thêm chút thời gian sau lệnh => chờ tối đa 3 giây.
        let ctime: u64 = p.create_time.parse().unwrap();
        let settles = |want: bool| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            loop {
                if find(pid, ctime).unwrap().suspended == want {
                    return true;
                }
                if std::time::Instant::now() > deadline {
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        };
        pm.act(&store, &target, ProcessOp::Suspend).unwrap();
        assert!(settles(true), "tiến trình phải ở trạng thái tạm dừng");
        pm.act(&store, &target, ProcessOp::Resume).unwrap();
        assert!(settles(false), "tiến trình phải chạy lại");
        pm.act(&store, &target, ProcessOp::SetPriority(PriorityClass::BelowNormal)).unwrap();
        assert!(pm.act(&store, &target, ProcessOp::Trim).unwrap().trimmed_bytes.is_some());
        pm.act(&store, &target, ProcessOp::Kill).unwrap();
        let _ = child.wait();

        let actions: Vec<String> = store.recent_actions(10).unwrap().into_iter().map(|e| e.action).collect();
        assert_eq!(
            actions,
            ["process.kill", "process.trim", "process.priority", "process.resume", "process.suspend"]
        );
    }
}
