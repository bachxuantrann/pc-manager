//! PC Manager — backend Rust.

// Quy tắc CLAUDE.md được compiler kiểm tra thay vì chỉ dựa vào review:
// - `unsafe` chỉ được phép trong `sys/` (module đó tự `allow`).
// - mọi block unsafe phải có `// SAFETY:`.
// - không `unwrap`/`expect` ngoài test.
#![deny(unsafe_code)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod bg;
pub mod commands;
pub mod crashlog;
pub mod diagnosis;
pub mod error;
pub mod hwinfo;
pub mod jobs;
pub mod settings;
pub mod state;
pub mod sync;
pub mod sys;
pub mod user;
mod tray;

// Các module chức năng.
pub mod cleaner;
pub mod disk;
pub mod memory;
pub mod process;
pub mod startup;
pub mod store;
pub mod usage;

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

use crate::state::AppState;

/// Chu kỳ kiểm tra auto Smart Boost và thời gian nghỉ giữa 2 lần Boost tự động.
const AUTO_SMART_INTERVAL: Duration = Duration::from_secs(60);
const AUTO_SMART_COOLDOWN: Duration = Duration::from_secs(5 * 60);

/// Thread nền nhẹ: chỉ gọi GlobalMemoryStatusEx mỗi 60s khi người dùng bật auto.
fn spawn_auto_smart_boost(app: AppHandle) {
    std::thread::Builder::new()
        .name("auto-smart-boost".into())
        .spawn(move || loop {
            std::thread::sleep(AUTO_SMART_INTERVAL);
            // Một lượt lỗi không làm chết luồng (sync::survive).
            sync::survive("auto-smart-boost", || {
                let state = app.state::<AppState>();
                if let Some(report) = state.memory.auto_tick(&state.store, AUTO_SMART_COOLDOWN) {
                    let _ = app.emit(commands::memory::EVENT_BOOSTED, &report);
                }
            });
        })
        .map(|_| ())
        .unwrap_or_else(|e| eprintln!("không tạo được thread auto smart boost: {e}"));
}

/// Lấy mẫu tiến trình đang chạy mỗi 60s (kể cả khi cửa sổ ẩn ở tray) để biết
/// "lần dùng cuối" của ứng dụng. Chỉ mở handle cho tiến trình mới.
const USAGE_SAMPLE_INTERVAL: Duration = Duration::from_secs(60);

fn spawn_usage_sampler(app: AppHandle) {
    std::thread::Builder::new()
        .name("usage-sampler".into())
        .spawn(move || {
            sys::process::enter_background_mode();
            let mut sampler = usage::sampler::Sampler::default();
            loop {
                sync::survive("usage-sampler", || sampler.tick(&app.state::<AppState>().store));
                std::thread::sleep(USAGE_SAMPLE_INTERVAL);
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| eprintln!("không tạo được thread usage sampler: {e}"));
}

/// Chờ sau khi đóng cửa sổ rồi mới thu gọn bộ nhớ (để WebView2 giải phóng xong).
const TRIM_AFTER_CLOSE: Duration = Duration::from_secs(5);

/// Kiểm tra lịch dọn tự động mỗi 30 phút (rất nhẹ: chỉ đọc 1 dòng SQLite).
const AUTO_CLEAN_CHECK: Duration = Duration::from_secs(30 * 60);

fn spawn_auto_clean(app: AppHandle) {
    std::thread::Builder::new()
        .name("auto-clean".into())
        .spawn(move || loop {
            std::thread::sleep(AUTO_CLEAN_CHECK);
            let state = app.state::<AppState>();
            let mut cfg = settings::AutoClean::load(&state.store);
            if !cfg.due(settings::now_ms()) {
                continue;
            }
            // Ghi mốc chạy TRƯỚC khi dọn: nếu lượt này lỗi thì đợi tới kỳ sau, không lặp lỗi mỗi 30 phút.
            let previous_run = cfg.last_run_ms;
            cfg.last_run_ms = Some(settings::now_ms());
            let _ = cfg.save(&state.store);
            let result = sync::survive("auto-clean", || state.cleaner.auto_clean(&state.store, &cfg.rule_ids))
                .unwrap_or_else(|| Err(error::AppError::Internal("dọn rác tự động".into())));
            // Người dùng đang quét/dọn tay => lượt này chưa chạy: trả lại mốc cũ để lần kiểm tra sau
            // (30 phút) thử lại, thay vì bỏ cả kỳ (1/7/30 ngày). Đọc lại cài đặt vì có thể vừa bị đổi.
            if matches!(result, Err(error::AppError::Busy(_))) {
                let mut latest = settings::AutoClean::load(&state.store);
                latest.last_run_ms = previous_run;
                let _ = latest.save(&state.store);
                continue;
            }
            let detail = match &result {
                Ok(r) => format!("{} file, {} byte", r.deleted_files, r.freed_bytes),
                Err(e) => e.to_string(),
            };
            state.store.log_action_best_effort("cleaner.auto", &cfg.rule_ids.join(","), result.is_ok(), &detail);
        })
        .map(|_| ())
        .unwrap_or_else(|e| eprintln!("không tạo được thread auto clean: {e}"));
}

/// Luồng nhận yêu cầu "hiện cửa sổ" khi người dùng mở app lần nữa.
fn spawn_show_listener(app: AppHandle, guard: sys::instance::InstanceGuard) {
    std::thread::Builder::new()
        .name("single-instance".into())
        .spawn(move || loop {
            if guard.wait_show(Duration::from_secs(3600)) {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    sync::survive("show-window", || tray::show_main_window(&handle));
                });
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| eprintln!("không tạo được thread single-instance: {e}"));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Ghi mọi panic ra crash.log để chẩn đoán (app chạy tiếp nhờ panic = "unwind").
    crashlog::install();
    let tray_only = std::env::args().any(|a| a == settings::TRAY_ARG);
    // Vừa được chạy lại với quyền admin: chờ bản cũ thoát hẳn (nhả mutex, và WebView2
    // của nó dùng chung thư mục dữ liệu) rồi mới khởi động.
    if let Some(pid) = commands::arg_value(commands::WAIT_PID_ARG).and_then(|p| p.parse::<u32>().ok()) {
        sys::process::wait_exit(pid, Duration::from_secs(10));
    }
    // Chỉ một bản chạy: đã có bản khác => nhờ nó hiện cửa sổ (trừ khi khởi động ngầm) rồi thoát.
    let guard = match sys::instance::acquire(!tray_only) {
        Ok(sys::instance::Acquire::First(g)) => Some(g),
        Ok(sys::instance::Acquire::AlreadyRunning) => return,
        Err(e) => {
            eprintln!("không kiểm tra được bản đang chạy: {e}");
            None
        }
    };

    let result = tauri::Builder::default()
        .setup(move |app| {
            // Nâng quyền bằng tài khoản admin khác (máy domain): tài khoản đó chỉ để lấy quyền,
            // cài đặt/lịch sử vẫn thuộc user đang đăng nhập => dùng AppData\Roaming của user đó.
            let data_dir = sys::session::other_interactive_user_roaming_appdata()
                .map(|d| d.join(&app.config().identifier))
                .or_else(|| app.path().app_data_dir().ok());
            app.manage(AppState::init(data_dir.as_deref()));
            tray::setup(app.handle())?;
            spawn_auto_smart_boost(app.handle().clone());
            spawn_usage_sampler(app.handle().clone());
            spawn_auto_clean(app.handle().clone());
            if let Some(g) = guard {
                spawn_show_listener(app.handle().clone(), g);
            }
            // Cửa sổ có `"create": false` trong tauri.conf.json: chỉ tạo khi cần.
            // Khởi động cùng Windows (task với --tray) => không tạo WebView, chạy ngầm ở tray.
            if !tray_only {
                tray::show_main_window(app.handle());
            }
            Ok(())
        })
        // Cửa sổ chính bị đóng (xuống tray): chờ WebView2 dọn xong rồi thu gọn bộ nhớ của app.
        .on_window_event(|window, event| {
            // Cửa sổ mất focus (chuyển sang app khác, thu nhỏ) => giao diện nhả RAM; quay lại => bình thường.
            if let WindowEvent::Focused(focused) = event {
                if window.label() == tray::MAIN_WINDOW {
                    tray::set_ui_memory_low(window.app_handle(), !focused);
                }
            }
            if matches!(event, WindowEvent::Destroyed) && window.label() == tray::MAIN_WINDOW {
                let app = window.app_handle().clone();
                let _ = std::thread::Builder::new().name("trim-self".into()).spawn(move || {
                    std::thread::sleep(TRIM_AFTER_CLOSE);
                    // Người dùng đã mở lại cửa sổ trong lúc chờ => không thu gọn.
                    if app.get_webview_window(tray::MAIN_WINDOW).is_none() {
                        if let Err(e) = sys::process::trim_self() {
                            eprintln!("không thu gọn được bộ nhớ: {e}");
                        }
                    }
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::app_relaunch_elevated,
            commands::recent_actions,
            commands::jobs_snapshot,
            commands::jobs_ack,
            commands::system_info,
            commands::diagnosis_run,
            commands::memory::memory_status,
            commands::memory::memory_boost,
            commands::memory::memory_smart_boost,
            commands::memory::memory_settings,
            commands::memory::set_memory_settings,
            commands::process::process_list,
            commands::process::process_kill,
            commands::process::process_suspend,
            commands::process::process_resume,
            commands::process::process_set_priority,
            commands::process::process_trim,
            commands::process::process_open_location,
            commands::startup::startup_list,
            commands::startup::startup_set_enabled,
            commands::cleaner::cleaner_scan,
            commands::cleaner::cleaner_clean,
            commands::cleaner::cleaner_cancel,
            commands::cleaner::system_windows_old_info,
            commands::cleaner::system_analyze_winsxs,
            commands::cleaner::system_clean_winsxs,
            commands::cleaner::system_clean_windows_old,
            commands::apps::apps_list,
            commands::apps::apps_uninstall,
            commands::apps::apps_leftovers_scan,
            commands::apps::apps_leftovers_clean,
            commands::disk::disk_drives,
            commands::disk::disk_scan_large,
            commands::disk::disk_scan_duplicates,
            commands::disk::disk_cancel,
            commands::disk::disk_recycle,
            commands::settings::settings_get,
            commands::settings::settings_set_auto_clean,
            commands::settings::settings_set_whitelist,
            commands::settings::settings_set_autostart,
        ])
        .build(tauri::generate_context!());

    match result {
        // Bấm X => cửa sổ đóng hẳn (WebView2 được giải phóng) nhưng app vẫn chạy ở tray.
        // Chỉ thoát khi gọi `app.exit(..)` (menu tray "Thoát") — lúc đó `code` có giá trị.
        Ok(app) => app.run(|_app, event| {
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        }),
        Err(e) => {
            eprintln!("Không khởi động được PC Manager: {e}");
            std::process::exit(1);
        }
    }
}
