use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::disk::{self, DiskProgress, DriveInfo, DupScan, LargeScan, RecycleReport};
use crate::error::{AppError, AppResult};
use crate::jobs;
use crate::state::AppState;

/// Ngưỡng nhỏ nhất UI được gửi. Ngưỡng quá nhỏ trên ổ vài triệu file sẽ giữ cả danh sách trong RAM.
const MIN_LARGE_BYTES: u64 = 1024 * 1024;
const MIN_DUP_BYTES: u64 = 64 * 1024;

/// Kẹp ngưỡng kích thước nhận từ UI về tối thiểu `floor`.
fn clamp_min(min_bytes: u64, floor: u64) -> u64 {
    min_bytes.max(floor)
}

#[tauri::command]
pub async fn disk_drives() -> AppResult<Vec<DriveInfo>> {
    tauri::async_runtime::spawn_blocking(disk::drives).await.map_err(|e| crate::error::task_failed("xử lý ổ đĩa", e))
}

/// Quét file lớn (chỉ đọc). `root` là thư mục bắt đầu quét.
#[tauri::command]
pub async fn disk_scan_large(app: AppHandle, root: String, min_bytes: u64, on_progress: Channel<DiskProgress>) -> AppResult<LargeScan> {
    let root = disk::validate_root(&root)?;
    jobs::run(app, "disk.large", "xử lý ổ đĩa", Some(on_progress), move |s, report| s.disk.scan_large(&root, clamp_min(min_bytes, MIN_LARGE_BYTES), &|p| report(p))).await
}

/// Tìm file trùng (chỉ đọc): size → blake3 4KB đầu → blake3 toàn bộ.
#[tauri::command]
pub async fn disk_scan_duplicates(app: AppHandle, root: String, min_bytes: u64, on_progress: Channel<DiskProgress>) -> AppResult<DupScan> {
    let root = disk::validate_root(&root)?;
    jobs::run(app, "disk.dups", "xử lý ổ đĩa", Some(on_progress), move |s, report| s.disk.scan_duplicates(&root, clamp_min(min_bytes, MIN_DUP_BYTES), &|p| report(p))).await
}

#[tauri::command]
pub async fn disk_cancel(state: State<'_, AppState>) -> AppResult<()> {
    state.disk.cancel();
    Ok(())
}

/// Chuyển file đã chọn vào Thùng rác. Chỉ nhận `scanId` + id — KHÔNG nhận đường dẫn.
#[tauri::command]
pub async fn disk_recycle(app: AppHandle, scan_id: u64, file_ids: Vec<u32>, on_progress: Channel<DiskProgress>) -> AppResult<RecycleReport> {
    if file_ids.is_empty() || file_ids.len() > 5000 {
        return Err(AppError::InvalidInput("chưa chọn file nào".into()));
    }
    jobs::run(app, "disk.recycle", "xử lý ổ đĩa", Some(on_progress), move |s, report| {
        s.disk.recycle(&s.store, scan_id, &file_ids, &|p| report(p))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_threshold_is_clamped() {
        assert_eq!(clamp_min(0, MIN_DUP_BYTES), 64 * 1024);
        assert_eq!(clamp_min(1, MIN_LARGE_BYTES), 1024 * 1024);
        assert_eq!(clamp_min(500 * 1024 * 1024, MIN_LARGE_BYTES), 500 * 1024 * 1024);
    }
}
