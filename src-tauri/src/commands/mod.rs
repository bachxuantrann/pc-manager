//! Tauri commands: mỏng — validate input → gọi module → map lỗi.

pub mod memory;

use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::state::AppState;
use crate::store::ActionLogEntry;
use crate::sys;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub is_elevated: bool,
    pub pid: u32,
}

#[tauri::command]
pub async fn app_info() -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        is_elevated: sys::token::is_elevated()?,
        pid: std::process::id(),
    })
}

#[tauri::command]
pub async fn recent_actions(state: State<'_, AppState>, limit: u32) -> AppResult<Vec<ActionLogEntry>> {
    state.store.recent_actions(limit.clamp(1, 500))
}
