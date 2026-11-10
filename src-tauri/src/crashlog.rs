//! Ghi lại mọi panic vào crash.log để chẩn đoán.
//!
//! Release dùng `panic = "unwind"`: panic trong một tác vụ chỉ làm tác vụ đó báo lỗi
//! (xem `sync`, `error::task_failed`), app chạy tiếp. Hook này vẫn ghi nội dung lỗi, vị trí
//! trong code và tên luồng vào `%APPDATA%\com.pcmanager.app\logs\crash.log` (giữ tối đa ~256 KB), rồi chuyển cho
//! hook mặc định.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Trùng `identifier` trong tauri.conf.json (thư mục dữ liệu của app).
const APP_DIR: &str = "com.pcmanager.app";
const MAX_BYTES: u64 = 256 * 1024;

pub fn log_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join(APP_DIR).join("logs").join("crash.log"))
}

/// Dòng ghi cho một panic (tách riêng để test).
pub fn format_entry(unix_secs: u64, version: &str, thread: &str, location: &str, message: &str) -> String {
    format!("[unix {unix_secs}] PC Manager {version} — panic ở luồng '{thread}' tại {location}: {message}\n")
}

pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(không rõ nội dung)".to_owned());
        let location = info.location().map_or_else(|| "?".to_owned(), |l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let thread = std::thread::current().name().unwrap_or("không tên").to_owned();
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let entry = format_entry(secs, env!("CARGO_PKG_VERSION"), &thread, &location, &message);
        if let Some(path) = log_path() {
            let _ = write_entry(&path, &entry);
        }
        default(info);
    }));
}

fn write_entry(path: &std::path::Path, entry: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Quá giới hạn thì bắt đầu file mới (giữ đúng 1 bản cũ).
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(path, path.with_extension("old.log"));
    }
    std::fs::OpenOptions::new().create(true).append(true).open(path)?.write_all(entry.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_format_and_append() {
        let e = format_entry(1_700_000_000, "1.0.1", "bg-0", r"src\usage\userassist.rs:63:50", "byte index 38 is not a char boundary");
        assert!(e.contains("1.0.1") && e.contains("bg-0") && e.contains("userassist.rs:63") && e.ends_with('\n'));

        let dir = std::env::temp_dir().join(format!("pcm-crashlog-{}", std::process::id()));
        let path = dir.join("logs").join("crash.log");
        write_entry(&path, "a\n").unwrap();
        write_entry(&path, "b\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nb\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_lives_in_app_data() {
        if let Some(p) = log_path() {
            assert!(p.ends_with(r"com.pcmanager.app\logs\crash.log"));
        }
    }
}
