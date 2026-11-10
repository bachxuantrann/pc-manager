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
pub mod error;
pub mod settings;
pub mod state;
pub mod sys;
mod tray;

// Các module chức năng — được lấp đầy qua từng giai đoạn (xem PROMPTS.md).
pub mod cleaner;
pub mod disk;
pub mod memory;
pub mod process;
pub mod startup;
pub mod store;
pub mod usage;

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, RunEvent};

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
            let state = app.state::<AppState>();
            if let Some(report) = state.memory.auto_tick(&state.store, AUTO_SMART_COOLDOWN) {
                let _ = app.emit(commands::memory::EVENT_BOOSTED, &report);
            }
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
                sampler.tick(&app.state::<AppState>().store);
                std::thread::sleep(USAGE_SAMPLE_INTERVAL);
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| eprintln!("không tạo được thread usage sampler: {e}"));
}

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
            let result = state.cleaner.auto_clean(&state.store, &cfg.rule_ids);
            let detail = match &result {
                Ok(r) => format!("{} file, {} byte", r.deleted_files, r.freed_bytes),
                Err(e) => e.to_string(),
            };
            state.store.log_action_best_effort("cleaner.auto", &cfg.rule_ids.join(","), result.is_ok(), &detail);
            cfg.last_run_ms = Some(settings::now_ms());
            let _ = cfg.save(&state.store);
        })
        .map(|_| ())
        .unwrap_or_else(|e| eprintln!("không tạo được thread auto clean: {e}"));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir().ok();
            app.manage(AppState::init(data_dir.as_deref()));
            tray::setup(app.handle())?;
            spawn_auto_smart_boost(app.handle().clone());
            spawn_usage_sampler(app.handle().clone());
            spawn_auto_clean(app.handle().clone());
            // Cửa sổ có `"create": false` trong tauri.conf.json: chỉ tạo khi cần.
            // Khởi động cùng Windows (task với --tray) => không tạo WebView, chạy ngầm ở tray.
            if !std::env::args().any(|a| a == settings::TRAY_ARG) {
                tray::show_main_window(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::recent_actions,
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
