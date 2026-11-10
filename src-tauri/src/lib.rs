//! PC Manager — backend Rust.

// Quy tắc CLAUDE.md được compiler kiểm tra thay vì chỉ dựa vào review:
// - `unsafe` chỉ được phép trong `sys/` (module đó tự `allow`).
// - mọi block unsafe phải có `// SAFETY:`.
// - không `unwrap`/`expect` ngoài test.
#![deny(unsafe_code)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod commands;
pub mod error;
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

use tauri::{AppHandle, Emitter, Manager, WindowEvent};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir().ok();
            app.manage(AppState::init(data_dir.as_deref()));
            tray::setup(app.handle())?;
            spawn_auto_smart_boost(app.handle().clone());
            Ok(())
        })
        // Bấm X => ẩn xuống tray thay vì thoát. Thoát qua menu tray.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == tray::MAIN_WINDOW {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
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
        ])
        .build(tauri::generate_context!());

    match result {
        Ok(app) => app.run(|_app, _event| {}),
        Err(e) => {
            eprintln!("Không khởi động được PC Manager: {e}");
            std::process::exit(1);
        }
    }
}
