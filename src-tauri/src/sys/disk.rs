//! Ổ đĩa: danh sách, loại, nhãn, hệ thống file, dung lượng.

use windows::core::HSTRING;
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveType {
    Removable,
    Fixed,
    Network,
    CdRom,
    RamDisk,
    Unknown,
}

impl DriveType {
    /// Giá trị của GetDriveTypeW (DRIVE_*).
    fn from_raw(v: u32) -> Self {
        match v {
            2 => Self::Removable,
            3 => Self::Fixed,
            4 => Self::Network,
            5 => Self::CdRom,
            6 => Self::RamDisk,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Drive {
    /// "C:\"
    pub root: String,
    pub kind: DriveType,
    pub label: String,
    pub fs: String,
    pub total: u64,
    pub free: u64,
}

/// Các chữ cái ổ đĩa đang có (bit i = ổ 'A' + i).
pub fn letters() -> Vec<char> {
    // SAFETY: không tham số, chỉ trả về bitmask.
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8).filter(|i| mask & (1 << i) != 0).map(|i| char::from(b'A' + i)).collect()
}

/// Thông tin một ổ. Ổ CD trống / ổ mạng mất kết nối => dung lượng 0.
pub fn drive(letter: char) -> Drive {
    let root = format!("{letter}:\\");
    let h = HSTRING::from(root.as_str());
    // SAFETY: `h` là chuỗi NUL-terminated sống tới hết lời gọi.
    let kind = DriveType::from_raw(unsafe { GetDriveTypeW(&h) });
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: out-pointer trỏ tới biến stack; lỗi (ổ chưa sẵn sàng) được bỏ qua.
    let _ = unsafe { GetDiskFreeSpaceExW(&h, Some(&mut free), Some(&mut total), None) };
    let mut label = [0u16; 261];
    let mut fs = [0u16; 261];
    // SAFETY: buffer ghi được đúng độ dài slice; các out-pointer khác là None.
    let ok = unsafe { GetVolumeInformationW(&h, Some(&mut label), None, None, None, Some(&mut fs)) }.is_ok();
    let s = |b: &[u16]| String::from_utf16_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]);
    Drive {
        root,
        kind,
        label: if ok { s(&label) } else { String::new() },
        fs: if ok { s(&fs) } else { String::new() },
        total,
        free,
    }
}

/// Định danh file trên ổ (serial volume, file index). Hai đường dẫn cùng id = hard link
/// của cùng một file — xoá một cái không lấy lại dung lượng.
pub fn file_id(path: &std::path::Path) -> Option<(u32, u64)> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION};
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let f = std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(0x7) // read | write | delete
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: handle của `f` còn mở suốt lời gọi (f sống tới cuối hàm); `info` là out-param trên stack.
    unsafe { GetFileInformationByHandle(HANDLE(f.as_raw_handle()), &mut info) }.ok()?;
    Some((info.dwVolumeSerialNumber, (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn hard_links_share_file_id() {
        let dir = std::env::temp_dir().join(format!("pcm-fid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b, c) = (dir.join("a"), dir.join("b"), dir.join("c"));
        std::fs::write(&a, b"x").unwrap();
        std::fs::hard_link(&a, &b).unwrap();
        std::fs::write(&c, b"x").unwrap();
        assert_eq!(super::file_id(&a), super::file_id(&b));
        assert_ne!(super::file_id(&a), super::file_id(&c));
        assert!(super::file_id(&a).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn system_drive_has_space() {
        let sys = std::env::var("SystemDrive").unwrap();
        let letter = sys.chars().next().unwrap();
        assert!(super::letters().contains(&letter));
        let d = super::drive(letter);
        assert_eq!(d.kind, super::DriveType::Fixed);
        assert!(d.total > 0 && d.free <= d.total);
        assert_eq!(d.fs, "NTFS");
    }
}
