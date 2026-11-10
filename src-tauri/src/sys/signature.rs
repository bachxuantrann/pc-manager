//! Xác minh chữ ký Authenticode bằng `WinVerifyTrust` và lấy tên publisher.
//!
//! 1. Thử chữ ký nhúng trong file (WTD_CHOICE_FILE).
//! 2. Nếu file không có chữ ký nhúng → tìm trong catalog hệ thống
//!    (hầu hết file của Windows được ký kiểu này).
//!
//! Không kiểm tra thu hồi qua mạng (WTD_REVOKE_NONE + CACHE_ONLY_URL_RETRIEVAL)
//! để không bao giờ chặn vì mạng chậm.

use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows::core::{GUID, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Security::Cryptography::Catalog::{
    CryptCATAdminAcquireContext2, CryptCATAdminCalcHashFromFileHandle2, CryptCATAdminEnumCatalogFromHash,
    CryptCATAdminReleaseCatalogContext, CryptCATAdminReleaseContext, CryptCATCatalogInfoFromContext,
    CATALOG_INFO,
};
use windows::Win32::Security::Cryptography::{CertGetNameStringW, CERT_NAME_SIMPLE_DISPLAY_TYPE};
use windows::Win32::Security::WinTrust::{
    WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData, WinVerifyTrust,
    WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_CATALOG_INFO, WINTRUST_DATA, WINTRUST_DATA_0,
    WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_CATALOG, WTD_CHOICE_FILE,
    WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
};

/// TRUST_E_NOSIGNATURE: file không có chữ ký nhúng.
const TRUST_E_NOSIGNATURE: i32 = 0x800B_0100_u32 as i32;
/// TRUST_E_SUBJECT_FORM_UNKNOWN: loại file không chứa được chữ ký nhúng (vd .txt).
const TRUST_E_SUBJECT_FORM_UNKNOWN: i32 = 0x800B_0003_u32 as i32;
/// TRUST_E_PROVIDER_UNKNOWN.
const TRUST_E_PROVIDER_UNKNOWN: i32 = 0x800B_0001_u32 as i32;

/// Kết quả kiểm tra chữ ký.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Signature {
    /// Chữ ký hợp lệ, `name` là tên chủ chứng chỉ (vd "Microsoft Windows").
    Signed { name: String, catalog: bool },
    /// Không có chữ ký (cả nhúng lẫn catalog).
    Unsigned,
    /// Có chữ ký nhưng không hợp lệ (hết hạn, bị sửa, không tin cậy…).
    Invalid { code: String },
}

pub fn verify(path: &Path) -> Signature {
    let wpath = HSTRING::from(path.as_os_str());
    match verify_file(&wpath) {
        (0, name) => Signature::Signed { name: name.unwrap_or_default(), catalog: false },
        (TRUST_E_NOSIGNATURE | TRUST_E_SUBJECT_FORM_UNKNOWN | TRUST_E_PROVIDER_UNKNOWN, _) => match verify_catalog(path, &wpath) {
            Some((0, name)) => Signature::Signed { name: name.unwrap_or_default(), catalog: true },
            Some((code, _)) => Signature::Invalid { code: format!("0x{code:08X}") },
            None => Signature::Unsigned,
        },
        (code, _) => Signature::Invalid { code: format!("0x{code:08X}") },
    }
}

fn base_data(choice: WINTRUST_DATA_0, union_choice: windows::Win32::Security::WinTrust::WINTRUST_DATA_UNION_CHOICE) -> WINTRUST_DATA {
    WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: union_choice,
        Anonymous: choice,
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    }
}

/// Chạy WinVerifyTrust, đọc tên signer, rồi luôn đóng state (tránh leak).
fn run_verify(data: &mut WINTRUST_DATA) -> (i32, Option<String>) {
    let mut action: GUID = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // SAFETY: `data` là WINTRUST_DATA hợp lệ với union trỏ tới struct còn sống
    // trên stack của hàm gọi; HWND null = không UI.
    let status = unsafe { WinVerifyTrust(HWND::default(), &mut action, std::ptr::from_mut(data).cast::<c_void>()) };
    let name = if status == 0 { signer_name(data.hWVTStateData) } else { None };
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    // SAFETY: đóng state data do lời gọi VERIFY ở trên cấp phát.
    unsafe { WinVerifyTrust(HWND::default(), &mut action, std::ptr::from_mut(data).cast::<c_void>()) };
    (status, name)
}

fn verify_file(path: &HSTRING) -> (i32, Option<String>) {
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(path.as_ptr()),
        ..Default::default()
    };
    let mut data = base_data(WINTRUST_DATA_0 { pFile: &mut file }, WTD_CHOICE_FILE);
    run_verify(&mut data)
}

/// RAII cho context CryptCATAdmin.
struct CatAdmin(isize);
impl Drop for CatAdmin {
    fn drop(&mut self) {
        // SAFETY: `self.0` do CryptCATAdminAcquireContext2 cấp, đóng đúng 1 lần.
        unsafe {
            let _ = CryptCATAdminReleaseContext(self.0, 0);
        }
    }
}

/// RAII cho catalog context tìm được từ hash.
struct CatInfo<'a>(&'a CatAdmin, isize);
impl Drop for CatInfo<'_> {
    fn drop(&mut self) {
        // SAFETY: `self.1` do CryptCATAdminEnumCatalogFromHash cấp với admin `self.0`.
        unsafe {
            let _ = CryptCATAdminReleaseCatalogContext(self.0 .0, self.1, 0);
        }
    }
}

/// `None` = không có trong catalog nào (thực sự không ký).
fn verify_catalog(path: &Path, wpath: &HSTRING) -> Option<(i32, Option<String>)> {
    let file = File::open(path).ok()?;
    let hfile = HANDLE(file.as_raw_handle());

    let mut raw_admin = 0isize;
    // SAFETY: out-pointer hợp lệ; SHA256 là thuật toán hash catalog chuẩn từ Win8.
    unsafe {
        CryptCATAdminAcquireContext2(&mut raw_admin, None, windows::core::w!("SHA256"), None, None).ok()?;
    }
    let admin = CatAdmin(raw_admin);

    let mut hash_len = 0u32;
    // SAFETY: gọi lần 1 với pbhash = None để lấy kích thước; `hfile` còn mở.
    unsafe { CryptCATAdminCalcHashFromFileHandle2(admin.0, hfile, &mut hash_len, None, None).ok()? };
    if hash_len == 0 || hash_len > 256 {
        return None;
    }
    let mut hash = vec![0u8; hash_len as usize];
    // SAFETY: `hash` có đúng `hash_len` byte ghi được.
    unsafe { CryptCATAdminCalcHashFromFileHandle2(admin.0, hfile, &mut hash_len, Some(hash.as_mut_ptr()), None).ok()? };

    // SAFETY: admin hợp lệ, `hash` là slice đã khởi tạo.
    let raw_info = unsafe { CryptCATAdminEnumCatalogFromHash(admin.0, &hash, None, None) };
    if raw_info == 0 {
        return None;
    }
    let info = CatInfo(&admin, raw_info);

    let mut cat = CATALOG_INFO { cbStruct: size_of::<CATALOG_INFO>() as u32, wszCatalogFile: [0; 260] };
    // SAFETY: `cat` đúng kiểu, cbStruct đã đặt.
    unsafe { CryptCATCatalogInfoFromContext(info.1, &mut cat, 0).ok()? };

    let tag: Vec<u16> = hash
        .iter()
        .flat_map(|b| format!("{b:02X}").encode_utf16().collect::<Vec<_>>())
        .chain(std::iter::once(0))
        .collect();
    let mut catalog = WINTRUST_CATALOG_INFO {
        cbStruct: size_of::<WINTRUST_CATALOG_INFO>() as u32,
        pcwszCatalogFilePath: PCWSTR(cat.wszCatalogFile.as_ptr()),
        pcwszMemberTag: PCWSTR(tag.as_ptr()),
        pcwszMemberFilePath: PCWSTR(wpath.as_ptr()),
        hMemberFile: hfile,
        pbCalculatedFileHash: hash.as_mut_ptr(),
        cbCalculatedFileHash: hash_len,
        hCatAdmin: admin.0,
        ..Default::default()
    };
    let mut data = base_data(WINTRUST_DATA_0 { pCatalog: &mut catalog }, WTD_CHOICE_CATALOG);
    let result = run_verify(&mut data);
    drop(info);
    drop(file);
    Some(result)
}

/// Tên hiển thị của chứng chỉ signer đầu tiên.
fn signer_name(state: HANDLE) -> Option<String> {
    // SAFETY: `state` là hWVTStateData hợp lệ (VERIFY thành công, chưa CLOSE).
    // Các con trỏ trả về thuộc state data, sống tới khi CLOSE; được kiểm tra
    // null và chỉ số trước khi deref.
    unsafe {
        let prov = WTHelperProvDataFromStateData(state);
        if prov.is_null() {
            return None;
        }
        let sgnr = WTHelperGetProvSignerFromChain(prov, 0, false, 0);
        if sgnr.is_null() || (*sgnr).csCertChain == 0 || (*sgnr).pasCertChain.is_null() {
            return None;
        }
        let cert = (*(*sgnr).pasCertChain).pCert;
        if cert.is_null() {
            return None;
        }
        let mut buf = [0u16; 256];
        let n = CertGetNameStringW(cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None, Some(&mut buf)) as usize;
        // n tính cả NUL; n <= 1 nghĩa là chuỗi rỗng.
        (n > 1).then(|| String::from_utf16_lossy(&buf[..n.min(buf.len()) - 1]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_is_signed_by_microsoft() {
        let sig = verify(Path::new(r"C:\Windows\explorer.exe"));
        match sig {
            Signature::Signed { name, .. } => assert!(name.contains("Microsoft"), "{name}"),
            other => panic!("explorer.exe phải được ký: {other:?}"),
        }
    }

    #[test]
    fn plain_text_file_is_unsigned() {
        let p = std::env::temp_dir().join("pcm_sig_test.txt");
        std::fs::write(&p, b"hello").unwrap();
        assert_eq!(verify(&p), Signature::Unsigned);
        let _ = std::fs::remove_file(p);
    }
}
