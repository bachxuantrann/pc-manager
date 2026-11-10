use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::cleaner::{CleanReport, Progress, ScanResult};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

async fn blocking<T, F>(app: AppHandle, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| AppError::Other(format!("cleaner task: {e}")))?
}

/// Quét theo rule. Tiến độ gửi qua `on_progress` (~10 lần/giây).
#[tauri::command]
pub async fn cleaner_scan(app: AppHandle, on_progress: Channel<Progress>) -> AppResult<ScanResult> {
    blocking(app, move |s| {
        s.cleaner.scan(&|p| {
            let _ = on_progress.send(p);
        })
    })
    .await
}

/// Dọn các rule đã chọn từ lần quét `scan_id`. Chỉ nhận id rule — KHÔNG nhận đường dẫn.
#[tauri::command]
pub async fn cleaner_clean(
    app: AppHandle,
    scan_id: u64,
    rule_ids: Vec<String>,
    dry_run: bool,
    on_progress: Channel<Progress>,
) -> AppResult<CleanReport> {
    if rule_ids.is_empty() || rule_ids.len() > 64 || rule_ids.iter().any(|id| id.is_empty() || id.len() > 64) {
        return Err(AppError::InvalidInput("danh sách mục cần dọn không hợp lệ".into()));
    }
    blocking(app, move |s| {
        s.cleaner.clean(&s.store, scan_id, &rule_ids, dry_run, &|p| {
            let _ = on_progress.send(p);
        })
    })
    .await
}

#[tauri::command]
pub async fn cleaner_cancel(state: State<'_, AppState>) -> AppResult<()> {
    state.cleaner.cancel();
    Ok(())
}
