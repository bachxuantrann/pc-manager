//! Tauri commands: mỏng — validate input → gọi module → map lỗi.

pub mod apps;
pub mod cleaner;
pub mod disk;
pub mod memory;
pub mod process;
pub mod settings;
pub mod startup;
pub mod update;

use std::time::Duration;

/// Mở một trang Windows Settings cố định (`ms-settings:…`) qua `%SystemRoot%\explorer.exe`. Không nhận URI từ UI.
/// PC Manager đang có quyền admin => mở dưới user đang đăng nhập, quyền thường (cha = Explorer): Settings là app
/// UWP, mở bằng token admin thì trang trống rồi tự đóng.
pub(crate) fn open_ms_settings(uri: &'static str) -> crate::error::AppResult<()> {
    let explorer = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("explorer.exe");
    if crate::sys::token::is_elevated().unwrap_or(true) {
        crate::sys::launch::spawn_as_shell_user(&explorer, uri, &crate::user::environment(), 0)
            .map_err(|e| crate::error::AppError::Other(format!("không mở được Windows Settings: {}", e.message())))?;
        return Ok(());
    }
    std::process::Command::new(explorer).arg(uri).spawn()?;
    Ok(())
}

/// Chạy `f` trên thread blocking với `AppState`; tác vụ panic => lỗi `internal` thân thiện (`what` = đang làm gì).
pub(crate) async fn blocking<T, F>(app: tauri::AppHandle, what: &'static str, f: F) -> crate::error::AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&crate::state::AppState) -> crate::error::AppResult<T> + Send + 'static,
{
    use tauri::Manager;
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<crate::state::AppState>()))
        .await
        .map_err(|e| crate::error::task_failed(what, e))?
}

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store::ActionLogEntry;
use crate::sys;

/// Tham số dòng lệnh: trang mở sẵn sau khi chạy lại với quyền admin.
pub const PAGE_ARG: &str = "--page=";
/// Tham số dòng lệnh: chờ tiến trình cũ (PID) thoát trước khi tạo cửa sổ —
/// tránh 2 WebView2 khác mức quyền dùng chung một thư mục dữ liệu.
pub const WAIT_PID_ARG: &str = "--wait-pid=";

/// Id trang hợp lệ: chữ thường a-z, tối đa 16 ký tự.
fn valid_page(p: &str) -> bool {
    !p.is_empty() && p.len() <= 16 && p.bytes().all(|b| b.is_ascii_lowercase())
}

/// Giá trị của tham số `--xxx=` trên dòng lệnh.
pub fn arg_value(prefix: &str) -> Option<String> {
    std::env::args().find_map(|a| a.strip_prefix(prefix).map(str::to_owned))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub is_elevated: bool,
    pub pid: u32,
    /// Trang cần mở ngay (khi vừa chạy lại với quyền admin).
    pub start_page: Option<String>,
}

#[tauri::command]
pub async fn app_info() -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        is_elevated: sys::token::is_elevated()?,
        pid: std::process::id(),
        start_page: arg_value(PAGE_ARG).filter(|p| valid_page(p)),
    })
}

/// Chạy lại PC Manager với quyền Administrator (Windows hỏi UAC), rồi thoát bản hiện tại.
/// Người dùng bấm No => trả lỗi `Cancelled`, app hiện tại chạy tiếp như cũ.
#[tauri::command]
pub async fn app_relaunch_elevated(app: AppHandle, state: State<'_, AppState>, page: Option<String>) -> AppResult<()> {
    if sys::token::is_elevated()? {
        return Err(AppError::InvalidInput("PC Manager đã chạy với quyền Administrator".into()));
    }
    let exe = std::env::current_exe()?;
    let mut args = format!("{WAIT_PID_ARG}{}", std::process::id());
    if let Some(p) = page.filter(|p| valid_page(p)) {
        args.push_str(&format!(" {PAGE_ARG}{p}"));
    }
    let started = tauri::async_runtime::spawn_blocking(move || sys::shell::run_elevated(&exe, &args))
        .await
        .map_err(|e| crate::error::task_failed("chạy lại với quyền Administrator", e))??;
    state.store.log_action_best_effort(
        "app.elevate",
        "PC Manager",
        started,
        if started { "chạy lại với quyền Administrator" } else { "người dùng huỷ UAC" },
    );
    if !started {
        return Err(AppError::Cancelled);
    }
    // Trả kết quả cho UI trước rồi mới thoát; bản mới chờ PID này thoát rồi mới mở cửa sổ.
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        app.exit(0);
    });
    Ok(())
}

#[tauri::command]
pub async fn recent_actions(state: State<'_, AppState>, limit: u32) -> AppResult<Vec<ActionLogEntry>> {
    state.store.recent_actions(limit.clamp(1, 500))
}

/// Tác vụ dài đang chạy / đã xong mà giao diện chưa nhận (cửa sổ vừa mở lại dùng để nối lại).
#[tauri::command]
pub async fn jobs_snapshot(state: State<'_, AppState>) -> AppResult<Vec<crate::jobs::Job>> {
    Ok(state.jobs.snapshot())
}

/// Giao diện đã nhận kết quả của tác vụ `kind` => backend xoá kết quả đó.
#[tauri::command]
pub async fn jobs_ack(state: State<'_, AppState>, kind: String) -> AppResult<()> {
    if !crate::jobs::KINDS.contains(&kind.as_str()) {
        return Err(AppError::InvalidInput("loại tác vụ không hợp lệ".into()));
    }
    state.jobs.ack(&kind);
    Ok(())
}

/// Cấu hình máy: Windows, CPU, GPU, mainboard, BIOS, RAM, ổ đĩa. Đọc một lần (vài chục ms),
/// không chạy nền, không cần quyền admin.
#[tauri::command]
pub async fn system_info() -> AppResult<crate::hwinfo::SystemInfo> {
    tauri::async_runtime::spawn_blocking(crate::hwinfo::collect)
        .await
        .map_err(|e| crate::error::task_failed("đọc cấu hình máy", e))
}

/// Smart Diagnosis: quét nhanh (chỉ đọc, ~vài trăm ms) hoặc quét sâu (+ phân tích WinSxS bằng DISM).
/// Tiến độ theo bước qua `on_progress`; chạy như tác vụ dài (cửa sổ mở lại vẫn nối lại được).
#[tauri::command]
pub async fn diagnosis_run(
    app: AppHandle,
    deep: bool,
    on_progress: tauri::ipc::Channel<crate::diagnosis::DiagProgress>,
) -> AppResult<crate::diagnosis::Report> {
    crate::jobs::run(app, "diagnosis.scan", "chẩn đoán máy", Some(on_progress), move |s, report| {
        crate::diagnosis::run(s, deep, &|p| report(p))
    })
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn page_ids() {
        assert!(super::valid_page("cleaner"));
        assert!(!super::valid_page("Cleaner"));
        assert!(!super::valid_page("a b"));
        assert!(!super::valid_page(""));
        assert!(!super::valid_page("x\"; calc"));
    }
}
