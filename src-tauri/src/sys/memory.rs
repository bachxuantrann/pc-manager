//! Truy vấn và thao tác bộ nhớ hệ thống (Win32 + NT native API).
//!
//! - `GlobalMemoryStatusEx`, `GetPerformanceInfo`: số liệu cơ bản, không cần quyền.
//! - `NtQuerySystemInformation(SystemMemoryListInformation)`: standby/modified,
//!   cần `SeProfileSingleProcessPrivilege` (admin).
//! - `NtSetSystemInformation(SystemMemoryListInformation, cmd)`: flush/purge.

use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

use super::ntdll::{NtQuerySystemInformation, NtSetSystemInformation, SYSTEM_MEMORY_LIST_INFORMATION};

/// Layout khớp `SYSTEM_MEMORY_LIST_INFORMATION` (phnt / ntexapi.h).
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct SystemMemoryListInformation {
    zero_page_count: usize,
    free_page_count: usize,
    modified_page_count: usize,
    modified_no_write_page_count: usize,
    bad_page_count: usize,
    page_count_by_priority: [usize; 8],
    repurposed_pages_by_priority: [usize; 8],
    modified_page_count_page_file: usize,
}

/// Lệnh cho `NtSetSystemInformation(SystemMemoryListInformation)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MemoryListCommand {
    FlushModifiedList = 3,
    PurgeStandbyList = 4,
    PurgeLowPriorityStandbyList = 5,
}

/// Số liệu cơ bản (byte), không cần quyền admin.
#[derive(Debug, Clone, Copy, Default)]
pub struct BasicMemory {
    pub total_phys: u64,
    pub avail_phys: u64,
    pub load_pct: u32,
    pub commit_total: u64,
    pub commit_limit: u64,
    pub system_cache: u64,
    pub page_size: u64,
}

/// Số trang trên các list của memory manager.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryLists {
    pub zero_pages: u64,
    pub free_pages: u64,
    pub modified_pages: u64,
    pub standby_pages: u64,
}

pub fn basic_memory() -> windows::core::Result<BasicMemory> {
    let mut ms = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `ms` hợp lệ và dwLength đã đặt đúng kích thước theo yêu cầu API.
    unsafe { GlobalMemoryStatusEx(&mut ms)? };

    let mut pi = PERFORMANCE_INFORMATION {
        cb: size_of::<PERFORMANCE_INFORMATION>() as u32,
        ..Default::default()
    };
    // SAFETY: `pi` hợp lệ, `cb` khớp kích thước buffer.
    unsafe { GetPerformanceInfo(&mut pi, pi.cb)? };

    let page = pi.PageSize as u64;
    Ok(BasicMemory {
        total_phys: ms.ullTotalPhys,
        avail_phys: ms.ullAvailPhys,
        load_pct: ms.dwMemoryLoad,
        commit_total: (pi.CommitTotal as u64).saturating_mul(page),
        commit_limit: (pi.CommitLimit as u64).saturating_mul(page),
        system_cache: (pi.SystemCache as u64).saturating_mul(page),
        page_size: page,
    })
}

/// Đọc standby/modified/free list. Cần privilege `SeProfileSingleProcessPrivilege`
/// đã bật (xem `token::enable_privilege`), nếu không sẽ trả lỗi
/// STATUS_PRIVILEGE_NOT_HELD.
pub fn memory_lists() -> windows::core::Result<MemoryLists> {
    let mut info = SystemMemoryListInformation::default();
    let mut ret_len = 0u32;
    // SAFETY: buffer `info` có layout #[repr(C)] khớp struct của kernel, kích
    // thước truyền vào đúng bằng size_of; `ret_len` là out-pointer hợp lệ.
    let status = unsafe {
        NtQuerySystemInformation(
            SYSTEM_MEMORY_LIST_INFORMATION,
            std::ptr::from_mut(&mut info).cast::<c_void>(),
            size_of::<SystemMemoryListInformation>() as u32,
            &mut ret_len,
        )
    };
    status.ok()?;
    Ok(MemoryLists {
        zero_pages: info.zero_page_count as u64,
        free_pages: info.free_page_count as u64,
        modified_pages: info.modified_page_count as u64,
        standby_pages: info.page_count_by_priority.iter().map(|&n| n as u64).sum(),
    })
}

/// Gửi lệnh tới memory manager (flush modified / purge standby).
/// Cần privilege `SeProfileSingleProcessPrivilege`.
pub fn memory_list_command(cmd: MemoryListCommand) -> windows::core::Result<()> {
    let value = cmd as u32;
    // SAFETY: kernel đọc đúng 4 byte (SYSTEM_MEMORY_LIST_COMMAND) từ `value`
    // trên stack; con trỏ sống suốt lời gọi.
    let status = unsafe {
        NtSetSystemInformation(
            SYSTEM_MEMORY_LIST_INFORMATION,
            std::ptr::from_ref(&value).cast::<c_void>(),
            size_of::<u32>() as u32,
        )
    };
    status.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_layout_matches_kernel() {
        // 5 + 8 + 8 + 1 trường ULONG_PTR.
        assert_eq!(size_of::<SystemMemoryListInformation>(), 22 * size_of::<usize>());
    }

    #[test]
    fn basic_memory_is_sane() {
        let m = basic_memory().unwrap();
        assert!(m.total_phys > 0);
        assert!(m.avail_phys <= m.total_phys);
        assert!(m.page_size >= 4096);
        assert!(m.commit_limit >= m.commit_total);
    }
}
