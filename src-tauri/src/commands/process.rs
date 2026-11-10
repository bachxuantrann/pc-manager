use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::process::{ActionOutcome, ProcessOp, ProcessRef, ProcessSnapshot};
use crate::state::AppState;
use crate::sys::process::PriorityClass;

/// Chạy trên thread blocking: enumerate + mở handle tiến trình mới có thể mất vài chục ms.
async fn blocking<T, F>(app: AppHandle, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| AppError::Other(format!("process task: {e}")))?
}

#[tauri::command]
pub async fn process_list(app: AppHandle) -> AppResult<ProcessSnapshot> {
    blocking(app, |s| s.process.snapshot()).await
}

async fn act(app: AppHandle, target: ProcessRef, op: ProcessOp) -> AppResult<ActionOutcome> {
    blocking(app, move |s| s.process.act(&s.store, &target, op)).await
}

#[tauri::command]
pub async fn process_kill(app: AppHandle, target: ProcessRef) -> AppResult<ActionOutcome> {
    act(app, target, ProcessOp::Kill).await
}

#[tauri::command]
pub async fn process_suspend(app: AppHandle, target: ProcessRef) -> AppResult<ActionOutcome> {
    act(app, target, ProcessOp::Suspend).await
}

#[tauri::command]
pub async fn process_resume(app: AppHandle, target: ProcessRef) -> AppResult<ActionOutcome> {
    act(app, target, ProcessOp::Resume).await
}

#[tauri::command]
pub async fn process_set_priority(
    app: AppHandle,
    target: ProcessRef,
    priority: PriorityClass,
) -> AppResult<ActionOutcome> {
    act(app, target, ProcessOp::SetPriority(priority)).await
}

#[tauri::command]
pub async fn process_trim(app: AppHandle, target: ProcessRef) -> AppResult<ActionOutcome> {
    act(app, target, ProcessOp::Trim).await
}

#[tauri::command]
pub async fn process_open_location(app: AppHandle, target: ProcessRef) -> AppResult<ActionOutcome> {
    act(app, target, ProcessOp::OpenLocation).await
}
