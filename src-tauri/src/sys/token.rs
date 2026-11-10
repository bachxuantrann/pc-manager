//! Access token của tiến trình hiện tại: kiểm tra elevated, bật privilege.

use std::ffi::c_void;
use std::mem::size_of;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_NOT_ALL_ASSIGNED, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, GetTokenInformation, LookupPrivilegeValueW, TokenElevation,
    LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, TOKEN_ACCESS_MASK, TOKEN_ADJUST_PRIVILEGES,
    TOKEN_ELEVATION, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::OwnedHandle;

/// Privilege cần cho đọc/purge standby list.
pub const SE_PROFILE_SINGLE_PROCESS: &str = "SeProfileSingleProcessPrivilege";

fn open_current_token(access: TOKEN_ACCESS_MASK) -> windows::core::Result<OwnedHandle> {
    let mut raw = HANDLE::default();
    // SAFETY: GetCurrentProcess trả pseudo-handle không cần đóng; `raw` là
    // out-pointer hợp lệ trên stack.
    unsafe { OpenProcessToken(GetCurrentProcess(), access, &mut raw)? };
    // SAFETY: OpenProcessToken vừa mở `raw` cho ta, chưa ai khác giữ nó.
    unsafe { OwnedHandle::from_raw(raw) }.ok_or_else(windows::core::Error::from_win32)
}

/// Tiến trình hiện tại có đang chạy với quyền admin (elevated) không.
pub fn is_elevated() -> windows::core::Result<bool> {
    let token = open_current_token(TOKEN_QUERY)?;
    let mut elev = TOKEN_ELEVATION::default();
    let mut ret_len = 0u32;
    // SAFETY: buffer `elev` đúng kiểu TOKEN_ELEVATION, kích thước truyền vào
    // khớp; token còn sống trong suốt lời gọi.
    unsafe {
        GetTokenInformation(
            token.raw(),
            TokenElevation,
            Some(std::ptr::from_mut(&mut elev).cast::<c_void>()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )?;
    }
    Ok(elev.TokenIsElevated != 0)
}

/// Bật một privilege cho token của tiến trình hiện tại.
///
/// Trả về `Ok(false)` nếu token không có privilege đó (thường do không chạy admin).
pub fn enable_privilege(name: &str) -> windows::core::Result<bool> {
    let token = open_current_token(TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY)?;
    let name = HSTRING::from(name);
    let mut luid = LUID::default();
    // SAFETY: `name` là chuỗi UTF-16 kết thúc NUL, sống tới hết lời gọi;
    // `luid` là out-pointer hợp lệ.
    unsafe { LookupPrivilegeValueW(PCWSTR::null(), &name, &mut luid)? };

    let tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
    };
    // SAFETY: `tp` là TOKEN_PRIVILEGES hợp lệ với đúng 1 phần tử; không cần
    // trạng thái cũ nên PreviousState = None. GetLastError đọc ngay sau lời gọi
    // trên cùng thread.
    let not_all_assigned = unsafe {
        AdjustTokenPrivileges(token.raw(), false, Some(&tp), 0, None, None)?;
        GetLastError() == ERROR_NOT_ALL_ASSIGNED
    };
    Ok(!not_all_assigned)
}

#[cfg(test)]
mod tests {
    #[test]
    fn is_elevated_does_not_fail() {
        // Kết quả tuỳ terminal chạy test, chỉ cần gọi API thành công.
        assert!(super::is_elevated().is_ok());
    }

    #[test]
    fn enable_privilege_reports_instead_of_failing() {
        // Không admin => Ok(false); admin => Ok(true). Không được Err.
        assert!(super::enable_privilege(super::SE_PROFILE_SINGLE_PROCESS).is_ok());
    }

    #[test]
    fn unknown_privilege_is_error() {
        assert!(super::enable_privilege("SeDoesNotExistPrivilege").is_err());
    }
}
