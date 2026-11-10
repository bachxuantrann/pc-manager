//! Dừng / chạy lại một Windows service (cần quyền Administrator).
//! Dùng khi dọn cache mà service đang giữ file (Windows Update, Delivery Optimization).

use std::time::{Duration, Instant};

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Services::{
    CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceStatus, StartServiceW,
    SC_HANDLE, SC_MANAGER_CONNECT, SERVICE_CONTROL_STOP, SERVICE_QUERY_STATUS, SERVICE_START, SERVICE_STATUS,
    SERVICE_STATUS_CURRENT_STATE, SERVICE_STOP, SERVICE_STOPPED,
};

/// RAII cho SC_HANDLE.
struct ScHandle(SC_HANDLE);

impl Drop for ScHandle {
    fn drop(&mut self) {
        // SAFETY: handle do OpenSCManagerW/OpenServiceW trả về, đóng đúng một lần.
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}

fn open(name: &str) -> windows::core::Result<(ScHandle, ScHandle)> {
    // SAFETY: tham số null = máy local, database mặc định.
    let scm = ScHandle(unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)? });
    let wname = HSTRING::from(name);
    // SAFETY: `scm` hợp lệ; `wname` NUL-terminated sống tới hết lời gọi.
    let svc = ScHandle(unsafe { OpenServiceW(scm.0, &wname, SERVICE_QUERY_STATUS | SERVICE_STOP | SERVICE_START)? });
    Ok((scm, svc))
}

fn state(svc: &ScHandle) -> windows::core::Result<SERVICE_STATUS_CURRENT_STATE> {
    let mut st = SERVICE_STATUS::default();
    // SAFETY: handle có quyền QUERY_STATUS; out-pointer hợp lệ.
    unsafe { QueryServiceStatus(svc.0, &mut st)? };
    Ok(st.dwCurrentState)
}

/// Dừng service và chờ tới khi dừng hẳn. Trả về `true` nếu trước đó nó đang chạy
/// (để biết có cần chạy lại không).
pub fn stop(name: &str, timeout: Duration) -> windows::core::Result<bool> {
    let (_scm, svc) = open(name)?;
    if state(&svc)? == SERVICE_STOPPED {
        return Ok(false);
    }
    let mut st = SERVICE_STATUS::default();
    // SAFETY: handle có quyền SERVICE_STOP; out-pointer hợp lệ. Lỗi
    // "đang dừng sẵn" được xử lý bằng vòng chờ bên dưới.
    let _ = unsafe { ControlService(svc.0, SERVICE_CONTROL_STOP, &mut st) };
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if state(&svc)? == SERVICE_STOPPED {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(windows::core::Error::new(
        windows::Win32::Foundation::E_FAIL,
        format!("service {name} không dừng trong {}s", timeout.as_secs()),
    ))
}

pub fn start(name: &str) -> windows::core::Result<()> {
    let (_scm, svc) = open(name)?;
    // SAFETY: handle có quyền SERVICE_START; không truyền tham số.
    unsafe { StartServiceW(svc.0, None) }
}
