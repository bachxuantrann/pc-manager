//! Đọc shortcut `.lnk` qua `IShellLinkW` (cần COM đã init trên thread này).

use std::path::{Path, PathBuf};

use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use super::com::ComGuard;

/// Đích của shortcut: đường dẫn exe và tham số.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkTarget {
    pub path: Option<PathBuf>,
    pub args: String,
}

/// `_com` chứng minh COM đã init trên thread hiện tại.
pub fn resolve_lnk(_com: &ComGuard, lnk: &Path) -> windows::core::Result<LinkTarget> {
    // SAFETY: COM đã init (bằng chứng `_com`); CLSID/IID khớp interface yêu cầu.
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)? };
    let file: IPersistFile = link.cast()?;
    let wpath = HSTRING::from(lnk.as_os_str());
    // SAFETY: `wpath` là chuỗi NUL-terminated sống tới hết lời gọi.
    unsafe { file.Load(&wpath, STGM_READ)? };

    let mut buf = vec![0u16; 1024];
    // SAFETY: buffer ghi được đúng buf.len() ký tự; pfd = null được phép; flag 0
    // = đường dẫn đầy đủ. Không gọi Resolve() để không tìm file trên mạng/đĩa.
    unsafe { link.GetPath(&mut buf, std::ptr::null_mut(), 0)? };
    let path = wide_to_string(&buf);

    // SAFETY: như trên, buffer ghi được buf.len() ký tự.
    unsafe { link.GetArguments(&mut buf)? };
    let args = wide_to_string(&buf);

    Ok(LinkTarget { path: (!path.is_empty()).then(|| PathBuf::from(path)), args })
}

/// Mở `exe` với quyền Administrator (verb "runas" → Windows hiện hộp UAC).
/// `Ok(true)` = đã khởi chạy; `Ok(false)` = người dùng bấm No ở UAC.
pub fn run_elevated(exe: &Path, args: &str) -> windows::core::Result<bool> {
    run_elevated_pid(exe, args).map(|pid| pid.is_some())
}

/// Mở `exe` với quyền Administrator và trả về PID của tiến trình vừa chạy (để theo dõi tới khi xong).
/// `Ok(None)` = người dùng bấm No ở UAC.
pub fn run_elevated_pid(exe: &Path, args: &str) -> windows::core::Result<Option<u32>> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_CANCELLED;
    use windows::Win32::System::Threading::GetProcessId;
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

    let verb = HSTRING::from("runas");
    let file = HSTRING::from(exe.as_os_str());
    let params = HSTRING::from(args);
    let dir = exe.parent().map(|d| HSTRING::from(d.as_os_str()));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        lpDirectory: dir.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
        nShow: 1, // SW_SHOWNORMAL
        ..Default::default()
    };
    // SAFETY: `info` có cbSize đúng; các chuỗi HSTRING NUL-terminated sống tới hết lời gọi.
    match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) => {}
        Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => return Ok(None),
        Err(e) => return Err(e),
    }
    // SAFETY: SEE_MASK_NOCLOSEPROCESS => hProcess (nếu có) thuộc về ta; bọc để tự đóng.
    let process = unsafe { super::OwnedHandle::from_raw(info.hProcess) };
    // Không có handle (vd. được giao cho tiến trình đang chạy sẵn) => không theo dõi được, coi như PID 0.
    // SAFETY: handle hợp lệ, còn sống trong lời gọi.
    Ok(Some(process.map_or(0, |h| unsafe { GetProcessId(h.raw()) })))
}

/// Giải chuỗi tài nguyên gián tiếp: `@dll,-123` (tên dịch vụ), `@{Gói?ms-resource://…}` (app Store).
/// Không giải được => None (người gọi dùng tên thô).
pub fn load_indirect_string(source: &str) -> Option<String> {
    let mut buf = [0u16; 512];
    // SAFETY: HSTRING kết thúc NUL sống tới hết lời gọi; `buf` ghi được, độ dài truyền theo slice;
    // tham số reserved phải là None.
    unsafe { windows::Win32::UI::Shell::SHLoadIndirectString(&HSTRING::from(source), &mut buf, None) }.ok()?;
    Some(wide_to_string(&buf)).filter(|s| !s.is_empty())
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_start_menu_shortcut() {
        let com = ComGuard::init().unwrap();
        // Shortcut có sẵn trên mọi máy Windows 10/11.
        let base = std::env::var("ProgramData").unwrap();
        let dir = Path::new(&base).join(r"Microsoft\Windows\Start Menu\Programs");
        let lnk = std::fs::read_dir(&dir)
            .unwrap()
            .chain(std::fs::read_dir(dir.join("Accessories")).into_iter().flatten())
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")))
            .expect("không tìm thấy .lnk nào trong Start Menu");
        let t = resolve_lnk(&com, &lnk).unwrap();
        assert!(t.path.is_some(), "{lnk:?} không có đích");
    }

    #[test]
    fn missing_file_is_error() {
        let com = ComGuard::init().unwrap();
        assert!(resolve_lnk(&com, Path::new(r"C:\khong-ton-tai.lnk")).is_err());
    }
}
