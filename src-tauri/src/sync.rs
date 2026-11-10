//! Chịu lỗi khi một luồng panic (release dùng `panic = "unwind"`).
//!
//! - Mutex bị "nhiễm độc" (poisoned) khi luồng giữ khoá panic. Dữ liệu được khoá ở app này
//!   đều là cache/cờ trạng thái đơn giản, nên lấy lại khoá và chạy tiếp an toàn hơn là
//!   coi khoá hỏng vĩnh viễn (vd. Dọn rác mãi báo lỗi, Ổ đĩa mãi "đang bận").
//! - Vòng lặp của luồng nền bọc trong `survive`: một vòng lỗi thì ghi log (panic hook đã ghi
//!   crash.log) và vòng sau vẫn chạy, thay vì luồng chết hẳn.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, MutexGuard, PoisonError, TryLockError};

pub trait LockExt<T> {
    /// Như `lock()`, nhưng lấy lại khoá nếu bị nhiễm độc.
    fn lock_recover(&self) -> MutexGuard<'_, T>;
    /// Như `try_lock()`: `None` chỉ khi đang có người giữ khoá; khoá nhiễm độc vẫn lấy được.
    fn try_lock_recover(&self) -> Option<MutexGuard<'_, T>>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn try_lock_recover(&self) -> Option<MutexGuard<'_, T>> {
        match self.try_lock() {
            Ok(g) => Some(g),
            Err(TryLockError::Poisoned(p)) => Some(p.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }
}

/// Chạy `f`; nếu panic thì nuốt lại (đã được ghi vào crash.log) và trả `None`.
pub fn survive<R>(what: &str, f: impl FnOnce() -> R) -> Option<R> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => Some(r),
        Err(_) => {
            eprintln!("[{what}] lỗi nội bộ, bỏ qua lượt này");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn poisoned() -> Arc<Mutex<u32>> {
        let m = Arc::new(Mutex::new(7));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("cố ý panic khi giữ khoá");
        })
        .join();
        assert!(m.is_poisoned());
        m
    }

    #[test]
    fn poisoned_mutex_is_recovered() {
        let m = poisoned();
        assert_eq!(*m.lock_recover(), 7);
        assert_eq!(m.try_lock_recover().map(|g| *g), Some(7));
    }

    #[test]
    fn try_lock_recover_reports_busy() {
        let m = Mutex::new(1);
        let _held = m.lock().unwrap();
        assert!(m.try_lock_recover().is_none());
    }

    #[test]
    fn survive_swallows_panic_and_keeps_going() {
        assert_eq!(survive("test", || 5), Some(5));
        assert_eq!(survive("test", || -> u32 { panic!("lỗi một lượt") }), None);
        // Sau panic vẫn gọi tiếp bình thường.
        assert_eq!(survive("test", || "ok"), Some("ok"));
    }
}
