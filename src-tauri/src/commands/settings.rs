use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::settings::{self, AutoClean};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRule {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub auto_clean: AutoClean,
    /// Rule được phép chọn cho dọn tự động.
    pub auto_rules: Vec<AutoRule>,
    pub whitelist: Vec<String>,
    pub autostart: bool,
    /// Lý do không được bật khởi động cùng Windows (app không nằm trong Program Files).
    pub autostart_blocked: Option<String>,
    /// App đang chạy với quyền Administrator (cần để bật/tắt khởi động cùng Windows).
    pub elevated: bool,
}

async fn blocking<T, F>(app: AppHandle, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| AppError::Other(format!("settings task: {e}")))?
}

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> AppResult<GeneralSettings> {
    blocking(app, |s| {
        Ok(GeneralSettings {
            auto_clean: AutoClean::load(&s.store),
            auto_rules: s
                .cleaner
                .auto_eligible()
                .into_iter()
                .map(|(id, name, description)| AutoRule { id, name, description })
                .collect(),
            whitelist: settings::load_whitelist(&s.store),
            autostart: settings::autostart_enabled(),
            autostart_blocked: settings::autostart_allowed().err(),
            elevated: crate::sys::token::is_elevated().unwrap_or(false),
        })
    })
    .await
}

/// Lưu lịch dọn tự động. Chỉ giữ rule hợp lệ cho auto (rủi ro thấp, không đụng service).
#[tauri::command]
pub async fn settings_set_auto_clean(app: AppHandle, enabled: bool, interval_days: u32, rule_ids: Vec<String>) -> AppResult<AutoClean> {
    if ![1, 7, 30].contains(&interval_days) || rule_ids.len() > 64 {
        return Err(AppError::InvalidInput("lịch dọn không hợp lệ".into()));
    }
    blocking(app, move |s| {
        let eligible: Vec<String> = s.cleaner.auto_eligible().into_iter().map(|(id, _, _)| id).collect();
        let mut a = AutoClean::load(&s.store);
        a.enabled = enabled;
        a.interval_days = interval_days;
        a.rule_ids = rule_ids.into_iter().filter(|id| eligible.contains(id)).collect();
        a.save(&s.store)?;
        s.store.log_action_best_effort(
            "settings.auto_clean",
            "auto_clean",
            true,
            &format!("{} mỗi {interval_days} ngày: {}", if a.enabled { "bật" } else { "tắt" }, a.rule_ids.join(", ")),
        );
        Ok(a)
    })
    .await
}

#[tauri::command]
pub async fn settings_set_whitelist(app: AppHandle, paths: Vec<String>) -> AppResult<Vec<String>> {
    blocking(app, move |s| settings::save_whitelist(&s.store, &paths)).await
}

#[tauri::command]
pub async fn settings_set_autostart(app: AppHandle, enabled: bool) -> AppResult<bool> {
    blocking(app, move |s| settings::set_autostart(&s.store, enabled)).await
}
