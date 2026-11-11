//! Người dùng đang đăng nhập (chủ desktop của phiên hiện tại) so với người dùng của tiến trình.
//!
//! Máy domain thường cho user thường nâng quyền bằng tài khoản admin KHÁC (UAC hỏi user/mật
//! khẩu). Khi đó tiến trình chạy dưới tài khoản admin, nhưng WebView2 cố ý hạ quyền: các tiến
//! trình `msedgewebview2.exe` chạy dưới user đang đăng nhập, nên không đọc/ghi được thư mục dữ
//! liệu trong profile của tài khoản admin (lỗi 0x8000FFFF, hộp thoại "can't read and write to
//! its data directory"). Xem MicrosoftEdge/WebView2Feedback#932, tauri-apps/tauri#13926.

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::OnceLock;

use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, LookupAccountNameW, TokenUser, PSID, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::RemoteDesktop::{
    WTSDomainName, WTSFreeMemory, WTSQuerySessionInformationW, WTSUserName, WTS_CURRENT_SERVER_HANDLE,
    WTS_CURRENT_SESSION, WTS_INFO_CLASS,
};

use super::token::open_current_token;

/// SID dạng chuỗi ("S-1-5-21-…").
fn sid_to_string(sid: PSID) -> Option<String> {
    let mut s = PWSTR::null();
    // SAFETY: `sid` trỏ tới một SID hợp lệ do người gọi giữ sống; `s` là out-pointer.
    unsafe { ConvertSidToStringSidW(sid, &mut s) }.ok()?;
    // SAFETY: lời gọi thành công => `s` là chuỗi UTF-16 kết thúc NUL do LocalAlloc cấp;
    // đọc xong thì trả bộ nhớ bằng LocalFree đúng một lần.
    unsafe {
        let out = s.to_string().ok();
        let _ = LocalFree(Some(HLOCAL(s.0.cast::<c_void>())));
        out
    }
}

/// SID của tài khoản chạy tiến trình hiện tại.
fn process_user_sid() -> Option<String> {
    let token = open_current_token(TOKEN_QUERY).ok()?;
    let mut len = 0u32;
    // SAFETY: gọi lần đầu chỉ để lấy kích thước buffer (lỗi ERROR_INSUFFICIENT_BUFFER là bình thường).
    let _ = unsafe { GetTokenInformation(token.raw(), TokenUser, None, 0, &mut len) };
    if len == 0 {
        return None;
    }
    // Buffer u64 để đủ căn lề cho TOKEN_USER (chứa con trỏ).
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: buffer có ít nhất `len` byte, căn lề 8; token còn sống trong suốt lời gọi.
    unsafe { GetTokenInformation(token.raw(), TokenUser, Some(buf.as_mut_ptr().cast::<c_void>()), len, &mut len) }
        .ok()?;
    // SAFETY: GetTokenInformation(TokenUser) vừa ghi một TOKEN_USER vào đầu buffer; SID nó trỏ
    // tới nằm trong cùng buffer, còn sống tới hết hàm.
    let user = unsafe { &*buf.as_ptr().cast::<TOKEN_USER>() };
    sid_to_string(user.User.Sid)
}

/// SID của tài khoản chạy tiến trình (cho test).
#[cfg(test)]
pub fn current_user_sid() -> Option<String> {
    process_user_sid()
}

/// Một trường thông tin của phiên đang chạy tiến trình (tên user, domain).
fn session_info(class: WTS_INFO_CLASS) -> Option<String> {
    let mut p = PWSTR::null();
    let mut bytes = 0u32;
    // SAFETY: các out-pointer hợp lệ trên stack; WTS_CURRENT_SERVER_HANDLE là máy cục bộ.
    unsafe { WTSQuerySessionInformationW(Some(WTS_CURRENT_SERVER_HANDLE), WTS_CURRENT_SESSION, class, &mut p, &mut bytes) }
        .ok()?;
    // SAFETY: lời gọi thành công => `p` là chuỗi UTF-16 kết thúc NUL do WTS cấp; trả bằng
    // WTSFreeMemory đúng một lần sau khi đọc.
    unsafe {
        let out = p.to_string().ok();
        WTSFreeMemory(p.0.cast::<c_void>());
        out.filter(|s| !s.is_empty())
    }
}

/// SID của tài khoản `domain\name`.
fn account_sid(domain: &str, name: &str) -> Option<String> {
    let full = HSTRING::from(format!("{domain}\\{name}"));
    let mut sid_len = 0u32;
    let mut dom_len = 0u32;
    let mut kind = SID_NAME_USE::default();
    // SAFETY: gọi lần đầu chỉ để lấy kích thước buffer; `full` sống tới hết lời gọi.
    let _ = unsafe { LookupAccountNameW(PCWSTR::null(), &full, None, &mut sid_len, None, &mut dom_len, &mut kind) };
    if sid_len == 0 {
        return None;
    }
    let mut sid = vec![0u64; (sid_len as usize).div_ceil(8)];
    let mut dom = vec![0u16; dom_len.max(1) as usize];
    let psid = PSID(sid.as_mut_ptr().cast::<c_void>());
    // SAFETY: buffer SID ≥ `sid_len` byte, buffer domain ≥ `dom_len` ký tự, đều sống tới hết hàm.
    unsafe {
        LookupAccountNameW(PCWSTR::null(), &full, Some(psid), &mut sid_len, Some(PWSTR(dom.as_mut_ptr())), &mut dom_len, &mut kind)
    }
    .ok()?;
    sid_to_string(psid)
}

/// Thay `%BIẾN%` bằng biến môi trường (ProfileImagePath dạng `%SystemDrive%\Users\x`).
fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(rest.get(..start).unwrap_or_default());
        let after = rest.get(start + 1..).unwrap_or_default();
        match after.find('%') {
            Some(end) => {
                let name = after.get(..end).unwrap_or_default();
                match std::env::var(name) {
                    Ok(v) if !name.is_empty() => out.push_str(&v),
                    _ => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = after.get(end + 1..).unwrap_or_default();
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Thư mục shell `value` ("Local AppData", "AppData") của user có SID `sid` (hive của user
/// đang đăng nhập đã được nạp); dự phòng: ProfileImagePath + `fallback` (vd. `AppData\Local`).
pub fn shell_folder_of(sid: &str, value: &str, fallback: &str) -> Option<PathBuf> {
    let shell = format!(r"{sid}\Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Folders");
    if let Ok(p) = windows_registry::USERS.open(&shell).and_then(|k| k.get_string(value)) {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    Some(profile_dir_of(sid)?.join(fallback))
}

/// SID của user đang đăng nhập, CHỈ khi khác user chạy tiến trình (vd. nâng quyền bằng tài
/// khoản admin domain khác). Tính một lần (tra cứu tài khoản domain có thể chậm).
pub fn other_interactive_user_sid() -> Option<&'static str> {
    static SID: OnceLock<Option<String>> = OnceLock::new();
    SID.get_or_init(|| {
        let name = session_info(WTSUserName)?;
        let domain = session_info(WTSDomainName).unwrap_or_default();
        let session_sid = account_sid(&domain, &name)?;
        let process_sid = process_user_sid()?;
        (!session_sid.eq_ignore_ascii_case(&process_sid)).then_some(session_sid)
    })
    .as_deref()
}

/// Thư mục profile (`C:\Users\x`) của user có SID `sid`.
pub fn profile_dir_of(sid: &str) -> Option<PathBuf> {
    let list = format!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid}");
    let profile = windows_registry::LOCAL_MACHINE.open(&list).and_then(|k| k.get_string("ProfileImagePath")).ok()?;
    Some(PathBuf::from(expand_env(&profile)))
}

/// `DOMAIN\tên` của user đang đăng nhập phiên này.
pub fn session_account() -> Option<String> {
    let name = session_info(WTSUserName)?;
    Some(match session_info(WTSDomainName) {
        Some(d) => format!(r"{d}\{name}"),
        None => name,
    })
}

/// `AppData\Local` của user đang đăng nhập khi tiến trình chạy dưới tài khoản khác; cùng user => `None`.
pub fn other_interactive_user_local_appdata() -> Option<PathBuf> {
    shell_folder_of(other_interactive_user_sid()?, "Local AppData", r"AppData\Local")
}

/// `AppData\Roaming` của user đang đăng nhập khi tiến trình chạy dưới tài khoản khác; cùng user => `None`.
///
/// Tài khoản admin chỉ dùng để lấy quyền ở hộp thoại UAC: cài đặt, lịch sử, log của app vẫn
/// thuộc về người đang ngồi trước máy.
pub fn other_interactive_user_roaming_appdata() -> Option<PathBuf> {
    shell_folder_of(other_interactive_user_sid()?, "AppData", r"AppData\Roaming")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_known_vars_and_keeps_unknown() {
        let drive = std::env::var("SystemDrive").unwrap_or_default();
        assert_eq!(expand_env(r"%SystemDrive%\Users\a"), format!(r"{drive}\Users\a"));
        assert_eq!(expand_env(r"%KHONG_CO_BIEN_NAY%\x"), r"%KHONG_CO_BIEN_NAY%\x");
        assert_eq!(expand_env("50% off"), "50% off");
        assert_eq!(expand_env(r"C:\Users\ánh"), r"C:\Users\ánh");
    }

    #[test]
    fn same_user_is_not_reported() {
        // Test chạy dưới chính user đang đăng nhập => không có "user khác".
        assert!(other_interactive_user_local_appdata().is_none());
        assert!(other_interactive_user_roaming_appdata().is_none());
    }

    #[test]
    fn session_user_resolves_to_own_sid() {
        // Bảo đảm `same_user_is_not_reported` đạt vì so sánh đúng, không phải vì tra cứu lỗi.
        let name = session_info(WTSUserName).expect("đọc được tên user của phiên");
        let domain = session_info(WTSDomainName).unwrap_or_default();
        assert_eq!(account_sid(&domain, &name), process_user_sid());
    }

    #[test]
    fn reads_own_sid_and_profile() {
        let sid = process_user_sid().expect("đọc được SID của tiến trình");
        assert!(sid.starts_with("S-1-"));
        let local = shell_folder_of(&sid, "Local AppData", r"AppData\Local").expect("tìm được AppData\\Local");
        assert!(local.is_dir(), "{}", local.display());
        let roaming = shell_folder_of(&sid, "AppData", r"AppData\Roaming").expect("tìm được AppData\\Roaming");
        assert!(roaming.is_dir(), "{}", roaming.display());
    }
}
