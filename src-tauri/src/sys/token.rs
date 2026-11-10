//! Access token của tiến trình hiện tại.

use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::OwnedHandle;

/// Mở token của tiến trình hiện tại với quyền `TOKEN_QUERY`.
fn open_current_token() -> windows::core::Result<OwnedHandle> {
    let mut raw = HANDLE::default();
    // SAFETY: GetCurrentProcess trả pseudo-handle không cần đóng; `raw` là
    // out-pointer hợp lệ trên stack.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw)? };
    OwnedHandle::new(raw).ok_or_else(windows::core::Error::from_win32)
}

/// Tiến trình hiện tại có đang chạy với quyền admin (elevated) không.
pub fn is_elevated() -> windows::core::Result<bool> {
    let token = open_current_token()?;
    let mut elev = TOKEN_ELEVATION::default();
    let mut ret_len = 0u32;
    // SAFETY: buffer `elev` đúng kiểu TOKEN_ELEVATION với kích thước truyền vào
    // khớp; token còn sống trong suốt lời gọi.
    unsafe {
        GetTokenInformation(
            token.raw(),
            TokenElevation,
            Some(&mut elev as *mut _ as *mut c_void),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )?;
    }
    Ok(elev.TokenIsElevated != 0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn is_elevated_does_not_fail() {
        // Kết quả tuỳ terminal chạy test, chỉ cần gọi API thành công.
        assert!(super::is_elevated().is_ok());
    }
}
