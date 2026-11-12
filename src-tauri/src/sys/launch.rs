//! Chạy một chương trình dưới user đang đăng nhập (chủ desktop), không phải tài khoản chạy app.
//!
//! Dùng khi app được nâng quyền bằng tài khoản admin KHÁC: trình gỡ của app cài riêng cho người
//! dùng (HKCU) phải chạy dưới chính người dùng đó, nếu không nó tìm file/registry trong profile
//! của admin và gỡ hỏng. Cách làm: tạo tiến trình với "cha" là Explorer của phiên hiện tại
//! (`PROC_THREAD_ATTRIBUTE_PARENT_PROCESS`) — tiến trình mới nhận token của Explorer, tức token
//! thường (không nâng quyền) của người dùng đang đăng nhập.

use std::ffi::c_void;
use std::mem::size_of;
use std::path::Path;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList, OpenProcess, UpdateProcThreadAttribute,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_CREATE_PROCESS,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS, STARTUPINFOEXW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId};

use super::OwnedHandle;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Khối biến môi trường cho CreateProcessW: "K=V\0K=V\0\0" (UTF-16), sắp theo tên như Windows yêu cầu.
fn env_block(vars: &[(String, String)]) -> Vec<u16> {
    let mut sorted: Vec<&(String, String)> = vars.iter().filter(|(k, _)| !k.is_empty() && !k.contains('=')).collect();
    sorted.sort_by_key(|(k, _)| k.to_uppercase());
    let mut out: Vec<u16> = Vec::new();
    for (k, v) in sorted {
        out.extend(format!("{k}={v}").encode_utf16());
        out.push(0);
    }
    if out.is_empty() {
        out.push(0);
    }
    out.push(0);
    out
}

/// Mở Explorer của phiên hiện tại với quyền tạo tiến trình con.
fn open_shell_process() -> windows::core::Result<OwnedHandle> {
    // SAFETY: không tham số; trả về HWND của desktop shell hoặc null.
    let hwnd = unsafe { GetShellWindow() };
    if hwnd.0.is_null() {
        return Err(windows::core::Error::new(windows::core::HRESULT(0x8007_0490_u32 as i32), "không tìm thấy Explorer của người dùng đang đăng nhập"));
    }
    let mut pid = 0u32;
    // SAFETY: `hwnd` hợp lệ vừa lấy; `pid` là out-pointer trên stack.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let open = || {
        // SAFETY: chỉ xin quyền PROCESS_CREATE_PROCESS trên một pid; handle trả về được bọc ngay.
        unsafe { OpenProcess(PROCESS_CREATE_PROCESS, false, pid) }
    };
    // Explorer thuộc user khác => tài khoản admin cần SeDebugPrivilege để mở.
    let raw = open().or_else(|_| {
        let _ = super::token::enable_privilege("SeDebugPrivilege");
        open()
    })?;
    // SAFETY: OpenProcess vừa mở `raw` cho ta.
    unsafe { OwnedHandle::from_raw(raw) }.ok_or_else(windows::core::Error::from_win32)
}

/// Danh sách thuộc tính tiến trình (một thuộc tính: tiến trình cha). Tự `Delete…` khi drop.
struct AttrList {
    buf: Vec<usize>,
}

impl AttrList {
    fn ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        LPPROC_THREAD_ATTRIBUTE_LIST(self.buf.as_mut_ptr().cast::<c_void>())
    }

    /// `parent` phải sống lâu hơn lời gọi CreateProcessW dùng danh sách này.
    fn with_parent(parent: &HANDLE) -> windows::core::Result<Self> {
        let mut size = 0usize;
        // SAFETY: lần gọi đầu chỉ hỏi kích thước (lỗi ERROR_INSUFFICIENT_BUFFER là bình thường).
        let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut size) };
        let mut list = Self { buf: vec![0usize; size.div_ceil(size_of::<usize>()).max(1)] };
        // SAFETY: buffer có ít nhất `size` byte, căn lề usize.
        unsafe { InitializeProcThreadAttributeList(Some(list.ptr()), 1, None, &mut size)? };
        // SAFETY: danh sách vừa khởi tạo; giá trị là con trỏ tới một HANDLE sống tới hết CreateProcessW.
        unsafe {
            UpdateProcThreadAttribute(
                list.ptr(),
                0,
                PROC_THREAD_ATTRIBUTE_PARENT_PROCESS as usize,
                Some(std::ptr::from_ref(parent).cast::<c_void>()),
                size_of::<HANDLE>(),
                None,
                None,
            )?
        };
        Ok(list)
    }
}

impl Drop for AttrList {
    fn drop(&mut self) {
        // SAFETY: danh sách đã được InitializeProcThreadAttributeList khởi tạo thành công.
        unsafe { DeleteProcThreadAttributeList(self.ptr()) };
    }
}

/// Chạy `program args` dưới user đang đăng nhập (token của Explorer), với biến môi trường `env`.
/// `extra_flags`: cờ CreateProcess thêm (vd. CREATE_NO_WINDOW cho test). Trả về PID.
pub fn spawn_as_shell_user(
    program: &Path,
    args: &str,
    env: &[(String, String)],
    extra_flags: u32,
) -> windows::core::Result<u32> {
    let parent = open_shell_process()?;
    let parent_raw = parent.raw();
    let mut attrs = AttrList::with_parent(&parent_raw)?;

    let program_s = program.display().to_string();
    let app = wide(&program_s);
    let mut cmd = wide(&format!("\"{program_s}\" {args}"));
    let env = env_block(env);
    let cwd = program.parent().map(|d| wide(&d.display().to_string()));

    let mut si = STARTUPINFOEXW::default();
    si.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    si.lpAttributeList = attrs.ptr();
    let mut pi = PROCESS_INFORMATION::default();
    let flags = EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(extra_flags);
    // SAFETY: mọi chuỗi kết thúc NUL và sống tới hết lời gọi; `cmd` ghi được (CreateProcessW có thể
    // sửa); `env` là khối UTF-16 kết thúc bằng hai NUL đúng với CREATE_UNICODE_ENVIRONMENT; `si`
    // có cb đúng của STARTUPINFOEXW và danh sách thuộc tính còn sống (attrs, parent).
    unsafe {
        CreateProcessW(
            PCWSTR(app.as_ptr()),
            Some(PWSTR(cmd.as_mut_ptr())),
            None,
            None,
            false,
            flags,
            Some(env.as_ptr().cast::<c_void>()),
            cwd.as_ref().map_or(PCWSTR::null(), |c| PCWSTR(c.as_ptr())),
            &si.StartupInfo,
            &mut pi,
        )?
    };
    // SAFETY: CreateProcessW thành công trả về hai handle thuộc về ta; bọc để tự đóng.
    unsafe {
        drop(OwnedHandle::from_raw(pi.hThread));
        drop(OwnedHandle::from_raw(pi.hProcess));
    }
    drop(attrs);
    drop(parent);
    Ok(pi.dwProcessId)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_block_is_sorted_and_double_nul_terminated() {
        let b = env_block(&[("b".into(), "2".into()), ("A".into(), "1".into()), ("=C:".into(), "x".into())]);
        let s = String::from_utf16(&b).unwrap();
        assert_eq!(s, "A=1\0b=2\0\0");
        assert_eq!(String::from_utf16(&env_block(&[])).unwrap(), "\0\0");
    }

    #[test]
    fn runs_under_explorer_with_given_environment() {
        // Cùng user: tiến trình con có cha là Explorer và nhận đúng biến môi trường truyền vào.
        // %TEMP% của user đang đăng nhập: tiến trình con chạy dưới user đó phải ghi được vào đây.
        let temp = crate::user::env("TEMP").map_or_else(std::env::temp_dir, std::path::PathBuf::from);
        let dir = temp.join(format!("pcm-launch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out.txt");
        let mut env: Vec<(String, String)> = std::env::vars().collect();
        env.push(("PCM_LAUNCH_TEST".into(), "xin chào".into()));
        let cmd = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap()).join(r"System32\cmd.exe");
        let pid = spawn_as_shell_user(&cmd, &format!("/d /c echo %PCM_LAUNCH_TEST%> \"{}\"", out.display()), &env, 0x0800_0000).unwrap();
        let ppid = crate::sys::process::enumerate().unwrap().into_iter().find(|p| p.pid == pid).map(|p| p.ppid);
        assert!(crate::sys::process::wait_exit(pid, std::time::Duration::from_secs(10)));
        let text = std::fs::read(&out).unwrap();
        assert!(!text.is_empty(), "cmd đã chạy và đọc được biến môi trường");
        if let Some(ppid) = ppid {
            let mut shell_pid = 0u32;
            // SAFETY: như trong open_shell_process.
            unsafe { GetWindowThreadProcessId(GetShellWindow(), Some(&mut shell_pid)) };
            assert_eq!(ppid, shell_pid, "cha là Explorer");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
