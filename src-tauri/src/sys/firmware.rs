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

/// Số giây kể từ lần máy **thức dậy** gần nhất (từ Sleep, Hibernate, hoặc bật máy khi Fast Startup
/// đang bật — "Shut down" khi đó thực ra là ngủ đông nhân Windows). `None` = chưa thức dậy lần nào
/// kể từ lần khởi động thật (tức uptime đã là thời gian từ lúc bật máy).
pub fn secs_since_last_wake() -> Option<u64> {
    let mut wake: u64 = 0;
    // SAFETY: không có input; output trỏ tới một u64 trên stack, độ dài khai báo đúng 8 byte.
    let status = unsafe {
        super::ntdll::NtPowerInformation(
            super::ntdll::POWER_LAST_WAKE_TIME,
            std::ptr::null(),
            0,
            std::ptr::from_mut(&mut wake).cast(),
            u32::try_from(std::mem::size_of::<u64>()).ok()?,
        )
    };
    if status.is_err() || wake == 0 {
        return None;
    }
    // Cùng gốc thời gian (từ lúc khởi động, tính cả lúc ngủ): tick count (ms) và interrupt time (100ns).
    // SAFETY: không tham số, chỉ đọc bộ đếm của hệ thống.
    let now_ms = unsafe { GetTickCount64() };
    Some((now_ms / 1000).saturating_sub(wake / 10_000_000))
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
