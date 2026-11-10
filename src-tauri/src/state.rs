//! State dùng chung giữa các command, tray và thread nền.

use std::path::Path;

use crate::memory::MemoryManager;
use crate::process::ProcessManager;
use crate::store::Store;

pub struct AppState {
    pub store: Store,
    pub memory: MemoryManager,
    pub process: ProcessManager,
}

impl AppState {
    /// Mở DB ở `data_dir`; nếu lỗi thì dùng DB trong RAM để app vẫn chạy.
    pub fn init(data_dir: Option<&Path>) -> Self {
        let store = data_dir
            .and_then(|dir| {
                std::fs::create_dir_all(dir).ok()?;
                match Store::open(&dir.join("pcmanager.db")) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        eprintln!("[state] không mở được DB: {e}");
                        None
                    }
                }
            })
            .or_else(|| Store::in_memory().ok());

        let store = match store {
            Some(s) => s,
            // SQLite in-memory gần như không thể lỗi; nếu có thì không thể chạy tiếp.
            None => {
                eprintln!("[state] không khởi tạo được SQLite");
                std::process::exit(1);
            }
        };
        Self { store, memory: MemoryManager::init(), process: ProcessManager::default() }
    }
}
