//! Các hàm NT native không có (hoặc không ổn định) trong windows-rs.
//! `raw-dylib`: không cần ntdll.lib lúc link.

use std::ffi::c_void;

use windows::Win32::Foundation::{HANDLE, NTSTATUS};

/// `SYSTEM_INFORMATION_CLASS`.
pub const SYSTEM_PROCESS_INFORMATION: u32 = 5;
pub const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 80;

/// STATUS_INFO_LENGTH_MISMATCH — buffer quá nhỏ.
pub const STATUS_INFO_LENGTH_MISMATCH: NTSTATUS = NTSTATUS(0xC000_0004_u32 as i32);

#[link(name = "ntdll", kind = "raw-dylib")]
extern "system" {
    pub fn NtQuerySystemInformation(
        class: u32,
        info: *mut c_void,
        info_len: u32,
        return_len: *mut u32,
    ) -> NTSTATUS;
    pub fn NtSetSystemInformation(class: u32, info: *const c_void, info_len: u32) -> NTSTATUS;
    pub fn NtSuspendProcess(process: HANDLE) -> NTSTATUS;
    pub fn NtResumeProcess(process: HANDLE) -> NTSTATUS;
}
