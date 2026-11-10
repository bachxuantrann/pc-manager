use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppResult;
use crate::memory::{
    BoostMode, BoostReport, BoostTrigger, MemorySettings, MemoryStatus, SmartBoostOutcome,
};
use crate::state::AppState;

/// Sự kiện phát ra mỗi khi Boost xong (từ UI, tray hay auto).
pub const EVENT_BOOSTED: &str = "memory://boosted";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOverview {
    pub status: MemoryStatus,
    /// false => không đọc được standby/modified và không Boost được.
    pub privileged: bool,
}

#[tauri::command]
pub async fn memory_status(state: State<'_, AppState>) -> AppResult<MemoryOverview> {
    Ok(MemoryOverview { status: state.memory.snapshot()?, privileged: state.memory.privileged() })
}

/// Chạy Boost trên thread blocking (flush modified có thể mất vài trăm ms).
pub async fn run_boost(app: AppHandle, trigger: BoostTrigger, mode: BoostMode) -> AppResult<BoostReport> {
    let handle = app.clone();
    let report = tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        state.memory.boost(&state.store, trigger, mode)
    })
    .await
    .map_err(|e| crate::error::task_failed("Boost RAM", e))??;
    let _ = app.emit(EVENT_BOOSTED, &report);
    Ok(report)
}

#[tauri::command]
pub async fn memory_boost(app: AppHandle, mode: BoostMode) -> AppResult<BoostReport> {
    run_boost(app, BoostTrigger::Manual, mode).await
}

#[tauri::command]
pub async fn memory_smart_boost(app: AppHandle) -> AppResult<SmartBoostOutcome> {
    let handle = app.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        let threshold = MemorySettings::load(&state.store).threshold();
        state.memory.smart_boost(&state.store, threshold, BoostTrigger::Smart)
    })
    .await
    .map_err(|e| crate::error::task_failed("Smart Boost", e))??;
    if let SmartBoostOutcome::Boosted { report } = &outcome {
        let _ = app.emit(EVENT_BOOSTED, report);
    }
    Ok(outcome)
}

#[tauri::command]
pub async fn memory_settings(state: State<'_, AppState>) -> AppResult<MemorySettings> {
    Ok(MemorySettings::load(&state.store))
}

#[tauri::command]
pub async fn set_memory_settings(
    state: State<'_, AppState>,
    settings: MemorySettings,
) -> AppResult<MemorySettings> {
    settings.save(&state.store)?;
    Ok(MemorySettings::load(&state.store))
}
