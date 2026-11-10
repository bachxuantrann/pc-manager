//! PC Manager — backend Rust.

pub mod commands;
pub mod error;
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

use tauri::WindowEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            tray::setup(app.handle())?;
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
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .build(tauri::generate_context!());

    match result {
        Ok(app) => app.run(|_app, _event| {}),
        Err(e) => {
            eprintln!("Không khởi động được PC Manager: {e}");
            std::process::exit(1);
        }
    }
}
