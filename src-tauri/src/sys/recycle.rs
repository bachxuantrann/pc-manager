//! Thùng rác: hỏi dung lượng (mọi ổ) và làm rỗng qua Shell API.

use std::mem::size_of;

use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHQUERYRBINFO,
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

#[cfg(test)]
mod tests {
    #[test]
    fn query_does_not_fail() {
        // Chỉ đọc — KHÔNG gọi empty() trong test.
        assert!(super::query().is_ok());
    }
}
