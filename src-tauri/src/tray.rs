//! Tray icon: chạy nền, menu Boost nhanh / Mở / Thoát.

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
    match tauri::WebviewWindowBuilder::from_config(app, &cfg).and_then(|b| b.build()) {
        Ok(w) => {
            let _ = w.set_focus();
        }
        Err(e) => eprintln!("không tạo được cửa sổ chính: {e}"),
    }
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
