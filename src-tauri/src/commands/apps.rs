use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::usage::leftovers::{LeftoverReport, LeftoverScan};
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

/// Tìm phần còn sót của app `id`. Backend tự tìm và giữ kết quả; UI chỉ xem.
#[tauri::command]
pub async fn apps_leftovers_scan(app: AppHandle, id: String) -> AppResult<LeftoverScan> {
    if id.is_empty() || id.len() > 300 {
        return Err(AppError::InvalidInput("id không hợp lệ".into()));
    }
    tauri::async_runtime::spawn_blocking(move || {
        crate::sys::process::enter_background_mode();
        app.state::<AppState>().leftovers.scan(&id)
    })
    .await
    .map_err(|e| AppError::Other(format!("leftovers task: {e}")))?
}

/// Chuyển các mục đã chọn vào Thùng rác (key registry: sao lưu .reg rồi xoá).
/// Chỉ nhận `scanId` + id mục — không nhận đường dẫn.
#[tauri::command]
pub async fn apps_leftovers_clean(app: AppHandle, scan_id: u64, item_ids: Vec<u32>) -> AppResult<LeftoverReport> {
    if item_ids.is_empty() || item_ids.len() > 500 {
        return Err(AppError::InvalidInput("chưa chọn mục nào".into()));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.leftovers.clean(&state.store, scan_id, &item_ids)
    })
    .await
    .map_err(|e| AppError::Other(format!("leftovers task: {e}")))?
}
