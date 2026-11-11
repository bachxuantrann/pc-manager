//! Thùng rác của user đang đăng nhập.
//!
//! API shell (`SHEmptyRecycleBin`, `SHFileOperation`) luôn dùng Thùng rác của tài khoản chạy
//! tiến trình. Khi nâng quyền bằng tài khoản admin khác, file "xoá vào Thùng rác" sẽ nằm trong
//! Thùng rác của admin — người dùng không thấy, không khôi phục được. Khi đó ta làm trực tiếp
//! trên `X:\$Recycle.Bin\<SID của user>` theo đúng định dạng của Windows 10/11:
//! mỗi mục là một cặp `$R<6 ký tự><đuôi>` (dữ liệu) + `$I<6 ký tự><đuôi>` (thông tin gốc).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use crate::sys::recycle::RecycleInfo;

/// FILETIME (100ns từ 1601-01-01) của mốc Unix.
const FILETIME_UNIX_EPOCH: u64 = 116_444_736_000_000_000;

/// Nội dung file `$I` (phiên bản 2, Windows 10+).
fn encode_info(original: &str, size: u64, deleted_filetime: u64) -> Vec<u8> {
    let path: Vec<u16> = original.encode_utf16().chain(std::iter::once(0)).collect();
    let mut out = Vec::with_capacity(28 + path.len() * 2);
    out.extend_from_slice(&2u64.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&deleted_filetime.to_le_bytes());
    out.extend_from_slice(&u32::try_from(path.len()).unwrap_or(u32::MAX).to_le_bytes());
    for c in path {
        out.extend_from_slice(&c.to_le_bytes());
    }
    out
}

/// Kích thước gốc ghi trong file `$I` (phiên bản 1 và 2 đều để ở byte 8..16).
fn info_size(bytes: &[u8]) -> Option<u64> {
    let version = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    if !(1..=2).contains(&version) {
        return None;
    }
    Some(u64::from_le_bytes(bytes.get(8..16)?.try_into().ok()?))
}

/// 6 ký tự [A-Z0-9] cho tên `$R`/`$I`, lấy từ băm của đường dẫn + thời điểm + lần thử.
fn suffix(seed: &str, attempt: u32) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let hash = blake3::hash(format!("{seed}|{nanos}|{attempt}").as_bytes());
    hash.as_bytes().iter().take(6).map(|b| char::from(ALPHABET[usize::from(*b) % ALPHABET.len()])).collect()
}

fn bin_dir(sid: &str, letter: char) -> PathBuf {
    PathBuf::from(format!(r"{letter}:\$Recycle.Bin\{sid}"))
}

/// Các thư mục Thùng rác đang có của user trên mọi ổ.
fn bins(sid: &str) -> Vec<PathBuf> {
    crate::sys::disk::letters().into_iter().map(|l| bin_dir(sid, l)).filter(|d| d.is_dir()).collect()
}

/// Các mục `$I` trong một thư mục Thùng rác.
fn info_files(bin: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(bin) else { return Vec::new() };
    rd.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("$I")))
        .collect()
}

/// Tổng dung lượng (không theo symlink/junction).
fn total_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    std::fs::read_dir(path).map_or(0, |rd| rd.flatten().map(|e| total_size(&e.path())).sum())
}

/// Dung lượng Thùng rác (mọi ổ) của user đang đăng nhập.
pub fn query() -> Result<RecycleInfo, String> {
    let Some(sid) = super::other_sid() else { return crate::sys::recycle::query().map_err(|e| e.to_string()) };
    let mut info = RecycleInfo::default();
    for bin in bins(sid) {
        for i in info_files(&bin) {
            info.items += 1;
            info.bytes += std::fs::read(&i).ok().and_then(|b| info_size(&b)).unwrap_or(0);
        }
    }
    Ok(info)
}

/// Làm rỗng Thùng rác (mọi ổ) của user đang đăng nhập. Chỉ xoá các mục `$R…`/`$I…` trong
/// `X:\$Recycle.Bin\<SID>`, không đụng `desktop.ini` hay thứ gì khác.
pub fn empty() -> Result<(), String> {
    let Some(sid) = super::other_sid() else { return crate::sys::recycle::empty().map_err(|e| e.to_string()) };
    let mut failed = 0u32;
    for bin in bins(sid) {
        let Ok(rd) = std::fs::read_dir(&bin) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("$R") || name.starts_with("$I")) {
                continue;
            }
            let path = e.path();
            // remove_dir_all không đi theo symlink/junction (chỉ xoá chính liên kết).
            let res = match e.file_type() {
                Ok(t) if t.is_dir() => std::fs::remove_dir_all(&path),
                _ => std::fs::remove_file(&path),
            };
            if res.is_err() {
                failed += 1;
            }
        }
    }
    if failed > 0 {
        return Err(format!("{failed} mục đang bị khoá, chưa xoá được"));
    }
    Ok(())
}

/// Chuyển `path` vào Thùng rác của user đang đăng nhập (khôi phục được từ Thùng rác của họ).
pub fn move_to_recycle_bin(path: &Path) -> Result<(), String> {
    match super::other_sid() {
        Some(sid) => move_into_bin(sid, path),
        None => crate::sys::recycle::move_to_recycle_bin(path),
    }
}

/// Chuyển `path` vào Thùng rác của user `sid` (ghi trực tiếp cặp `$R`/`$I`).
fn move_into_bin(sid: &str, path: &Path) -> Result<(), String> {
    let shown = path.display().to_string();
    let original = shown.strip_prefix(r"\\?\").unwrap_or(&shown).to_owned();
    let letter = original
        .chars()
        .next()
        .filter(|c| c.is_ascii_alphabetic() && original.get(1..3) == Some(r":\"))
        .ok_or("chỉ hỗ trợ đường dẫn trên ổ đĩa (X:\\…)")?
        .to_ascii_uppercase();
    let bin = bin_dir(sid, letter);
    if !bin.is_dir() {
        return Err(format!("chưa có Thùng rác của người dùng trên ổ {letter}: — hãy mở PC Manager ở chế độ thường để xoá mục này"));
    }
    let meta = std::fs::symlink_metadata(path).map_err(|_| "không còn tồn tại".to_owned())?;
    let size = if meta.is_dir() { total_size(path) } else { meta.len() };
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let (data, info) = (0..20)
        .map(|n| {
            let s = suffix(&original, n);
            (bin.join(format!("$R{s}{ext}")), bin.join(format!("$I{s}{ext}")))
        })
        .find(|(r, i)| std::fs::symlink_metadata(r).is_err() && std::fs::symlink_metadata(i).is_err())
        .ok_or("không tạo được tên trong Thùng rác")?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let filetime = u64::try_from(now / 100).unwrap_or(0) + FILETIME_UNIX_EPOCH;
    // Cùng ổ => chỉ đổi tên, không chép dữ liệu.
    std::fs::rename(path, &data).map_err(|e| format!("không chuyển được vào Thùng rác: {e}"))?;
    if let Err(e) = std::fs::write(&info, encode_info(&original, size, filetime)) {
        // Không ghi được thông tin gốc => trả lại chỗ cũ, không để mục "mồ côi".
        let _ = std::fs::rename(&data, path);
        return Err(format!("không ghi được thông tin Thùng rác: {e}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_file_roundtrip() {
        let b = encode_info(r"C:\Users\ánh\a.txt", 1234, FILETIME_UNIX_EPOCH);
        assert_eq!(info_size(&b), Some(1234));
        assert_eq!(&b[..8], &2u64.to_le_bytes());
        let len = u32::from_le_bytes(b[24..28].try_into().unwrap()) as usize;
        let chars: Vec<u16> = b[28..].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(chars.len(), len);
        assert_eq!(String::from_utf16(&chars[..len - 1]).unwrap(), r"C:\Users\ánh\a.txt");
        assert_eq!(chars[len - 1], 0);
        assert_eq!(info_size(&[0u8; 4]), None);
        assert_eq!(info_size(&[9u8; 32]), None, "phiên bản lạ => bỏ qua");
    }

    #[test]
    fn suffix_is_six_upper_alnum() {
        let s = suffix("x", 0);
        assert_eq!(s.len(), 6);
        assert!(s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()));
        assert_ne!(suffix("x", 0), suffix("x", 1));
    }

    #[test]
    fn same_user_uses_shell_recycle_bin() {
        // Cùng user => đi qua API shell như trước.
        assert!(query().is_ok());
    }
}

#[cfg(test)]
mod format_check {
    /// Kiểm tra định dạng với Explorer: ghi một file vào Thùng rác của CHÍNH user đang chạy test
    /// bằng `move_into_bin`, rồi xem Explorer có liệt kê đúng tên/đường dẫn gốc không.
    /// Chạy tay: `cargo test format_check -- --ignored --nocapture`, sau đó khôi phục/xoá mục đó.
    #[test]
    #[ignore = "ghi vào Thùng rác thật của người chạy test"]
    fn writes_entry_explorer_can_read() {
        let sid = crate::sys::session::current_user_sid().expect("SID");
        let dir = std::env::temp_dir().join(format!("pcm-recycle-fmt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("pcm-kiểm-tra.txt");
        std::fs::write(&file, b"pc manager recycle format check").unwrap();
        super::move_into_bin(&sid, &file).unwrap();
        assert!(!file.exists());
        println!("ĐÃ GHI: {}", file.display());
    }
}
