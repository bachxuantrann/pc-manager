//! Tray icon: chạy nền, menu Boost nhanh / Mở / Thoát.

use std::path::{Path, PathBuf};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::commands::memory::run_boost;
use crate::memory::{BoostMode, BoostTrigger};

const MB: u64 = 1024 * 1024;

pub const MAIN_WINDOW: &str = "main";
const TRAY_ID: &str = "main-tray";

fn set_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}

/// Boost từ menu tray; kết quả hiện ở tooltip (UI nhận event nếu đang mở).
fn quick_boost(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        set_tooltip(&app, "PC Manager — đang Boost…");
        let text = match run_boost(app.clone(), BoostTrigger::Tray, BoostMode::Light).await {
            Ok(r) => format!(
                "PC Manager — RAM khả dụng +{} MB, xoá {} MB cache",
                r.gain.available_gain / MB,
                r.gain.standby_cleared.unwrap_or(0) / MB
            ),
            Err(e) => format!("PC Manager — Boost lỗi: {e}"),
        };
        set_tooltip(&app, &text);
    });
}

/// Đặt mức dùng RAM của giao diện (WebView2). Lỗi (runtime WebView2 cũ…) bị bỏ qua.
pub fn set_ui_memory_low(app: &AppHandle, low: bool) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.with_webview(move |pw| {
            if let Err(e) = crate::sys::webview::set_memory_low(&pw.controller(), low) {
                eprintln!("không đặt được mức RAM WebView2: {e}");
            }
        });
    }
}

/// Thử tạo thư mục và ghi 1 file tạm để biết có dùng được không.
fn dir_is_writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".pcm-write-test");
    let ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Ghi chẩn đoán WebView2 vào `logs\webview.log` cạnh crash.log — trong profile của user đang
/// đăng nhập kể cả khi app được nâng quyền bằng tài khoản admin khác (người dùng mở được).
fn diag(msg: &str) {
    use std::io::Write;
    const MAX_BYTES: u64 = 256 * 1024;
    eprintln!("{msg}");
    let Some(path) = crate::crashlog::log_path().map(|p| p.with_file_name("webview.log")) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Quá lớn => bắt đầu lại (chỉ vài dòng mỗi lần mở cửa sổ, hiếm khi chạm ngưỡng).
    let restart = std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES);
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true);
    if restart {
        opts.write(true).truncate(true);
    } else {
        opts.append(true);
    }
    if let Ok(mut f) = opts.open(&path) {
        let user = std::env::var("USERNAME").unwrap_or_default();
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "[unix {secs}] [{user}] {msg}");
    }
}

/// Các thư mục dữ liệu WebView2 theo thứ tự thử.
///
/// Chạy bằng tài khoản admin KHÁC user đang đăng nhập (máy domain): WebView2 tự hạ quyền về
/// user đang đăng nhập, nên thư mục phải nằm trong profile của user đó — thử nó trước tiên
/// (không kiểm tra ghi được từ đây vì tiến trình admin ghi được cả chỗ user thường không ghi được).
/// Sau đó: mặc định (`None` = để Tauri tự chọn), AppData\Roaming, Temp — chỉ giữ chỗ ghi được.
fn webview_dir_candidates(app: &AppHandle) -> Vec<Option<PathBuf>> {
    let id = &app.config().identifier;
    let paths = app.path();
    let mut out: Vec<Option<PathBuf>> = Vec::new();
    if let Some(local) = crate::sys::session::other_interactive_user_local_appdata() {
        let dir = local.join(id).join("EBWebView");
        diag(&format!("chạy khác user đang đăng nhập => dùng thư mục của user đó: {}", dir.display()));
        out.push(Some(dir));
    }
    match paths.local_data_dir().ok().map(|d| d.join(id).join("EBWebView")) {
        Some(d) if dir_is_writable(&d) => out.push(None),
        Some(d) => diag(&format!("thư mục mặc định không ghi được: {}", d.display())),
        None => out.push(None),
    }
    let mut extra: Vec<PathBuf> = Vec::new();
    if let Ok(d) = paths.data_dir() {
        extra.push(d.join(id).join("EBWebView"));
    }
    extra.push(std::env::temp_dir().join(id).join("EBWebView"));
    for d in extra {
        if dir_is_writable(&d) {
            out.push(Some(d));
        } else {
            diag(&format!("thư mục dự phòng không ghi được: {}", d.display()));
        }
    }
    out
}

/// Hiện cửa sổ chính; nếu đã bị đóng (ẩn xuống tray) thì tạo lại từ cấu hình.
///
/// Đóng cửa sổ = huỷ hẳn WebView2 (các tiến trình msedgewebview2 thoát), nên khi
/// chạy ngầm ở tray chỉ còn tiến trình Rust. Đổi lại, mở lại mất ~1 giây.
pub fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let Some(cfg) = app.config().app.windows.iter().find(|w| w.label == MAIN_WINDOW).cloned() else {
        eprintln!("không có cấu hình cửa sổ '{MAIN_WINDOW}'");
        return;
    };
    // Tham số WebView2 (tắt GPU…) lấy từ `additionalBrowserArgs` trong tauri.conf.json.
    // Máy công ty / tài khoản admin khác có thể chặn thư mục dữ liệu mặc định của WebView2
    // => thử lần lượt các thư mục ghi được; tạo cửa sổ lỗi thì thử thư mục kế tiếp.
    diag(&format!(
        "mở cửa sổ: user={}\\{} profile={}",
        std::env::var("USERDOMAIN").unwrap_or_default(),
        std::env::var("USERNAME").unwrap_or_default(),
        std::env::var("USERPROFILE").unwrap_or_default()
    ));
    for dir in webview_dir_candidates(app) {
        let label = dir.as_deref().map_or_else(|| "(mặc định)".to_owned(), |d| d.display().to_string());
        let mut builder = match tauri::WebviewWindowBuilder::from_config(app, &cfg) {
            Ok(b) => b,
            Err(e) => {
                diag(&format!("không tạo được cửa sổ chính: {e}"));
                return;
            }
        };
        if let Some(d) = dir {
            builder = builder.data_directory(d);
        }
        match builder.build() {
            Ok(w) => {
                diag(&format!("WebView2 chạy với: {label}"));
                let _ = w.set_focus();
                return;
            }
            Err(e) => {
                diag(&format!("tạo WebView2 lỗi với {label}: {e}"));
                if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
                    let _ = w.destroy();
                }
            }
        }
    }
    diag("đã thử hết cách mà vẫn không tạo được cửa sổ chính");
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let boost = MenuItem::with_id(app, "boost", "Boost nhanh (nhẹ)", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Mở PC Manager", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&boost, &open, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("PC Manager")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "boost" => quick_boost(app.clone()),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
