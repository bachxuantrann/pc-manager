//! Lớp an toàn của Cleaner — kiểm tra ĐỘC LẬP với rule, áp dụng cho mọi file
//! cả lúc quét lẫn ngay trước khi xoá (CLAUDE.md, AN TOÀN #3, #4).

use std::fs::Metadata;
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, Prefix};
use std::time::{Duration, SystemTime};

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

/// Symlink, junction, mount point, file đám mây (OneDrive "chỉ online")…
/// Không bao giờ đi vào / xoá. `meta` phải lấy bằng `symlink_metadata` hoặc
/// `DirEntry::metadata` (cả hai đều KHÔNG đi theo link trên Windows).
pub fn is_link_or_cloud(meta: &Metadata) -> bool {
    meta.file_attributes()
        & (FILE_ATTRIBUTE_REPARSE_POINT
            | FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
        || meta.file_type().is_symlink()
}

/// Tuổi tính theo mốc MỚI NHẤT trong (ngày tạo, ngày sửa). File vừa được
/// copy vào (ngày sửa cũ nhưng ngày tạo mới) vẫn được coi là mới.
pub fn age(now: SystemTime, created: Option<SystemTime>, modified: Option<SystemTime>) -> Option<Duration> {
    let newest = match (created, modified) {
        (Some(c), Some(m)) => c.max(m),
        (Some(t), None) | (None, Some(t)) => t,
        // Không biết tuổi => không được coi là đủ tuổi.
        (None, None) => return None,
    };
    // Thời gian ở tương lai (đồng hồ lệch) => tuổi 0.
    Some(now.duration_since(newest).unwrap_or(Duration::ZERO))
}

pub fn old_enough(meta: &Metadata, now: SystemTime, min_age_hours: u32) -> bool {
    let min = Duration::from_secs(u64::from(min_age_hours) * 3600);
    age(now, meta.created().ok(), meta.modified().ok()).is_some_and(|a| a >= min)
}

/// Các thư mục dùng để dựng danh sách cấm (tiêm vào để test).
#[derive(Debug, Clone, Default)]
pub struct BanCtx {
    pub windows: String,
    pub user_profile: String,
    pub users_root: String,
    pub program_data: String,
    pub program_files: Vec<String>,
    /// Documents, Desktop, Pictures… (kể cả khi đã bị chuyển sang ổ khác).
    pub user_folders: Vec<String>,
}

impl BanCtx {
    /// Theo user đang đăng nhập (kể cả khi app được nâng quyền bằng tài khoản admin khác).
    pub fn from_env() -> Self {
        let env = |k: &str| crate::user::env(k).unwrap_or_default();
        let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let profile = env("USERPROFILE");
        let users_root = Path::new(&profile).parent().map(|p| p.display().to_string()).unwrap_or_default();
        let mut user_folders: Vec<String> = ["Documents", "Desktop", "Pictures", "Videos", "Music", "Downloads", "OneDrive", "Favorites"]
            .iter()
            .filter(|_| !profile.is_empty())
            .map(|f| format!(r"{profile}\{f}"))
            .collect();
        for k in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial", "PUBLIC"] {
            let v = env(k);
            if !v.is_empty() {
                user_folders.push(v);
            }
        }
        user_folders.extend(redirected_known_folders());
        // Nâng quyền bằng tài khoản admin khác: Dọn rác chỉ làm cho user đang đăng nhập —
        // cấm hẳn toàn bộ profile của tài khoản admin.
        if crate::user::is_other_user() {
            if let Ok(admin_profile) = std::env::var("USERPROFILE") {
                if !admin_profile.is_empty() {
                    user_folders.push(admin_profile);
                }
            }
        }
        Self {
            windows,
            user_profile: profile,
            users_root,
            program_data: env("ProgramData"),
            program_files: vec![env("ProgramFiles"), env("ProgramFiles(x86)"), env("ProgramW6432")],
            user_folders,
        }
    }
}

/// Known folder bị chuyển chỗ (vd Documents nằm ở D:\Docs) — đọc từ registry.
fn redirected_known_folders() -> Vec<String> {
    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders";
    const NAMES: &[&str] = &[
        "Personal",
        "Desktop",
        "My Pictures",
        "My Video",
        "My Music",
        "{374DE290-123F-4565-9164-39C4925E467B}", // Downloads
        "Favorites",
    ];
    let Ok(key) = crate::user::hkcu().and_then(|k| k.open(KEY)) else { return Vec::new() };
    NAMES
        .iter()
        .filter_map(|n| key.get_string(n).ok())
        .map(|v| crate::startup::command::expand_env(&v, crate::user::env))
        .filter(|v| !v.contains('%'))
        .collect()
}

pub struct BanList {
    /// Cấm chính nó và MỌI thứ bên trong.
    under: Vec<(String, &'static str)>,
    /// Cấm chính nó và con trực tiếp (file nằm ngay ở gốc).
    shallow: Vec<(String, &'static str)>,
    /// Chỉ cấm đúng đường dẫn đó.
    exact: Vec<(String, &'static str)>,
}

pub fn norm(s: &str) -> String {
    let s = s.replace('/', "\\");
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    s.trim_end_matches('\\').to_lowercase()
}

impl BanList {
    pub fn new(ctx: &BanCtx) -> Self {
        let w = &ctx.windows;
        let mut under = vec![
            (format!(r"{w}\System32"), "thư mục hệ thống System32"),
            (format!(r"{w}\SysWOW64"), "thư mục hệ thống SysWOW64"),
            (format!(r"{w}\WinSxS"), "kho thành phần WinSxS (chỉ dọn bằng DISM)"),
            (format!(r"{w}\Prefetch"), "thư mục Prefetch"),
            (format!(r"{w}\Boot"), "thư mục khởi động"),
        ];
        under.extend(ctx.program_files.iter().map(|p| (p.clone(), "thư mục cài đặt ứng dụng")));
        under.extend(ctx.user_folders.iter().map(|p| (p.clone(), "thư mục dữ liệu cá nhân")));
        let shallow = vec![
            (ctx.user_profile.clone(), "gốc thư mục người dùng"),
            (ctx.users_root.clone(), "thư mục Users"),
            (ctx.program_data.clone(), "gốc ProgramData"),
        ];
        let exact = vec![(w.clone(), "thư mục Windows")];
        let clean = |v: Vec<(String, &'static str)>| -> Vec<(String, &'static str)> {
            v.into_iter().filter(|(p, _)| !p.trim().is_empty()).map(|(p, r)| (norm(&p), r)).collect()
        };
        Self { under: clean(under), shallow: clean(shallow), exact: clean(exact) }
    }

    pub fn from_env() -> Self {
        Self::new(&BanCtx::from_env())
    }

    /// `Some(lý do)` nếu đường dẫn bị cấm xoá.
    pub fn check(&self, path: &Path) -> Option<&'static str> {
        let mut normals = 0usize;
        for c in path.components() {
            match c {
                Component::Prefix(p) => {
                    if !matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) {
                        return Some("ổ mạng / đường dẫn đặc biệt");
                    }
                }
                Component::RootDir => {}
                Component::Normal(_) => normals += 1,
                Component::CurDir | Component::ParentDir => return Some("đường dẫn chứa '.' hoặc '..'"),
            }
        }
        if !path.is_absolute() {
            return Some("đường dẫn tương đối");
        }
        // X:\ và mọi thứ nằm ngay ở gốc ổ đĩa.
        if normals <= 1 {
            return Some("gốc ổ đĩa");
        }
        let s = norm(&path.to_string_lossy());
        if crate::settings::is_protected(&s) {
            return Some("nằm trong danh sách bảo vệ của bạn");
        }
        let parent = path.parent().map(|p| norm(&p.to_string_lossy())).unwrap_or_default();
        for (base, why) in &self.under {
            if s == *base || s.strip_prefix(base.as_str()).is_some_and(|rest| rest.starts_with('\\')) {
                return Some(why);
            }
        }
        for (base, why) in &self.shallow {
            if s == *base || parent == *base {
                return Some(why);
            }
        }
        self.exact.iter().find(|(base, _)| s == *base).map(|(_, why)| *why)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bans() -> BanList {
        BanList::new(&BanCtx {
            windows: r"C:\Windows".into(),
            user_profile: r"C:\Users\An".into(),
            users_root: r"C:\Users".into(),
            program_data: r"C:\ProgramData".into(),
            program_files: vec![r"C:\Program Files".into(), r"C:\Program Files (x86)".into()],
            user_folders: vec![
                r"C:\Users\An\Documents".into(),
                r"C:\Users\An\Desktop".into(),
                r"C:\Users\An\Pictures".into(),
                r"D:\Tai lieu".into(), // Documents bị chuyển sang D:
            ],
        })
    }

    fn banned(p: &str) -> bool {
        bans().check(Path::new(p)).is_some()
    }

    /// Danh sách cấm cứng trong CLAUDE.md, AN TOÀN #4.
    #[test]
    fn hard_ban_list_from_claude_md() {
        for p in [
            r"C:\Windows\System32",
            r"C:\Windows\System32\drivers\etc\hosts",
            r"C:\Windows\SysWOW64\kernel32.dll",
            r"C:\Windows\WinSxS\amd64_x\file.dll",
            r"C:\Windows\Prefetch\CHROME.EXE-1234.pf",
            r"C:\Users\An",
            r"C:\Users\An\ntuser.dat",
            r"C:\Users\An\Documents\bao-cao.docx",
            r"C:\Users\An\Desktop\a.lnk",
            r"C:\Users\An\Pictures\2024\anh.jpg",
            r"C:\",
            r"D:\",
            r"E:\file-o-goc.txt",
        ] {
            assert!(banned(p), "phải bị cấm: {p}");
        }
    }

    #[test]
    fn ban_is_case_insensitive_and_ignores_separators() {
        assert!(banned(r"c:\windows\SYSTEM32\x.dll"));
        assert!(banned("C:/Windows/System32/x.dll"));
        assert!(banned(r"\\?\C:\Windows\System32\x.dll"));
        assert!(banned(r"C:\Users\An\Documents\"));
    }

    #[test]
    fn prefix_match_respects_path_boundary() {
        // "System32Backup" không nằm trong System32.
        assert!(!banned(r"C:\Windows\System32Backup\x.tmp"));
        assert!(!banned(r"C:\Users\An\DocumentsOld\x.tmp"));
    }

    #[test]
    fn redirected_documents_and_other_protected_places() {
        assert!(banned(r"D:\Tai lieu\hop-dong.pdf"));
        assert!(banned(r"C:\Program Files\App\app.exe"));
        assert!(banned(r"C:\Windows"));
        assert!(banned(r"C:\Users\Binh")); // hồ sơ người dùng khác
        assert!(banned(r"C:\ProgramData\config.ini"));
    }

    #[test]
    fn traversal_relative_and_network_paths_are_banned() {
        assert!(banned(r"C:\Users\An\AppData\Local\Temp\..\..\..\Documents\x"));
        assert!(banned(r"Temp\x.tmp"));
        assert!(banned(r"\\server\share\x.tmp"));
    }

    #[test]
    fn legit_cleanup_targets_are_allowed() {
        for p in [
            r"C:\Users\An\AppData\Local\Temp\setup123\x.tmp",
            r"C:\Windows\Temp\x.log",
            r"C:\Windows\MEMORY.DMP",
            r"C:\Windows\SoftwareDistribution\Download\abc\x.cab",
            r"C:\ProgramData\Microsoft\Windows\WER\ReportArchive\x\Report.wer",
        ] {
            assert!(!banned(p), "phải được phép: {p}");
        }
    }

    #[test]
    fn age_uses_newest_timestamp() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100 * 3600);
        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(10 * 3600);
        let recent = SystemTime::UNIX_EPOCH + Duration::from_secs(99 * 3600);
        // Ngày sửa cũ nhưng vừa được copy vào (ngày tạo mới) => tuổi 1h.
        assert_eq!(age(now, Some(recent), Some(old)), Some(Duration::from_secs(3600)));
        assert_eq!(age(now, None, Some(old)), Some(Duration::from_secs(90 * 3600)));
        assert_eq!(age(now, None, None), None);
        let future = now + Duration::from_secs(60);
        assert_eq!(age(now, Some(future), None), Some(Duration::ZERO));
    }

    #[test]
    fn real_environment_bans_system32_and_profile() {
        let b = BanList::from_env();
        let sys = std::env::var("SystemRoot").unwrap();
        assert!(b.check(&Path::new(&sys).join(r"System32\notepad.exe")).is_some());
        let profile = std::env::var("USERPROFILE").unwrap();
        assert!(b.check(Path::new(&profile)).is_some());
        assert!(b.check(&Path::new(&profile).join(r"AppData\Local\Temp\x.tmp")).is_none());
    }
}
