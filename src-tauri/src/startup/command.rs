//! Tách đường dẫn file thực thi từ dòng lệnh khởi động (logic thuần, test được).
//!
//! Dòng lệnh trong Run key rất lộn xộn:
//! - `"C:\Program Files\App\app.exe" --minimized`
//! - `C:\Program Files\App\app.exe -silent` (không ngoặc, có dấu cách)
//! - `%ProgramFiles%\App\app.exe`
//! - `rundll32.exe C:\x\y.dll,Entry` (file thật là DLL)
//! - `ctfmon.exe` (chỉ tên, nằm trong System32)

use std::path::{Path, PathBuf};

/// Thay `%VAR%` bằng giá trị từ `lookup`; biến không biết thì giữ nguyên.
pub fn expand_env(s: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match lookup(name) {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Tìm file thực thi trong dòng lệnh đã expand.
///
/// - `exists`: kiểm tra file tồn tại (tiêm vào để test).
/// - `search_dirs`: nơi tìm khi dòng lệnh chỉ có tên file (System32, Windows).
pub fn executable_path(cmd: &str, exists: &dyn Fn(&Path) -> bool, search_dirs: &[PathBuf]) -> Option<PathBuf> {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return None;
    }
    let (exe, args) = split_exe(cmd, exists);
    let exe = resolve_bare(exe, exists, search_dirs);

    // rundll32 chỉ là vỏ — publisher thật là DLL trong tham số.
    let is_rundll = exe
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("rundll32.exe") || n.eq_ignore_ascii_case("rundll32"));
    if is_rundll {
        let dll = args.trim().trim_start_matches('"');
        let dll = dll.split([',', '"']).next().unwrap_or("").trim();
        if !dll.is_empty() {
            return Some(resolve_bare(PathBuf::from(dll), exists, search_dirs));
        }
    }
    Some(exe)
}

/// Tách (exe, phần còn lại).
fn split_exe<'a>(cmd: &'a str, exists: &dyn Fn(&Path) -> bool) -> (PathBuf, &'a str) {
    if let Some(stripped) = cmd.strip_prefix('"') {
        return match stripped.find('"') {
            Some(end) => (PathBuf::from(&stripped[..end]), &stripped[end + 1..]),
            None => (PathBuf::from(stripped), ""),
        };
    }
    // Không có ngoặc: thử ghép dần từng token cho tới khi ra file tồn tại
    // (giống cách CreateProcess đoán "C:\Program Files\...").
    let mut ends: Vec<usize> = cmd.match_indices(' ').map(|(i, _)| i).collect();
    ends.push(cmd.len());
    for &end in &ends {
        let candidate = Path::new(&cmd[..end]);
        if exists(candidate) || exists(&with_exe(candidate)) {
            return (PathBuf::from(&cmd[..end]), &cmd[end..]);
        }
    }
    // Không file nào tồn tại: cắt sau ".exe" đầu tiên, hoặc lấy token đầu.
    let lower = cmd.to_ascii_lowercase();
    if let Some(i) = lower.find(".exe") {
        let end = i + 4;
        return (PathBuf::from(&cmd[..end]), &cmd[end..]);
    }
    let end = cmd.find(' ').unwrap_or(cmd.len());
    (PathBuf::from(&cmd[..end]), &cmd[end..])
}

fn with_exe(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".exe");
    PathBuf::from(s)
}

/// Tên trần (không có thư mục) => tìm trong `search_dirs`, thử thêm ".exe".
fn resolve_bare(p: PathBuf, exists: &dyn Fn(&Path) -> bool, search_dirs: &[PathBuf]) -> PathBuf {
    let bare = p.parent().is_none_or(|d| d.as_os_str().is_empty());
    if bare {
        for dir in search_dirs {
            for cand in [dir.join(&p), with_exe(&dir.join(&p))] {
                if exists(&cand) {
                    return cand;
                }
            }
        }
    } else if !exists(&p) && exists(&with_exe(&p)) {
        return with_exe(&p);
    }
    p
}

/// Chuẩn hoá ImagePath của service: `\SystemRoot\…`, `\??\C:\…`, `system32\…`.
pub fn normalize_image_path(raw: &str, windows_dir: &str) -> String {
    let s = raw.trim();
    let s = s.strip_prefix(r"\??\").unwrap_or(s);
    if let Some(rest) = strip_prefix_ci(s, r"\SystemRoot\") {
        return format!(r"{windows_dir}\{rest}");
    }
    if strip_prefix_ci(s, r"system32\").is_some() || strip_prefix_ci(s, r"syswow64\").is_some() {
        return format!(r"{windows_dir}\{s}");
    }
    s.to_owned()
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &s[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn fs(files: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<String> = files.iter().map(|f| f.to_ascii_lowercase()).collect();
        move |p: &Path| set.contains(&p.to_string_lossy().to_ascii_lowercase())
    }

    fn sys() -> Vec<PathBuf> {
        vec![PathBuf::from(r"C:\Windows\System32"), PathBuf::from(r"C:\Windows")]
    }

    fn exe(cmd: &str, files: &[&str]) -> Option<String> {
        executable_path(cmd, &fs(files), &sys()).map(|p| p.display().to_string())
    }

    #[test]
    fn expands_known_vars_and_keeps_unknown() {
        let env = |k: &str| match k.to_ascii_uppercase().as_str() {
            "PROGRAMFILES" => Some(r"C:\Program Files".to_owned()),
            _ => None,
        };
        assert_eq!(expand_env(r"%ProgramFiles%\A\a.exe", env), r"C:\Program Files\A\a.exe");
        assert_eq!(expand_env("%NOPE%\\x 100%", env), "%NOPE%\\x 100%");
        assert_eq!(expand_env("no vars", env), "no vars");
    }

    #[test]
    fn quoted_path_with_args() {
        assert_eq!(
            exe(r#""C:\Program Files\Steam\steam.exe" -silent"#, &[]).as_deref(),
            Some(r"C:\Program Files\Steam\steam.exe")
        );
    }

    #[test]
    fn unquoted_path_with_spaces_uses_existing_file() {
        let files = [r"C:\Program Files\My App\app.exe"];
        assert_eq!(
            exe(r"C:\Program Files\My App\app.exe --tray", &files).as_deref(),
            Some(r"C:\Program Files\My App\app.exe")
        );
        // Không có ".exe" trong lệnh nhưng file app.exe tồn tại.
        assert_eq!(
            exe(r"C:\Program Files\My App\app --tray", &files).as_deref(),
            Some(r"C:\Program Files\My App\app.exe")
        );
    }

    #[test]
    fn unquoted_fallback_cuts_after_exe() {
        assert_eq!(
            exe(r"C:\Program Files\Gone\gone.exe /background", &[]).as_deref(),
            Some(r"C:\Program Files\Gone\gone.exe")
        );
    }

    #[test]
    fn bare_name_found_in_system32() {
        let files = [r"C:\Windows\System32\ctfmon.exe"];
        assert_eq!(exe("ctfmon.exe", &files).as_deref(), Some(r"C:\Windows\System32\ctfmon.exe"));
        assert_eq!(exe("ctfmon", &files).as_deref(), Some(r"C:\Windows\System32\ctfmon.exe"));
    }

    #[test]
    fn rundll32_reports_the_dll() {
        let files = [r"C:\Windows\System32\rundll32.exe"];
        assert_eq!(
            exe(r#"rundll32.exe "C:\Program Files\X\helper.dll",Start"#, &files).as_deref(),
            Some(r"C:\Program Files\X\helper.dll")
        );
        assert_eq!(exe(r"rundll32 C:\X\a.dll,Run", &files).as_deref(), Some(r"C:\X\a.dll"));
    }

    #[test]
    fn empty_command() {
        assert_eq!(exe("   ", &[]), None);
    }

    #[test]
    fn service_image_paths() {
        let w = r"C:\Windows";
        assert_eq!(normalize_image_path(r"\SystemRoot\System32\drivers\x.sys", w), r"C:\Windows\System32\drivers\x.sys");
        assert_eq!(normalize_image_path(r"system32\svchost.exe -k netsvcs", w), r"C:\Windows\system32\svchost.exe -k netsvcs");
        assert_eq!(normalize_image_path(r"\??\C:\X\y.exe", w), r"C:\X\y.exe");
        assert_eq!(normalize_image_path(r#""C:\Program Files\S\s.exe""#, w), r#""C:\Program Files\S\s.exe""#);
    }
}
