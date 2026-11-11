//! Tác vụ dài (quét/dọn rác, DISM, quét ổ đĩa) được backend ghi lại để giao diện nối lại được.
//!
//! Đóng cửa sổ = huỷ WebView (app chạy tiếp ở tray), nhưng tác vụ vẫn chạy trong backend. Khi mở
//! lại cửa sổ, giao diện mới hỏi `jobs_snapshot`: tác vụ nào đang chạy thì hiện lại tiến độ (theo
//! dõi qua sự kiện `EVENT_JOB`), tác vụ nào đã xong mà giao diện chưa nhận thì hiện kết quả rồi
//! `jobs_ack` để xoá.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::LockExt;

/// Sự kiện toàn app: một tác vụ bắt đầu, có tiến độ mới, hoặc xong.
pub const EVENT_JOB: &str = "jobs://update";

/// Tiến độ được ghi/phát tối đa ~5 lần/giây (Channel của cửa sổ đã mở vẫn nhận đủ).
const PROGRESS_EVERY: Duration = Duration::from_millis(200);

/// Các loại tác vụ được theo dõi. Chuỗi trùng với `JobKind` phía UI.
pub const KINDS: &[&str] = &["diagnosis.scan", "cleaner.scan", "cleaner.clean", "winsxs.analyze", "winsxs.clean", "winold.clean", "disk.large", "disk.dups"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub ok: bool,
    /// Kết quả (ok) hoặc lỗi `{ code, message }`.
    pub value: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub kind: &'static str,
    pub running: bool,
    pub started_ms: i64,
    pub progress: Option<Value>,
    /// Có khi đã xong; giữ tới khi UI `jobs_ack`.
    pub outcome: Option<Outcome>,
}

#[derive(Default)]
pub struct Jobs {
    inner: Mutex<HashMap<&'static str, (Job, Instant)>>,
}

impl Jobs {
    pub fn snapshot(&self) -> Vec<Job> {
        self.inner.lock_recover().values().map(|(j, _)| j.clone()).collect()
    }

    /// UI đã nhận kết quả => xoá (tác vụ đang chạy thì giữ nguyên).
    pub fn ack(&self, kind: &str) {
        self.inner.lock_recover().retain(|k, (j, _)| *k != kind || j.running);
    }

    fn start(&self, app: &AppHandle, kind: &'static str) {
        let job = Job { kind, running: true, started_ms: crate::settings::now_ms(), progress: None, outcome: None };
        self.inner.lock_recover().insert(kind, (job.clone(), Instant::now()));
        let _ = app.emit(EVENT_JOB, &job);
    }

    fn progress(&self, app: &AppHandle, kind: &'static str, p: Value) {
        let job = {
            let mut g = self.inner.lock_recover();
            let Some((job, last)) = g.get_mut(kind) else { return };
            job.progress = Some(p);
            if last.elapsed() < PROGRESS_EVERY {
                return;
            }
            *last = Instant::now();
            job.clone()
        };
        let _ = app.emit(EVENT_JOB, &job);
    }

    fn finish<T: Serialize>(&self, app: &AppHandle, kind: &'static str, result: &AppResult<T>) {
        let outcome = match result {
            Ok(v) => Outcome { ok: true, value: serde_json::to_value(v).unwrap_or(Value::Null) },
            Err(e) => Outcome { ok: false, value: serde_json::to_value(e).unwrap_or(Value::Null) },
        };
        let job = {
            let mut g = self.inner.lock_recover();
            let entry = g.entry(kind).or_insert_with(|| {
                (Job { kind, running: false, started_ms: crate::settings::now_ms(), progress: None, outcome: None }, Instant::now())
            });
            entry.0.running = false;
            entry.0.outcome = Some(outcome);
            entry.0.clone()
        };
        let _ = app.emit(EVENT_JOB, &job);
    }
}

/// Chạy `f` trên luồng nền như một tác vụ được theo dõi: tiến độ gửi qua `channel` (cửa sổ đã
/// gọi lệnh) VÀ ghi vào `Jobs` + phát `EVENT_JOB` (cửa sổ mở lại sau đó vẫn theo dõi được).
pub async fn run<T, P, F>(app: AppHandle, kind: &'static str, what: &'static str, channel: Option<Channel<P>>, f: F) -> AppResult<T>
where
    T: Serialize + Send + 'static,
    P: Serialize + Clone + Send + 'static,
    Channel<P>: Send + Sync,
    F: FnOnce(&AppState, &(dyn Fn(P) + Sync)) -> AppResult<T> + Send + 'static,
{
    let handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        state.jobs.start(&handle, kind);
        let report = |p: P| {
            if let Some(ch) = &channel {
                // Cửa sổ đã đóng => gửi lỗi, bỏ qua: tác vụ vẫn chạy tiếp.
                let _ = ch.send(p.clone());
            }
            if let Ok(v) = serde_json::to_value(&p) {
                state.jobs.progress(&handle, kind, v);
            }
        };
        let r = f(&state, &report);
        state.jobs.finish(&handle, kind, &r);
        r
    })
    .await;
    match result {
        Ok(r) => r,
        Err(e) => {
            let err = crate::error::task_failed(what, e);
            let failed: AppResult<()> = Err(AppError::Internal(what.to_owned()));
            app.state::<AppState>().jobs.finish(&app, kind, &failed);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_keeps_running_jobs() {
        let jobs = Jobs::default();
        let job = |kind, running| Job { kind, running, started_ms: 0, progress: None, outcome: None };
        jobs.inner.lock_recover().insert("cleaner.scan", (job("cleaner.scan", true), Instant::now()));
        jobs.inner.lock_recover().insert("disk.large", (job("disk.large", false), Instant::now()));
        jobs.ack("cleaner.scan");
        jobs.ack("disk.large");
        let left: Vec<_> = jobs.snapshot().into_iter().map(|j| j.kind).collect();
        assert_eq!(left, vec!["cleaner.scan"], "đang chạy thì không bị xoá");
    }

    #[test]
    fn job_serializes_for_ui() {
        let j = Job {
            kind: "winsxs.clean",
            running: false,
            started_ms: 1,
            progress: Some(serde_json::json!(42.5)),
            outcome: Some(Outcome { ok: false, value: serde_json::to_value(AppError::Busy("x".into())).unwrap() }),
        };
        let v = serde_json::to_value(&j).unwrap();
        assert_eq!(v["kind"], "winsxs.clean");
        assert_eq!(v["startedMs"], 1);
        assert_eq!(v["outcome"]["value"]["code"], "busy");
    }
}
