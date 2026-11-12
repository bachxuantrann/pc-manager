//! Tự cập nhật: kiểm tra bản mới trên GitHub Release và cài khi người dùng bấm (xem `crate::update`).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::LockExt;
use crate::update::{pubkey_configured, UpdateSettings};

/// Bản mới tìm được ở lần kiểm tra gần nhất (để "Cập nhật" cài đúng bản đã hiện cho người dùng).
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    pub version: String,
    /// Nội dung "Có gì mới" (từ `latest.json`).
    pub notes: Option<String>,
    pub date_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub available: Option<Available>,
    pub auto: bool,
    pub last_check_ms: Option<i64>,
    /// Bản build có khoá công khai để kiểm chữ ký (thiếu => không cài được).
    pub configured: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

fn configured(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(pubkey_configured)
}

fn info(app: &AppHandle, s: &UpdateSettings) -> UpdateInfo {
    let available = app.state::<PendingUpdate>().0.lock_recover().as_ref().map(|u| Available {
        version: u.version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
        date_ms: u.date.map(|d| d.unix_timestamp().saturating_mul(1000)),
    });
    UpdateInfo {
        current: app.package_info().version.to_string(),
        available,
        auto: s.auto,
        last_check_ms: s.last_check_ms,
        configured: configured(app),
    }
}

/// Kiểm tra bản mới. `force = false` (tự động khi mở cửa sổ): chỉ hỏi mạng nếu đang bật tự kiểm tra và đã quá
/// 1 ngày từ lần trước; không thì trả trạng thái đã biết.
#[tauri::command]
pub async fn update_check(app: AppHandle, force: bool) -> AppResult<UpdateInfo> {
    let mut s = UpdateSettings::load(&app.state::<AppState>().store);
    let now = crate::settings::now_ms();
    if !force && !s.auto_due(now) {
        return Ok(info(&app, &s));
    }
    let handle = app.clone();
    let checked = app
        .updater_builder()
        // Bộ cài chạy rồi app thoát bằng process::exit: gỡ icon tray trước, ghi nhật ký.
        .on_before_exit(move || {
            handle.state::<AppState>().store.log_action_best_effort("app.update", "PC Manager", true, "đã tải và kiểm chữ ký, chạy bộ cài");
            handle.cleanup_before_exit();
        })
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::Other(format!("không khởi tạo được trình cập nhật: {e}")))?
        .check()
        .await;
    s.last_check_ms = Some(now);
    let _ = s.save(&app.state::<AppState>().store);
    let found = checked.map_err(|e| AppError::Other(format!("không kiểm tra được bản mới (cần mạng tới github.com): {e}")))?;
    *app.state::<PendingUpdate>().0.lock_recover() = found;
    Ok(info(&app, &s))
}

#[tauri::command]
pub async fn update_set_auto(app: AppHandle, enabled: bool) -> AppResult<UpdateInfo> {
    let store = &app.state::<AppState>().store;
    let mut s = UpdateSettings::load(store);
    s.auto = enabled;
    s.save(store)?;
    store.log_action_best_effort("settings.update", "tự kiểm tra cập nhật", true, if enabled { "bật" } else { "tắt" });
    Ok(info(&app, &s))
}

/// Tải bản mới đã tìm thấy, kiểm chữ ký rồi chạy bộ cài (Windows hỏi UAC). Thành công => app thoát và bộ cài
/// tự mở lại app sau khi cài; lệnh này chỉ trả về khi có lỗi (kể cả bấm No ở UAC).
#[tauri::command]
pub async fn update_install(app: AppHandle, on_progress: Channel<UpdateProgress>) -> AppResult<()> {
    if !configured(&app) {
        return Err(AppError::Forbidden(
            "bản PC Manager này chưa được cấu hình khoá ký cập nhật — hãy tải bản mới từ GitHub Release".into(),
        ));
    }
    let update = app
        .state::<PendingUpdate>()
        .0
        .lock_recover()
        .clone()
        .ok_or_else(|| AppError::InvalidInput("chưa có bản mới — hãy bấm Kiểm tra cập nhật".into()))?;
    let store = &app.state::<AppState>().store;
    store.log_action_best_effort("app.update", "PC Manager", true, &format!("tải {} → {}", update.current_version, update.version));
    let mut downloaded = 0u64;
    let mut last = Instant::now() - Duration::from_secs(1);
    let result = update
        .download_and_install(
            |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                // Tối đa ~10 lần/giây.
                if last.elapsed() >= Duration::from_millis(100) || total == Some(downloaded) {
                    last = Instant::now();
                    let _ = on_progress.send(UpdateProgress { downloaded, total });
                }
            },
            || {},
        )
        .await;
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = format!("không cập nhật được lên {}: {e}", update.version);
            store.log_action_best_effort("app.update", "PC Manager", false, &msg);
            Err(AppError::Other(msg))
        }
    }
}
