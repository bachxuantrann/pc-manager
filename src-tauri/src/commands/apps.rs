use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::usage::{self, AppsOverview};

/// Đọc registry + UserAssist + Prefetch và đo thư mục cài (có giới hạn thời gian)
/// trên pool nền ưu tiên thấp.
#[tauri::command]
pub async fn apps_list(app: AppHandle) -> AppResult<AppsOverview> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        crate::bg::pool()?.install(|| usage::list(&state.store))
    })
    .await
    .map_err(|e| AppError::Other(format!("apps task: {e}")))?
}

/// Mở trình gỡ cài đặt. Chỉ nhận `id` (key registry) — lệnh được đọc lại từ registry.
#[tauri::command]
pub async fn apps_uninstall(app: AppHandle, id: String) -> AppResult<String> {
    if id.is_empty() || id.len() > 300 {
        return Err(AppError::InvalidInput("id không hợp lệ".into()));
    }
    tauri::async_runtime::spawn_blocking(move || usage::uninstall(&app.state::<AppState>().store, &id))
        .await
        .map_err(|e| AppError::Other(format!("apps task: {e}")))?
}
