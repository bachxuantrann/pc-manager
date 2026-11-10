//! Thread pool nền dùng chung cho việc quét đĩa (Cleaner, đo dung lượng app…).
//!
//! CLAUDE.md: giới hạn rayon pool = max(2, cpus/2), mỗi thread ở chế độ
//! THREAD_MODE_BACKGROUND (CPU + I/O ưu tiên thấp) để không làm giật máy.

use std::sync::{Arc, OnceLock};

use crate::error::{AppError, AppResult};
use crate::sys;

static POOL: OnceLock<Arc<rayon::ThreadPool>> = OnceLock::new();

pub fn pool() -> AppResult<&'static rayon::ThreadPool> {
    shared().map(|p| &**p)
}

/// Pool dưới dạng `Arc` (jwalk cần để chạy trên pool có sẵn).
pub fn shared() -> AppResult<&'static Arc<rayon::ThreadPool>> {
    if let Some(p) = POOL.get() {
        return Ok(p);
    }
    let cpus = std::thread::available_parallelism().map_or(2, |n| n.get());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads((cpus / 2).max(2))
        .thread_name(|i| format!("bg-{i}"))
        .start_handler(|_| sys::process::enter_background_mode())
        .build()
        .map_err(|e| AppError::Other(format!("không tạo được thread pool: {e}")))?;
    Ok(POOL.get_or_init(|| Arc::new(pool)))
}
