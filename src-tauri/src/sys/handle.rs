//! RAII wrapper cho Win32 `HANDLE`: tự `CloseHandle` khi drop.
//!
//! Bọc `std::os::windows::io::OwnedHandle` (I/O safety, Rust 1.63+) thay vì tự
//! cài `Drop` + `unsafe impl Send/Sync`: std đã đảm bảo đóng đúng một lần và
//! an toàn đa luồng.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle as StdOwnedHandle};

use windows::Win32::Foundation::HANDLE;

/// Sở hữu duy nhất một `HANDLE` hợp lệ. Không `Clone` để tránh đóng hai lần.
#[derive(Debug)]
pub struct OwnedHandle(StdOwnedHandle);

impl OwnedHandle {
    /// Nhận quyền sở hữu `h`. Trả về `None` nếu handle null/INVALID_HANDLE_VALUE.
    ///
    /// # Safety
    /// `h` phải là handle do chính người gọi mở (không phải pseudo-handle như
    /// `GetCurrentProcess()`), còn mở, và không ai khác sẽ đóng nó.
    pub(crate) unsafe fn from_raw(h: HANDLE) -> Option<Self> {
        if h.is_invalid() || h.0.is_null() {
            return None;
        }
        // SAFETY: theo hợp đồng của hàm, `h` là handle hợp lệ, thuộc sở hữu
        // duy nhất của ta; std sẽ CloseHandle đúng một lần khi drop.
        Some(Self(unsafe { StdOwnedHandle::from_raw_handle(h.0) }))
    }

    /// Handle thô để truyền vào API Win32. Chỉ mượn — không được đóng.
    pub(crate) fn raw(&self) -> HANDLE {
        HANDLE(self.0.as_raw_handle())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::INVALID_HANDLE_VALUE;

    #[test]
    fn rejects_invalid_handles() {
        // SAFETY: handle không hợp lệ bị từ chối trước khi nhận sở hữu.
        unsafe {
            assert!(OwnedHandle::from_raw(HANDLE::default()).is_none());
            assert!(OwnedHandle::from_raw(INVALID_HANDLE_VALUE).is_none());
        }
    }
}
