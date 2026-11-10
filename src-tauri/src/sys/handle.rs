//! RAII wrapper cho Win32 `HANDLE`: tự `CloseHandle` khi drop.

use windows::Win32::Foundation::{CloseHandle, HANDLE};

/// Sở hữu duy nhất một `HANDLE` hợp lệ. Đóng handle trong `Drop`.
///
/// Không `Clone`/`Copy` để tránh đóng hai lần.
#[derive(Debug)]
pub struct OwnedHandle(HANDLE);

impl OwnedHandle {
    /// Nhận quyền sở hữu `h`. Trả về `None` nếu handle null/INVALID_HANDLE_VALUE.
    ///
    /// Người gọi phải đảm bảo `h` là handle do mình mở (không phải pseudo-handle
    /// như `GetCurrentProcess()`), và không ai khác sẽ đóng nó.
    pub(crate) fn new(h: HANDLE) -> Option<Self> {
        if h.is_invalid() || h.0.is_null() {
            None
        } else {
            Some(Self(h))
        }
    }

    /// Handle thô để truyền vào API Win32. Không được đóng handle này.
    pub(crate) fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: `self.0` là handle hợp lệ do struct này sở hữu duy nhất
        // (đảm bảo bởi `new`), và chỉ bị đóng đúng một lần tại đây.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

// SAFETY: Win32 kernel handle có thể dùng/đóng từ bất kỳ thread nào.
unsafe impl Send for OwnedHandle {}
// SAFETY: các API nhận `HANDLE` chỉ đọc giá trị handle, an toàn khi chia sẻ.
unsafe impl Sync for OwnedHandle {}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::INVALID_HANDLE_VALUE;

    #[test]
    fn rejects_invalid_handles() {
        assert!(OwnedHandle::new(HANDLE::default()).is_none());
        assert!(OwnedHandle::new(INVALID_HANDLE_VALUE).is_none());
    }
}
