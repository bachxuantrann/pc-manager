use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::disk::{self, DiskProgress, DriveInfo, DupScan, LargeScan, RecycleReport};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

async fn blocking<T, F>(app: AppHandle, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| crate::error::task_failed("xử lý ổ đĩa", e))?
}

#[tauri::command]
pub async fn disk_drives() -> AppResult<Vec<DriveInfo>> {
    tauri::async_runtime::spawn_blocking(disk::drives).await.map_err(|e| crate::error::task_failed("xử lý ổ đĩa", e))
}

/// Quét file lớn (chỉ đọc). `root` là thư mục bắt đầu quét.
#[tauri::command]
pub async fn disk_scan_large(app: AppHandle, root: String, min_bytes: u64, on_progress: Channel<DiskProgress>) -> AppResult<LargeScan> {
    let root = disk::validate_root(&root)?;
    blocking(app, move |s| {
        s.disk.scan_large(&root, min_bytes, &|p| {
            let _ = on_progress.send(p);
        })
    })
    .await
}

/// Tìm file trùng (chỉ đọc): size → blake3 4KB đầu → blake3 toàn bộ.
#[tauri::command]
pub async fn disk_scan_duplicates(app: AppHandle, root: String, min_bytes: u64, on_progress: Channel<DiskProgress>) -> AppResult<DupScan> {
    let root = disk::validate_root(&root)?;
    blocking(app, move |s| {
        s.disk.scan_duplicates(&root, min_bytes, &|p| {
            let _ = on_progress.send(p);
        })
    })
    .await
}

#[tauri::command]
pub async fn disk_cancel(state: State<'_, AppState>) -> AppResult<()> {
    state.disk.cancel();
    Ok(())
}

/// Chuyển file đã chọn vào Thùng rác. Chỉ nhận `scanId` + id — KHÔNG nhận đường dẫn.
#[tauri::command]
pub async fn disk_recycle(app: AppHandle, scan_id: u64, file_ids: Vec<u32>) -> AppResult<RecycleReport> {
    if file_ids.is_empty() || file_ids.len() > 5000 {
        return Err(AppError::InvalidInput("chưa chọn file nào".into()));
    }
    blocking(app, move |s| s.disk.recycle(&s.store, scan_id, &file_ids)).await
}
