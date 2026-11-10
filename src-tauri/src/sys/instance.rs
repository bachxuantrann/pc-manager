//! Chỉ cho một bản PC Manager chạy trong mỗi phiên đăng nhập.
//!
//! Bản đầu tiên tạo mutex + event có tên. Bản sau thấy mutex đã tồn tại thì
//! "bật" event (bản đầu hiện cửa sổ) rồi tự thoát.
//!
//! Hai bản có thể khác mức quyền (bản đầu là admin, bản sau mở bằng quyền
//! thường). Đối tượng do tiến trình admin tạo mặc định mang nhãn High integrity
//! nên tiến trình thường không mở được để ghi — vì vậy gắn nhãn Medium
//! (`S:(ML;;NW;;;ME)`) và chỉ cho người dùng đăng nhập tương tác + SYSTEM + Admins.

use std::time::Duration;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{GetLastError, LocalFree, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, HLOCAL, WAIT_OBJECT_0};
use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::System::Threading::{CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE};

use super::OwnedHandle;

const MUTEX_NAME: PCWSTR = w!("Local\\PcManager.SingleInstance");
const EVENT_NAME: PCWSTR = w!("Local\\PcManager.ShowWindow");
/// GA cho Interactive Users, SYSTEM, Administrators; nhãn Medium, chặn ghi từ mức thấp hơn.
const SDDL: PCWSTR = w!("D:(A;;GA;;;IU)(A;;GA;;;SY)(A;;GA;;;BA)S:(ML;;NW;;;ME)");

/// Giữ suốt vòng đời app: mutex đánh dấu "đang chạy", event nhận yêu cầu hiện cửa sổ.
#[derive(Debug)]
pub struct InstanceGuard {
    _mutex: OwnedHandle,
    show: OwnedHandle,
}

#[derive(Debug)]
pub enum Acquire {
    /// Bản đầu tiên — tiếp tục chạy.
    First(InstanceGuard),
    /// Đã có bản khác chạy (đã gửi yêu cầu hiện cửa sổ nếu `notify`).
    AlreadyRunning,
}

/// Security descriptor từ SDDL, tự `LocalFree` khi drop.
struct Sd(PSECURITY_DESCRIPTOR);

impl Sd {
    fn new() -> windows::core::Result<Self> {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        // SAFETY: SDDL là hằng NUL-terminated; `sd` là out-param, hệ thống cấp phát
        // bằng LocalAlloc và ta giải phóng trong Drop.
        unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(SDDL, SDDL_REVISION_1, &mut sd, None)? };
        Ok(Self(sd))
    }

    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0 .0,
            bInheritHandle: false.into(),
        }
    }
}

impl Drop for Sd {
    fn drop(&mut self) {
        // SAFETY: con trỏ do ConvertStringSecurityDescriptorToSecurityDescriptorW cấp
        // bằng LocalAlloc, chỉ giải phóng một lần ở đây.
        unsafe { LocalFree(Some(HLOCAL(self.0 .0))) };
    }
}

/// Báo bản đang chạy hiện cửa sổ.
fn notify_existing() {
    // SAFETY: tên là hằng NUL-terminated.
    if let Ok(raw) = unsafe { OpenEventW(EVENT_MODIFY_STATE, false, EVENT_NAME) } {
        // SAFETY: handle mới mở, ta sở hữu duy nhất.
        if let Some(h) = unsafe { OwnedHandle::from_raw(raw) } {
            // SAFETY: `h` là event hợp lệ có quyền EVENT_MODIFY_STATE.
            let _ = unsafe { SetEvent(h.raw()) };
        }
    }
}

/// Giành quyền "bản duy nhất". `notify`: nếu đã có bản khác thì yêu cầu nó hiện cửa sổ.
pub fn acquire(notify: bool) -> windows::core::Result<Acquire> {
    let sd = Sd::new()?;
    let sa = sd.attributes();
    // SAFETY: `sa` và descriptor bên trong sống tới hết lời gọi; tên NUL-terminated.
    let created = unsafe { CreateMutexW(Some(&sa), false, MUTEX_NAME) };
    // Đọc ngay sau lời gọi: ERROR_ALREADY_EXISTS khi mutex đã có (vẫn trả handle).
    // SAFETY: không tiền điều kiện.
    let last = unsafe { GetLastError() };
    let mutex = match created {
        // SAFETY: handle mới tạo/mở, ta sở hữu duy nhất.
        Ok(raw) => unsafe { OwnedHandle::from_raw(raw) },
        // Mutex của bản admin cũ không có nhãn Medium => tiến trình thường bị từ chối.
        Err(e) if e.code() == ERROR_ACCESS_DENIED.to_hresult() => None,
        Err(e) => return Err(e),
    };
    if mutex.is_none() || last == ERROR_ALREADY_EXISTS {
        if notify {
            notify_existing();
        }
        return Ok(Acquire::AlreadyRunning);
    }
    // SAFETY: như trên. Auto-reset, ban đầu chưa bật.
    let raw = unsafe { CreateEventW(Some(&sa), false, false, EVENT_NAME)? };
    // SAFETY: handle mới tạo, ta sở hữu duy nhất.
    let show = unsafe { OwnedHandle::from_raw(raw) }.ok_or_else(windows::core::Error::from_win32)?;
    let Some(mutex) = mutex else { return Ok(Acquire::AlreadyRunning) };
    Ok(Acquire::First(InstanceGuard { _mutex: mutex, show }))
}

impl InstanceGuard {
    /// Chờ yêu cầu hiện cửa sổ từ một lần mở app khác. `true` nếu có yêu cầu.
    pub fn wait_show(&self, timeout: Duration) -> bool {
        let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: `show` là event hợp lệ, sống tới hết lời gọi.
        unsafe { WaitForSingleObject(self.show.raw(), ms) == WAIT_OBJECT_0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_sees_first() {
        let first = acquire(false).unwrap();
        println!("lần 1: {first:?}");
        assert!(matches!(first, Acquire::First(_)), "lần 1 phải là bản đầu: {first:?}");
        let second = acquire(false).unwrap();
        assert!(matches!(second, Acquire::AlreadyRunning));
        drop(first);
        assert!(matches!(acquire(false).unwrap(), Acquire::First(_)), "nhả xong thì giành lại được");
    }

    #[test]
    fn descriptor_parses() {
        let sd = Sd::new().unwrap();
        assert!(!sd.0 .0.is_null());
    }
}
