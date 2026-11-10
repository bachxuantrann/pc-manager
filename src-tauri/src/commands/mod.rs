//! Tauri commands: mỏng — validate input → gọi module → map lỗi.

use serde::Serialize;

use crate::error::AppResult;
use crate::sys;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub is_elevated: bool,
    pub pid: u32,
}

#[tauri::command]
pub fn app_info() -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        is_elevated: sys::token::is_elevated()?,
        pid: std::process::id(),
    })
}
