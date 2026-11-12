//! Ngữ cảnh "người dùng đang đăng nhập".
//!
//! Máy domain: user thường nâng quyền bằng tài khoản admin KHÁC ở hộp thoại UAC. Tài khoản đó
//! chỉ để lấy quyền — mọi thứ "của người dùng" (biến môi trường, HKCU, Temp, thư mục Startup,
//! Thùng rác, cài đặt của app) phải là của người đang ngồi trước máy, không phải của tài khoản
//! admin. Cùng một user (trường hợp thường) => giữ nguyên hành vi cũ: env của tiến trình, HKCU.

pub mod recycle;

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use windows_registry::{Key, CURRENT_USER, USERS};

use crate::startup::command::expand_env;
use crate::sys::session;

/// Biến môi trường gắn với từng user: KHÔNG bao giờ lấy từ tiến trình (của tài khoản admin).
const PER_USER_VARS: &[&str] = &[
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "HOMEDRIVE",
    "HOMEPATH",
    "HOMESHARE",
    "USERNAME",
    "USERDOMAIN",
    "USERDNSDOMAIN",
    "USERDOMAIN_ROAMINGPROFILE",
    "LOGONSERVER",
    "ONEDRIVE",
    "ONEDRIVECONSUMER",
    "ONEDRIVECOMMERCIAL",
    "TEMP",
    "TMP",
];

struct Other {
    sid: String,
    account: String,
    /// Tên biến viết HOA -> giá trị (đã thay %BIẾN%).
    vars: HashMap<String, String>,
    /// Biến riêng của tài khoản admin (HKCU\Environment của nó) mà user đang đăng nhập không có.
    admin_only: HashSet<String>,
}

fn other() -> Option<&'static Other> {
    static OTHER: OnceLock<Option<Other>> = OnceLock::new();
    OTHER.get_or_init(load_other).as_ref()
}

fn string_values(key: &Key) -> Vec<(String, String)> {
    let Ok(values) = key.values() else { return Vec::new() };
    values.filter_map(|(n, v)| Some((n, String::try_from(v).ok()?))).filter(|(n, _)| !n.is_empty()).collect()
}

fn load_other() -> Option<Other> {
    let sid = session::other_interactive_user_sid()?.to_owned();
    let account = session::session_account().unwrap_or_default();
    let mut fixed: Vec<(String, String)> = Vec::new();
    let mut push = |k: &str, v: Option<std::path::PathBuf>| {
        if let Some(v) = v {
            fixed.push((k.to_owned(), v.display().to_string()));
        }
    };
    push("USERPROFILE", session::profile_dir_of(&sid));
    push("APPDATA", session::shell_folder_of(&sid, "AppData", r"AppData\Roaming"));
    push("LOCALAPPDATA", session::shell_folder_of(&sid, "Local AppData", r"AppData\Local"));
    if let Some((domain, name)) = account.split_once('\\') {
        fixed.push(("USERDOMAIN".into(), domain.to_owned()));
        fixed.push(("USERNAME".into(), name.to_owned()));
    }
    let mut raw = Vec::new();
    for sub in ["Volatile Environment", "Environment"] {
        if let Ok(k) = USERS.open(format!(r"{sid}\{sub}")) {
            raw.extend(string_values(&k));
        }
    }
    let vars = resolve_vars(&fixed, &raw, &|k| std::env::var(k).ok());
    let admin_only = CURRENT_USER
        .open("Environment")
        .map(|k| string_values(&k))
        .unwrap_or_default()
        .into_iter()
        .map(|(n, _)| n.to_ascii_uppercase())
        .filter(|n| !vars.contains_key(n))
        .collect();
    Some(Other { sid, account, vars, admin_only })
}

/// Gộp biến của user: `fixed` (profile, AppData… đọc từ hệ thống) ưu tiên hơn `raw` (đọc từ
/// `HKU\<sid>\Environment`, có thể chứa `%USERPROFILE%`…). Thay `%BIẾN%` bằng chính các biến
/// của user trước, rồi mới tới biến của máy (`machine`) — trừ biến gắn với user.
fn resolve_vars(
    fixed: &[(String, String)],
    raw: &[(String, String)],
    machine: &dyn Fn(&str) -> Option<String>,
) -> HashMap<String, String> {
    let mut vars: HashMap<String, String> = HashMap::new();
    for (k, v) in fixed.iter().chain(raw) {
        vars.entry(k.to_ascii_uppercase()).or_insert_with(|| v.clone());
    }
    let snapshot = vars.clone();
    let lookup = |k: &str| {
        let up = k.to_ascii_uppercase();
        snapshot.get(&up).cloned().or_else(|| (!PER_USER_VARS.contains(&up.as_str())).then(|| machine(k)).flatten())
    };
    for v in vars.values_mut() {
        if v.contains('%') {
            *v = expand_env(v, lookup);
        }
    }
    if let Some(local) = vars.get("LOCALAPPDATA").cloned() {
        for t in ["TEMP", "TMP"] {
            vars.entry(t.into()).or_insert_with(|| format!(r"{local}\Temp"));
        }
    }
    vars
}

/// Đọc một biến từ ngữ cảnh đã có (tách ra để test).
fn lookup_in(o: &Other, key: &str, machine: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let up = key.to_ascii_uppercase();
    if let Some(v) = o.vars.get(&up) {
        return Some(v.clone());
    }
    if PER_USER_VARS.contains(&up.as_str()) || o.admin_only.contains(&up) {
        return None;
    }
    machine(key)
}

/// Biến môi trường như của user đang đăng nhập (`%TEMP%`, `%LOCALAPPDATA%`, `%USERPROFILE%`…).
pub fn env(key: &str) -> Option<String> {
    match other() {
        Some(o) => lookup_in(o, key, &|k| std::env::var(k).ok()),
        None => std::env::var(key).ok(),
    }
}

/// Toàn bộ biến môi trường cho một tiến trình con chạy dưới user đang đăng nhập: biến của máy
/// (lấy từ tiến trình) + biến của user đó; bỏ biến riêng của tài khoản admin.
pub fn environment() -> Vec<(String, String)> {
    let process: Vec<(String, String)> = std::env::vars().collect();
    match other() {
        Some(o) => merge_environment(o, process),
        None => process,
    }
}

fn merge_environment(o: &Other, process: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = process
        .into_iter()
        .filter(|(k, _)| {
            let up = k.to_ascii_uppercase();
            !PER_USER_VARS.contains(&up.as_str()) && !o.admin_only.contains(&up) && !o.vars.contains_key(&up)
        })
        .collect();
    out.extend(o.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
    out
}

/// Tiến trình đang chạy dưới tài khoản khác user đang đăng nhập.
pub fn is_other_user() -> bool {
    other().is_some()
}

/// SID của user đang đăng nhập khi khác tài khoản chạy tiến trình.
pub fn other_sid() -> Option<&'static str> {
    other().map(|o| o.sid.as_str())
}

/// `DOMAIN\tên` của user đang đăng nhập khi khác tài khoản chạy tiến trình.
pub fn other_account() -> Option<&'static str> {
    other().map(|o| o.account.as_str()).filter(|a| !a.is_empty())
}

/// HKCU của user đang đăng nhập (`HKEY_USERS\<SID>` khi tiến trình chạy dưới tài khoản khác).
pub fn hkcu() -> windows_registry::Result<Key> {
    match other() {
        Some(o) => USERS.open(&o.sid),
        None => CURRENT_USER.open(""),
    }
}

/// Tên gốc HKCU cho reg.exe: `HKCU` hoặc `HKU\<SID>`.
pub fn hkcu_reg_root() -> String {
    match other() {
        Some(o) => format!(r"HKU\{}", o.sid),
        None => "HKCU".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect()
    }

    #[test]
    fn user_vars_win_and_expand_from_user_profile() {
        let fixed = pairs(&[("USERPROFILE", r"C:\Users\thuong"), ("LOCALAPPDATA", r"C:\Users\thuong\AppData\Local")]);
        let raw = pairs(&[("TEMP", r"%USERPROFILE%\AppData\Local\Temp"), ("USERPROFILE", r"C:\sai"), ("Tools", r"%SystemRoot%\x")]);
        let machine = |k: &str| match k.to_ascii_uppercase().as_str() {
            "SYSTEMROOT" => Some(r"C:\Windows".to_owned()),
            "USERPROFILE" => Some(r"C:\Users\admin".to_owned()),
            _ => None,
        };
        let v = resolve_vars(&fixed, &raw, &machine);
        assert_eq!(v["USERPROFILE"], r"C:\Users\thuong");
        assert_eq!(v["TEMP"], r"C:\Users\thuong\AppData\Local\Temp");
        assert_eq!(v["TMP"], r"C:\Users\thuong\AppData\Local\Temp", "thiếu TMP => LOCALAPPDATA\\Temp");
        assert_eq!(v["TOOLS"], r"C:\Windows\x");
    }

    #[test]
    fn per_user_vars_never_come_from_admin_process() {
        let o = Other {
            sid: "S-1-5-21-1".into(),
            account: r"DC\thuong".into(),
            vars: HashMap::from([("APPDATA".to_owned(), r"C:\Users\thuong\AppData\Roaming".to_owned())]),
            admin_only: HashSet::from(["ADMINTOOL".to_owned()]),
        };
        let machine = |k: &str| Some(format!("admin:{k}"));
        assert_eq!(lookup_in(&o, "appdata", &machine).as_deref(), Some(r"C:\Users\thuong\AppData\Roaming"));
        assert_eq!(lookup_in(&o, "OneDrive", &machine), None, "biến theo user mà user không có => không mượn của admin");
        assert_eq!(lookup_in(&o, "AdminTool", &machine), None);
        assert_eq!(lookup_in(&o, "SystemRoot", &machine).as_deref(), Some("admin:SystemRoot"));
    }

    #[test]
    fn child_environment_drops_admin_values() {
        let o = Other {
            sid: "S-1-5-21-1".into(),
            account: r"DC\thuong".into(),
            vars: HashMap::from([
                ("USERPROFILE".to_owned(), r"C:\Users\thuong".to_owned()),
                ("TEMP".to_owned(), r"C:\Users\thuong\AppData\Local\Temp".to_owned()),
            ]),
            admin_only: HashSet::from(["ADMINTOOL".to_owned()]),
        };
        let process = vec![
            ("UserProfile".to_owned(), r"C:\Users\admin".to_owned()),
            ("OneDrive".to_owned(), r"C:\Users\admin\OneDrive".to_owned()),
            ("AdminTool".to_owned(), "x".to_owned()),
            ("SystemRoot".to_owned(), r"C:\Windows".to_owned()),
        ];
        let mut env = merge_environment(&o, process);
        env.sort();
        assert_eq!(
            env,
            vec![
                ("SystemRoot".to_owned(), r"C:\Windows".to_owned()),
                ("TEMP".to_owned(), r"C:\Users\thuong\AppData\Local\Temp".to_owned()),
                ("USERPROFILE".to_owned(), r"C:\Users\thuong".to_owned()),
            ]
        );
    }

    #[test]
    fn same_user_uses_process_context() {
        if is_other_user() {
            println!("bỏ qua: test đang chạy bằng tài khoản admin khác user đang đăng nhập");
            return;
        }
        assert!(!is_other_user());
        assert_eq!(env("LOCALAPPDATA"), std::env::var("LOCALAPPDATA").ok());
        assert_eq!(hkcu_reg_root(), "HKCU");
        assert!(hkcu().and_then(|k| k.open(r"Software\Microsoft")).is_ok());
    }
}
