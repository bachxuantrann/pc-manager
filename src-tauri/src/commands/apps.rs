use std::time::{Duration, SystemTime};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::usage::leftovers::{LeftoverReport, LeftoverScan};
use crate::usage::{self, installed, AppsOverview};

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

/// Phát ra khi trình gỡ cài đặt (và mọi tiến trình con) đã kết thúc.
pub const EVENT_UNINSTALL_FINISHED: &str = "apps://uninstall-finished";

/// Trình gỡ chạy quá lâu thì thôi theo dõi.
const UNINSTALL_WATCH_TIMEOUT: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallFinished {
    pub id: String,
    pub name: String,
    /// App vẫn còn trong danh sách đã cài (người dùng huỷ, hoặc trình gỡ lỗi).
    pub still_installed: bool,
    pub timed_out: bool,
}

/// Mở trình gỡ cài đặt. Chỉ nhận `id` (key registry) — lệnh được đọc lại từ registry.
/// Một thread nền chờ trình gỡ chạy xong rồi phát `EVENT_UNINSTALL_FINISHED`
/// để UI mở "Dọn phần còn sót".
#[tauri::command]
pub async fn apps_uninstall(app: AppHandle, id: String) -> AppResult<String> {
    if id.is_empty() || id.len() > 300 {
        return Err(AppError::InvalidInput("id không hợp lệ".into()));
    }
    let handle = app.clone();
    let started = tauri::async_runtime::spawn_blocking(move || usage::uninstall(&handle.state::<AppState>().store, &id))
        .await
        .map_err(|e| AppError::Other(format!("apps task: {e}")))??;
    let started_at = SystemTime::now();
    let name = started.app.name.clone();
    // Chụp lại thông tin app: trình gỡ sẽ xoá key registry, nhưng vẫn cần biết thư mục cài để dọn sót.
    app.state::<AppState>().leftovers.remember(started.app.clone());
    let spawned = std::thread::Builder::new().name("uninstall-watch".into()).spawn(move || {
        crate::sys::process::enter_background_mode();
        let finished = usage::wait_uninstall(started.pid, started_at, UNINSTALL_WATCH_TIMEOUT);
        // Trình gỡ thường xoá key ngay trước khi thoát; chờ chút cho registry ổn định.
        std::thread::sleep(Duration::from_millis(1500));
        let payload = UninstallFinished {
            still_installed: installed::find(&started.app.id).is_some(),
            id: started.app.id,
            name: started.app.name,
            timed_out: !finished,
        };
        let detail = if payload.timed_out { "quá thời gian chờ" } else if payload.still_installed { "app vẫn còn" } else { "đã gỡ" };
        app.state::<AppState>().store.log_action_best_effort("app.uninstall.finished", &payload.name, !payload.still_installed, detail);
        let _ = app.emit(EVENT_UNINSTALL_FINISHED, &payload);
    });
    if let Err(e) = spawned {
        eprintln!("không tạo được thread theo dõi gỡ cài đặt: {e}");
    }
    Ok(name)
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
