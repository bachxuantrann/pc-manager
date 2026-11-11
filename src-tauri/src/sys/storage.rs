//! Thông tin ổ đĩa vật lý (`\\.\PhysicalDriveN`) qua IOCTL: model, dung lượng, loại bus, SSD/HDD.
//!
//! Mở ổ với quyền truy cập 0 (chỉ hỏi thông tin, không đọc dữ liệu) => không cần quyền admin.
//! Không lấy số serial (thông tin định danh máy).

use std::ffi::c_void;
use std::mem::{offset_of, size_of};

use windows::core::HSTRING;
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR,
    DISK_GEOMETRY_EX, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_DESCRIPTOR,
    STORAGE_PROPERTY_ID, STORAGE_PROPERTY_QUERY,
};
use windows::Win32::System::IO::DeviceIoControl;

use super::OwnedHandle;

/// Số ổ vật lý tối đa thử mở (\\.\PhysicalDrive0..N). Số thứ tự có thể không liền nhau.
const MAX_DRIVES: u32 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalDisk {
    pub index: u32,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub revision: Option<String>,
    /// Giá trị `STORAGE_BUS_TYPE` thô (17 = NVMe, 11 = SATA, 7 = USB…).
    pub bus_type: Option<i32>,
    pub size_bytes: Option<u64>,
    /// `true` = có thời gian tìm vết (HDD), `false` = SSD.
    pub seek_penalty: Option<bool>,
}

fn open_disk(index: u32) -> Option<OwnedHandle> {
    let path = HSTRING::from(format!(r"\\.\PhysicalDrive{index}"));
    // SAFETY: `path` là chuỗi NUL-terminated sống tới hết lời gọi; quyền 0 = chỉ truy vấn.
    let h = unsafe {
        CreateFileW(&path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, None, OPEN_EXISTING, FILE_FLAGS_AND_ATTRIBUTES(0), None)
    }
    .ok()?;
    // SAFETY: CreateFileW vừa mở `h` cho ta.
    unsafe { OwnedHandle::from_raw(h) }
}

/// Gọi một IOCTL, trả về các byte đã ghi (buffer căn lề 8).
fn ioctl(h: &OwnedHandle, code: u32, input: Option<&STORAGE_PROPERTY_QUERY>, out_len: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u64; out_len.div_ceil(8)];
    let mut returned = 0u32;
    let (in_ptr, in_len) = match input {
        Some(q) => (Some(std::ptr::from_ref(q).cast::<c_void>()), u32::try_from(size_of::<STORAGE_PROPERTY_QUERY>()).ok()?),
        None => (None, 0),
    };
    let cap = u32::try_from(buf.len() * 8).ok()?;
    // SAFETY: handle còn mở; input (nếu có) trỏ tới một STORAGE_PROPERTY_QUERY sống tới hết lời
    // gọi; output là buffer ghi được `cap` byte; không dùng OVERLAPPED.
    unsafe { DeviceIoControl(h.raw(), code, in_ptr, in_len, Some(buf.as_mut_ptr().cast::<c_void>()), cap, Some(&mut returned), None) }
        .ok()?;
    let n = usize::try_from(returned).ok()?.min(buf.len() * 8);
    Some(buf.iter().flat_map(|w| w.to_le_bytes()).take(n).collect())
}

fn query(id: STORAGE_PROPERTY_ID) -> STORAGE_PROPERTY_QUERY {
    STORAGE_PROPERTY_QUERY { PropertyId: id, QueryType: PropertyStandardQuery, AdditionalParameters: [0] }
}

/// Đọc trường u32 ở vị trí của một field trong struct (bytes trả về từ driver).
fn u32_at(bytes: &[u8], off: usize) -> Option<u32> {
    bytes.get(off..off + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Chuỗi ASCII kết thúc NUL bắt đầu ở `off` (0 = không có).
fn str_at(bytes: &[u8], off: u32) -> Option<String> {
    let off = usize::try_from(off).ok().filter(|&o| o > 0)?;
    let rest = bytes.get(off..)?;
    let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
    let s = String::from_utf8_lossy(rest.get(..end)?).trim().to_owned();
    (!s.is_empty()).then_some(s)
}

fn describe(index: u32, h: &OwnedHandle) -> PhysicalDisk {
    let mut d = PhysicalDisk { index, vendor: None, product: None, revision: None, bus_type: None, size_bytes: None, seek_penalty: None };
    if let Some(b) = ioctl(h, IOCTL_STORAGE_QUERY_PROPERTY, Some(&query(StorageDeviceProperty)), 1024) {
        // Vị trí field lấy từ chính định nghĩa struct (offset_of!), không hard-code.
        let at = |off: usize| u32_at(&b, off);
        d.vendor = at(offset_of!(STORAGE_DEVICE_DESCRIPTOR, VendorIdOffset)).and_then(|o| str_at(&b, o));
        d.product = at(offset_of!(STORAGE_DEVICE_DESCRIPTOR, ProductIdOffset)).and_then(|o| str_at(&b, o));
        d.revision = at(offset_of!(STORAGE_DEVICE_DESCRIPTOR, ProductRevisionOffset)).and_then(|o| str_at(&b, o));
        d.bus_type = at(offset_of!(STORAGE_DEVICE_DESCRIPTOR, BusType)).and_then(|v| i32::try_from(v).ok());
    }
    if let Some(b) = ioctl(h, IOCTL_STORAGE_QUERY_PROPERTY, Some(&query(StorageDeviceSeekPenaltyProperty)), size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>()) {
        d.seek_penalty = b.get(offset_of!(DEVICE_SEEK_PENALTY_DESCRIPTOR, IncursSeekPenalty)).map(|&v| v != 0);
    }
    if let Some(b) = ioctl(h, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, None, size_of::<DISK_GEOMETRY_EX>()) {
        let off = offset_of!(DISK_GEOMETRY_EX, DiskSize);
        d.size_bytes = b
            .get(off..off + 8)
            .and_then(|s| <[u8; 8]>::try_from(s).ok())
            .map(i64::from_le_bytes)
            .and_then(|v| u64::try_from(v).ok())
            .filter(|&v| v > 0);
    }
    d
}

/// Mọi ổ đĩa vật lý mở được.
pub fn physical_disks() -> Vec<PhysicalDisk> {
    (0..MAX_DRIVES).filter_map(|i| open_disk(i).map(|h| describe(i, &h))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_bounded_and_trimmed() {
        let b = b"\0\0\0\0Samsung SSD 990 PRO 2TB   \0rest";
        assert_eq!(str_at(b, 4).as_deref(), Some("Samsung SSD 990 PRO 2TB"));
        assert_eq!(str_at(b, 0), None, "offset 0 = không có");
        assert_eq!(str_at(b, 999), None, "ngoài buffer");
        assert_eq!(u32_at(&[1, 0], 0), None);
    }

    #[test]
    fn reads_this_machines_disks_without_admin() {
        let disks = physical_disks();
        assert!(!disks.is_empty(), "máy nào cũng có ít nhất một ổ");
        let d = &disks[0];
        assert!(d.size_bytes.is_some_and(|s| s > 1_000_000_000), "{d:?}");
        assert!(d.product.is_some(), "{d:?}");
    }
}
