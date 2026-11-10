//! Tiến trình: liệt kê qua `NtQuerySystemInformation(SystemProcessInformation)`
//! và thao tác trên một handle đã xác minh danh tính (pid + thời điểm tạo).

use std::ffi::c_void;
use std::mem::size_of;
use std::path::PathBuf;

use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{FILETIME, HANDLE};
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::Threading::{
    GetCurrentThread, GetProcessTimes, IsProcessCritical, OpenProcess, QueryFullProcessImageNameW,
    SetPriorityClass, SetThreadPriority, TerminateProcess, ABOVE_NORMAL_PRIORITY_CLASS,
    BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    PROCESS_ACCESS_RIGHTS, PROCESS_CREATION_FLAGS, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_SET_QUOTA,
    PROCESS_SUSPEND_RESUME, PROCESS_TERMINATE, THREAD_MODE_BACKGROUND_BEGIN,
};

use super::ntdll::{
    NtQuerySystemInformation, NtResumeProcess, NtSuspendProcess, STATUS_INFO_LENGTH_MISMATCH,
    SYSTEM_PROCESS_INFORMATION,
};
use super::OwnedHandle;

// ---------------------------------------------------------------------------
// Layout NT (x64). Khớp phnt/ntexapi.h; kích thước được kiểm tra trong test.

#[repr(C)]
#[derive(Clone, Copy)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SystemProcessInformation {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name: UnicodeString,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
    handle_count: u32,
    session_id: u32,
    unique_process_key: usize,
    peak_virtual_size: usize,
    virtual_size: usize,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_page_count: usize,
    read_operation_count: i64,
    write_operation_count: i64,
    other_operation_count: i64,
    read_transfer_count: i64,
    write_transfer_count: i64,
    other_transfer_count: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SystemThreadInformation {
    kernel_time: i64,
    user_time: i64,
    create_time: i64,
    wait_time: u32,
    start_address: usize,
    client_id: [usize; 2],
    priority: i32,
    base_priority: i32,
    context_switches: u32,
    thread_state: u32,
    wait_reason: u32,
}

/// KTHREAD_STATE::Waiting và KWAIT_REASON::Suspended.
const THREAD_STATE_WAITING: u32 = 5;
const WAIT_REASON_SUSPENDED: u32 = 5;

// ---------------------------------------------------------------------------

/// Một dòng thô từ kernel — chưa có đường dẫn / publisher.
#[derive(Debug, Clone)]
pub struct RawProcess {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    /// FILETIME (100ns từ 1601) — cùng `pid` tạo thành danh tính duy nhất.
    pub create_time: u64,
    /// user + kernel, đơn vị 100ns.
    pub cpu_time: u64,
    pub private_ws: u64,
    pub working_set: u64,
    pub threads: u32,
    pub handles: u32,
    pub session_id: u32,
    pub base_priority: i32,
    /// Tất cả thread đang chờ với lý do Suspended.
    pub suspended: bool,
}

/// Chụp danh sách tiến trình.
pub fn enumerate() -> windows::core::Result<Vec<RawProcess>> {
    // Buffer kiểu u64 để được căn 8 byte như kernel yêu cầu.
    let mut buf: Vec<u64> = vec![0; 64 * 1024];
    loop {
        let byte_len = u32::try_from(buf.len() * 8).unwrap_or(u32::MAX);
        let mut needed = 0u32;
        // SAFETY: `buf` có đúng `byte_len` byte ghi được, căn 8; `needed` là
        // out-pointer hợp lệ.
        let status = unsafe {
            NtQuerySystemInformation(
                SYSTEM_PROCESS_INFORMATION,
                buf.as_mut_ptr().cast::<c_void>(),
                byte_len,
                &mut needed,
            )
        };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            // Số tiến trình có thể tăng giữa 2 lần gọi → chừa dư 64 KB.
            let words = (needed as usize + 64 * 1024).div_ceil(8);
            buf.resize(words.max(buf.len() * 2), 0);
            continue;
        }
        status.ok()?;
        break;
    }
    let bytes: Vec<u8> = buf.iter().flat_map(|w| w.to_ne_bytes()).collect();
    Ok(parse(&bytes, buf.as_ptr() as usize))
}

/// Phân tích buffer. `base_addr` là địa chỉ gốc của buffer kernel đã ghi —
/// con trỏ `ImageName.Buffer` trỏ vào vùng đó, ta quy đổi về offset trong
/// `bytes` và kiểm tra biên trước khi đọc (unsafe-checker ffi-15).
fn parse(bytes: &[u8], base_addr: usize) -> Vec<RawProcess> {
    let mut out = Vec::with_capacity(400);
    let mut offset = 0usize;
    while let Some(entry) = read_at::<SystemProcessInformation>(bytes, offset) {
        let threads_at = offset + size_of::<SystemProcessInformation>();
        let n_threads = entry.number_of_threads as usize;
        let suspended = n_threads > 0
            && (0..n_threads).all(|i| {
                read_at::<SystemThreadInformation>(bytes, threads_at + i * size_of::<SystemThreadInformation>())
                    .is_some_and(|t| t.thread_state == THREAD_STATE_WAITING && t.wait_reason == WAIT_REASON_SUSPENDED)
            });

        let pid = entry.unique_process_id as u32;
        let name = match (pid, image_name(bytes, base_addr, entry.image_name)) {
            (0, _) => "System Idle Process".to_owned(),
            (_, Some(n)) if !n.is_empty() => n,
            (4, _) => "System".to_owned(),
            (_, _) => format!("<pid {pid}>"),
        };
        out.push(RawProcess {
            pid,
            ppid: entry.inherited_from_unique_process_id as u32,
            name,
            create_time: entry.create_time.max(0) as u64,
            cpu_time: (entry.user_time.max(0) as u64).saturating_add(entry.kernel_time.max(0) as u64),
            private_ws: entry.working_set_private_size.max(0) as u64,
            working_set: entry.working_set_size as u64,
            threads: entry.number_of_threads,
            handles: entry.handle_count,
            session_id: entry.session_id,
            base_priority: entry.base_priority,
            suspended,
        });

        if entry.next_entry_offset == 0 {
            break;
        }
        offset += entry.next_entry_offset as usize;
    }
    out
}

/// Đọc một struct `T` (Copy, POD) tại `offset` nếu nằm trọn trong `bytes`.
fn read_at<T: Copy>(bytes: &[u8], offset: usize) -> Option<T> {
    let end = offset.checked_add(size_of::<T>())?;
    let slice = bytes.get(offset..end)?;
    // SAFETY: `slice` có đủ size_of::<T>() byte; read_unaligned không yêu cầu
    // căn lề. Chỉ dùng cho struct #[repr(C)] gồm số nguyên/con trỏ thô — mọi
    // mẫu bit đều hợp lệ (con trỏ không bao giờ bị deref trực tiếp).
    Some(unsafe { slice.as_ptr().cast::<T>().read_unaligned() })
}

fn image_name(bytes: &[u8], base_addr: usize, us: UnicodeString) -> Option<String> {
    if us.buffer.is_null() || us.length == 0 {
        return None;
    }
    let start = (us.buffer as usize).checked_sub(base_addr)?;
    let raw = bytes.get(start..start.checked_add(us.length as usize)?)?;
    let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_ne_bytes([c[0], c[1]])).collect();
    Some(String::from_utf16_lossy(&units))
}

// ---------------------------------------------------------------------------
// Thao tác trên một tiến trình cụ thể.

/// Handle tới tiến trình đã được xác minh đúng `create_time` (chống PID reuse).
pub struct ProcessHandle(OwnedHandle);

impl ProcessHandle {
    /// Mở tiến trình và kiểm tra thời điểm tạo khớp `create_time`.
    pub fn open(pid: u32, create_time: u64, access: Access) -> windows::core::Result<Option<Self>> {
        let rights = access.rights() | PROCESS_QUERY_LIMITED_INFORMATION;
        // SAFETY: OpenProcess không có tiền điều kiện về con trỏ.
        let raw = unsafe { OpenProcess(rights, false, pid)? };
        // SAFETY: OpenProcess vừa trả handle mới, ta là chủ sở hữu duy nhất.
        let Some(h) = (unsafe { OwnedHandle::from_raw(raw) }) else { return Ok(None) };
        let h = Self(h);
        Ok((h.create_time()? == create_time).then_some(h))
    }

    fn create_time(&self) -> windows::core::Result<u64> {
        let (mut c, mut e, mut k, mut u) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        // SAFETY: handle hợp lệ, 4 out-pointer trỏ tới FILETIME trên stack.
        unsafe { GetProcessTimes(self.0.raw(), &mut c, &mut e, &mut k, &mut u)? };
        Ok((u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime))
    }

    /// Tiến trình "critical" — kết thúc nó sẽ gây BSOD.
    pub fn is_critical(&self) -> windows::core::Result<bool> {
        let mut crit = BOOL(0);
        // SAFETY: handle có quyền QUERY_LIMITED_INFORMATION; out-pointer hợp lệ.
        unsafe { IsProcessCritical(self.0.raw(), &mut crit)? };
        Ok(crit.as_bool())
    }

    pub fn terminate(&self) -> windows::core::Result<()> {
        // SAFETY: handle được mở với PROCESS_TERMINATE.
        unsafe { TerminateProcess(self.0.raw(), 1) }
    }

    pub fn suspend(&self) -> windows::core::Result<()> {
        // SAFETY: handle được mở với PROCESS_SUSPEND_RESUME.
        unsafe { NtSuspendProcess(self.0.raw()) }.ok()
    }

    pub fn resume(&self) -> windows::core::Result<()> {
        // SAFETY: handle được mở với PROCESS_SUSPEND_RESUME.
        unsafe { NtResumeProcess(self.0.raw()) }.ok()
    }

    pub fn set_priority(&self, class: PriorityClass) -> windows::core::Result<()> {
        // SAFETY: handle được mở với PROCESS_SET_INFORMATION.
        unsafe { SetPriorityClass(self.0.raw(), class.flag()) }
    }

    /// Đẩy working set ra standby/pagefile (chỉ cho tiến trình người dùng chọn).
    pub fn empty_working_set(&self) -> windows::core::Result<()> {
        // SAFETY: handle được mở với PROCESS_SET_QUOTA.
        unsafe { EmptyWorkingSet(self.0.raw()) }
    }

    pub fn image_path(&self) -> Option<PathBuf> {
        image_path_of(self.0.raw())
    }
}

/// Thu gọn working set của CHÍNH PC Manager (sau khi đóng cửa sổ, phần bộ nhớ
/// từng dùng cho WebView không cần nằm trong RAM nữa). Trang nào cần sẽ được nạp
/// lại khi dùng tới.
pub fn trim_self() -> windows::core::Result<()> {
    use windows::Win32::System::Threading::GetCurrentProcess;
    // SAFETY: GetCurrentProcess trả pseudo-handle luôn hợp lệ cho tiến trình hiện
    // tại (có đủ quyền PROCESS_SET_QUOTA), không cần và không được CloseHandle.
    unsafe { EmptyWorkingSet(GetCurrentProcess()) }
}

/// Chờ tiến trình `pid` thoát, tối đa `timeout`. `true` nếu đã thoát (hoặc không còn tồn tại).
pub fn wait_exit(pid: u32, timeout: std::time::Duration) -> bool {
    use windows::Win32::Foundation::WAIT_OBJECT_0;
    use windows::Win32::System::Threading::{WaitForSingleObject, PROCESS_SYNCHRONIZE};
    // SAFETY: OpenProcess không có tiền điều kiện về con trỏ.
    let Ok(raw) = (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }) else { return true };
    // SAFETY: handle mới mở, ta sở hữu duy nhất.
    let Some(h) = (unsafe { OwnedHandle::from_raw(raw) }) else { return true };
    let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
    // SAFETY: `h` là handle tiến trình hợp lệ có quyền SYNCHRONIZE, sống tới hết lời gọi.
    unsafe { WaitForSingleObject(h.raw(), ms) == WAIT_OBJECT_0 }
}

/// Đường dẫn exe của `pid` (không cần xác minh create_time — chỉ để hiển thị).
pub fn image_path(pid: u32) -> Option<PathBuf> {
    // SAFETY: OpenProcess không có tiền điều kiện về con trỏ.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    // SAFETY: handle mới mở, ta sở hữu duy nhất.
    let h = unsafe { OwnedHandle::from_raw(raw) }?;
    image_path_of(h.raw())
}

fn image_path_of(h: HANDLE) -> Option<PathBuf> {
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` ghi được `len` ký tự UTF-16; API cập nhật `len` = số ký tự
    // thực tế (không tính NUL).
    unsafe { QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len) }.ok()?;
    let s = String::from_utf16_lossy(buf.get(..len as usize)?);
    Some(PathBuf::from(s))
}

/// Hạ ưu tiên thread hiện tại xuống chế độ nền (I/O + CPU) — dùng cho worker
/// kiểm tra chữ ký / quét đĩa để không làm giật máy.
pub fn enter_background_mode() {
    // SAFETY: GetCurrentThread trả pseudo-handle hợp lệ cho thread hiện tại.
    let _ = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
}

/// Quyền cần cho từng thao tác.
#[derive(Debug, Clone, Copy)]
pub enum Access {
    Query,
    Terminate,
    SuspendResume,
    SetPriority,
    SetQuota,
}

impl Access {
    fn rights(self) -> PROCESS_ACCESS_RIGHTS {
        match self {
            Access::Query => PROCESS_QUERY_LIMITED_INFORMATION,
            Access::Terminate => PROCESS_TERMINATE,
            Access::SuspendResume => PROCESS_SUSPEND_RESUME,
            Access::SetPriority => PROCESS_SET_INFORMATION,
            Access::SetQuota => PROCESS_SET_QUOTA,
        }
    }
}

/// Mức ưu tiên cho phép đổi. Cố ý KHÔNG có Realtime — có thể làm treo máy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PriorityClass {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
}

impl PriorityClass {
    fn flag(self) -> PROCESS_CREATION_FLAGS {
        match self {
            PriorityClass::Idle => IDLE_PRIORITY_CLASS,
            PriorityClass::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
            PriorityClass::Normal => NORMAL_PRIORITY_CLASS,
            PriorityClass::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
            PriorityClass::High => HIGH_PRIORITY_CLASS,
        }
    }

    /// Suy ra từ base priority trong snapshot (4/6/8/10/13/24).
    /// `None` cho Realtime hoặc giá trị lạ.
    pub fn from_base_priority(p: i32) -> Option<Self> {
        Some(match p {
            ..=4 => PriorityClass::Idle,
            5..=6 => PriorityClass::BelowNormal,
            7..=8 => PriorityClass::Normal,
            9..=10 => PriorityClass::AboveNormal,
            11..=15 => PriorityClass::High,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nt_struct_sizes_match_x64() {
        assert_eq!(size_of::<UnicodeString>(), 16);
        assert_eq!(size_of::<SystemProcessInformation>(), 0x100);
        assert_eq!(size_of::<SystemThreadInformation>(), 0x50);
    }

    #[test]
    fn enumerate_finds_self_with_correct_identity() {
        let list = enumerate().unwrap();
        assert!(list.len() > 10);
        let me = list.iter().find(|p| p.pid == std::process::id()).unwrap();
        assert!(me.name.to_lowercase().starts_with("pc_manager"));
        assert!(me.threads > 0 && me.private_ws > 0 && !me.suspended);
        // Mở lại chính mình bằng pid + create_time phải thành công…
        assert!(ProcessHandle::open(me.pid, me.create_time, Access::Query).unwrap().is_some());
        // …còn create_time sai thì bị từ chối (chống PID reuse).
        assert!(ProcessHandle::open(me.pid, me.create_time + 1, Access::Query).unwrap().is_none());
        assert!(list.iter().any(|p| p.pid == 4 && p.name == "System"));
    }

    #[test]
    fn parse_rejects_out_of_bounds_name() {
        let mut entry = vec![0u8; size_of::<SystemProcessInformation>()];
        // ImageName.Length = 8, Buffer trỏ ra ngoài buffer.
        entry[56..58].copy_from_slice(&8u16.to_ne_bytes());
        entry[64..72].copy_from_slice(&usize::MAX.to_ne_bytes());
        entry[80..88].copy_from_slice(&1234usize.to_ne_bytes());
        let list = parse(&entry, 0x1000);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "<pid 1234>");
    }

    #[test]
    fn priority_from_base() {
        assert_eq!(PriorityClass::from_base_priority(8), Some(PriorityClass::Normal));
        assert_eq!(PriorityClass::from_base_priority(13), Some(PriorityClass::High));
        assert_eq!(PriorityClass::from_base_priority(24), None);
    }
}
