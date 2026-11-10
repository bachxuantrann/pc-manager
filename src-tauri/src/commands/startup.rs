use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::startup::{self, Source, StartupEntry};
use crate::state::AppState;

/// Đọc registry + COM Task Scheduler + .lnk: chạy trên thread blocking riêng
/// (COM được init cho chính thread đó).
#[tauri::command]
pub async fn startup_list(app: AppHandle) -> AppResult<Vec<StartupEntry>> {
    tauri::async_runtime::spawn_blocking(move || startup::list(&app.state::<AppState>().process.publishers))
        .await
        .map_err(|e| crate::error::task_failed("xử lý mục khởi động", e))?
}

#[tauri::command]
pub async fn startup_set_enabled(app: AppHandle, source: Source, name: String, enabled: bool) -> AppResult<()> {
    if name.is_empty() || name.len() > 512 {
        return Err(AppError::InvalidInput("tên không hợp lệ".into()));
    }
    tauri::async_runtime::spawn_blocking(move || {
        startup::set_enabled(&app.state::<AppState>().store, source, &name, enabled)
    })
    .await
    .map_err(|e| crate::error::task_failed("xử lý mục khởi động", e))?
}
