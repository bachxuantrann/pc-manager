//! Bảng SMBIOS của firmware (`GetSystemFirmwareTable('RSMB')`) — không cần quyền admin, không WMI.

use windows::Win32::System::SystemInformation::{GetSystemFirmwareTable, GetTickCount64, FIRMWARE_TABLE_PROVIDER};

/// 'RSMB' (big-endian) — nhà cung cấp bảng SMBIOS thô.
const RSMB: FIRMWARE_TABLE_PROVIDER = FIRMWARE_TABLE_PROVIDER(u32::from_be_bytes(*b"RSMB"));
/// Header `RawSMBIOSData`: 4 byte phiên bản + 4 byte độ dài, rồi tới bảng.
const HEADER: usize = 8;

/// Phần bảng SMBIOS (bỏ header). `None` nếu firmware không có / không đọc được.
pub fn smbios_table() -> Option<Vec<u8>> {
    // SAFETY: buffer None => chỉ hỏi kích thước cần thiết.
    let size = unsafe { GetSystemFirmwareTable(RSMB, 0, None) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; usize::try_from(size).ok()?];
    // SAFETY: `buf` ghi được đúng buf.len() byte (hàm tự lấy độ dài từ slice).
    let written = unsafe { GetSystemFirmwareTable(RSMB, 0, Some(&mut buf)) };
    let written = usize::try_from(written).ok()?.min(buf.len());
    let len_field = buf.get(4..8).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))?;
    let end = HEADER.checked_add(usize::try_from(len_field).ok()?)?.min(written);
    buf.get(HEADER..end).map(<[u8]>::to_vec)
}

/// Số giây từ lúc Windows khởi động (không tính lúc ngủ/ngủ đông theo cách Windows đếm).
pub fn uptime_secs() -> u64 {
    // SAFETY: không tham số, chỉ đọc bộ đếm của hệ thống.
    let ms = unsafe { GetTickCount64() };
    ms / 1000
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_this_machines_table() {
        // Mọi máy x64 có UEFI/BIOS đều có SMBIOS; chỉ cần đọc được và phân tích không lỗi.
        let t = super::smbios_table().expect("đọc được bảng SMBIOS");
        assert!(t.len() > 100);
        let s = crate::hwinfo::smbios::parse(&t);
        assert!(s.bios.vendor.is_some() || s.bios.version.is_some());
    }
}
