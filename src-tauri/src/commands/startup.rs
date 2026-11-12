use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::startup::{self, Source, StartupEntry};
use crate::state::AppState;

/// Đọc registry + COM Task Scheduler + .lnk: chạy trên thread blocking riêng
/// (COM được init cho chính thread đó).
#[tauri::command]
pub async fn startup_list(app: AppHandle) -> AppResult<Vec<StartupEntry>> {
    tauri::async_runtime::spawn_blocking(move || startup::list(&app.state::<AppState>().process.publishers))
        .await
        .map_err(|e| crate::error::task_failed("xử lý mục khởi động", e))?
}

#[tauri::command]
pub async fn startup_set_enabled(app: AppHandle, source: Source, name: String, enabled: bool) -> AppResult<()> {
    if name.is_empty() || name.len() > 512 {
        return Err(AppError::InvalidInput("tên không hợp lệ".into()));
    }
    tauri::async_runtime::spawn_blocking(move || {
        startup::set_enabled(&app.state::<AppState>().store, source, &name, enabled)
    })
    .await
    .map_err(|e| crate::error::task_failed("xử lý mục khởi động", e))?
}

/// Mở tab Startup của Task Manager (bật/tắt app Store tự chạy — PC Manager chỉ đọc nguồn này).
/// Không dùng Settings → Apps → Startup: trang đó crash trên một số bản Windows 11 (SettingsHandlers_Startup.dll,
/// 0xc0000005, gặp ở 26100.8521). Mở qua `cmd /c start` (ShellExecute) để Windows tự xử lý quyền của Taskmgr
/// (manifest highestAvailable); PC Manager đang có quyền admin => chạy dưới user đang đăng nhập, nếu không Task
/// Manager hiện mục khởi động của tài khoản admin.
#[tauri::command]
pub async fn startup_open_settings() -> AppResult<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let sys32 = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32");
    let cmd = sys32.join("cmd.exe");
    let args = format!("/d /c start \"\" \"{}\" /0 /startup", sys32.join("Taskmgr.exe").display());
    if crate::sys::token::is_elevated().unwrap_or(true) {
        crate::sys::launch::spawn_as_shell_user(&cmd, &args, &crate::user::environment(), CREATE_NO_WINDOW)
            .map_err(|e| AppError::Other(format!("không mở được Task Manager: {}", e.message())))?;
    } else {
        std::process::Command::new(cmd).raw_arg(&args).creation_flags(CREATE_NO_WINDOW).spawn()?;
    }
    Ok(())
}
