use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::cleaner::system::{SystemCleanReport, WinSxsAnalysis, WindowsOldInfo};
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

/// Kích thước Windows.old (nếu có). Chỉ đọc.
#[tauri::command]
pub async fn system_windows_old_info(app: AppHandle) -> AppResult<WindowsOldInfo> {
    blocking(app, |s| Ok(s.system.windows_old_info())).await
}

/// `DISM /AnalyzeComponentStore` — chỉ đọc, ~1 phút, cần quyền admin.
#[tauri::command]
pub async fn system_analyze_winsxs(app: AppHandle) -> AppResult<WinSxsAnalysis> {
    blocking(app, |s| s.system.analyze_winsxs()).await
}

/// Dọn WinSxS bằng DISM /StartComponentCleanup. Tiến độ % qua `on_progress`.
#[tauri::command]
pub async fn system_clean_winsxs(app: AppHandle, on_progress: Channel<f32>) -> AppResult<SystemCleanReport> {
    blocking(app, move |s| {
        s.system.clean_winsxs(&s.store, &|p| {
            let _ = on_progress.send(p);
        })
    })
    .await
}

/// Xoá Windows.old bằng Disk Cleanup (mục "Previous Installations").
#[tauri::command]
pub async fn system_clean_windows_old(app: AppHandle) -> AppResult<SystemCleanReport> {
    blocking(app, |s| s.system.clean_windows_old(&s.store)).await
}
