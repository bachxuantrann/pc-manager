//! Thùng rác: hỏi dung lượng (mọi ổ), làm rỗng, và chuyển file vào Thùng rác.

use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    SHEmptyRecycleBinW, SHFileOperationW, SHQueryRecycleBinW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI,
    FOF_SILENT, FOF_WANTNUKEWARNING, FO_DELETE, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
    SHFILEOPSTRUCTW, SHQUERYRBINFO,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct RecycleInfo {
    pub bytes: u64,
    pub items: u64,
}

/// Tổng dung lượng Thùng rác trên mọi ổ đĩa.
pub fn query() -> windows::core::Result<RecycleInfo> {
    let mut info = SHQUERYRBINFO { cbSize: size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    // SAFETY: root null = mọi ổ; `info` có cbSize đúng.
    unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info)? };
    Ok(RecycleInfo { bytes: info.i64Size.max(0) as u64, items: info.i64NumItems.max(0) as u64 })
}

/// Làm rỗng Thùng rác trên mọi ổ, không hiện hộp thoại của Explorer
/// (người dùng đã xác nhận trong PC Manager).
pub fn empty() -> windows::core::Result<()> {
    // SAFETY: hwnd None, root null = mọi ổ; chỉ truyền cờ.
    unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND) }
}

/// Chuyển một file/thư mục vào Thùng rác (có thể khôi phục). Không hiện hộp
/// thoại xác nhận/lỗi; nếu mục quá lớn để vào Thùng rác, Explorer sẽ HỎI thay
/// vì xoá vĩnh viễn (FOF_WANTNUKEWARNING). Trả về mã lỗi của SHFileOperationW.
pub fn move_to_recycle_bin(path: &Path) -> Result<(), String> {
    // pFrom: danh sách chuỗi kết thúc bằng 2 ký tự NUL.
    let mut from: Vec<u16> = path.as_os_str().encode_wide().collect();
    if from.is_empty() || from.contains(&0) {
        return Err("đường dẫn không hợp lệ".into());
    }
    from.extend([0, 0]);
    let flags = FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT | FOF_WANTNUKEWARNING;
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        fFlags: flags.0 as u16,
        ..Default::default()
    };
    // SAFETY: `op` hợp lệ; `from` NUL-kép và sống tới hết lời gọi; pTo không dùng
    // với FO_DELETE; hNameMappings null vì không bật FOF_WANTMAPPINGHANDLE.
    let code = unsafe { SHFileOperationW(&mut op) };
    if code != 0 {
        return Err(format!("SHFileOperation lỗi 0x{code:X}"));
    }
    if op.fAnyOperationsAborted.as_bool() {
        return Err("thao tác bị huỷ".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn query_does_not_fail() {
        // Chỉ đọc — KHÔNG gọi empty() trong test.
        assert!(super::query().is_ok());
    }

    #[test]
    #[ignore = "tạo một mục nhỏ trong Thùng rác thật"]
    fn moves_a_temp_dir_to_recycle_bin() {
        let dir = std::env::temp_dir().join(format!("pcm-recycle-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("a.txt"), b"x").unwrap();
        super::move_to_recycle_bin(&dir).unwrap();
        assert!(!dir.exists());
        assert!(super::move_to_recycle_bin(std::path::Path::new("")).is_err());
    }
}
