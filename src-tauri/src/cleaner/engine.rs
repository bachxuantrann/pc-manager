//! Quét và xoá file cho MỘT rule. Không biết gì về UI / log / service.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

use super::glob;
use super::rules::Rule;
use super::safety::{self, BanList};

/// Một file tìm thấy lúc quét. `size` + `modified` dùng để phát hiện file
/// thay đổi giữa lúc quét và lúc xoá.
#[derive(Debug, Clone)]
pub struct Found {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Default)]
pub struct RuleScanData {
    pub roots: Vec<PathBuf>,
    pub files: Vec<Found>,
    /// Thư mục con (không gồm gốc) — để xoá nếu rỗng sau khi dọn.
    pub dirs: Vec<PathBuf>,
    pub skipped_young: u64,
    pub skipped_links: u64,
    pub skipped_banned: u64,
    pub read_errors: u64,
}

impl RuleScanData {
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

/// Bộ đếm tiến độ dùng chung giữa các thread.
#[derive(Default)]
pub struct Counters {
    pub files: AtomicU64,
    pub bytes: AtomicU64,
}

/// Mở rộng mẫu đường dẫn gốc: `%BIẾN%` và `*` trong từng đoạn
/// (vd `...\User Data\*\Cache` → mọi profile trình duyệt). Chỉ trả thư mục
/// thật sự tồn tại và không phải link.
pub fn expand_roots(template: &str, env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let expanded = crate::startup::command::expand_env(template, env);
    if expanded.contains('%') {
        return Vec::new(); // biến môi trường không tồn tại
    }
    let mut segs = expanded.split('\\').filter(|s| !s.is_empty());
    let Some(drive) = segs.next().filter(|d| d.len() == 2 && d.ends_with(':')) else { return Vec::new() };
    let mut bases = vec![PathBuf::from(format!("{drive}\\"))];
    for seg in segs {
        let mut next = Vec::new();
        for base in &bases {
            if glob::has_wildcard(seg) {
                let Ok(rd) = fs::read_dir(base) else { continue };
                for e in rd.flatten() {
                    let ok = e.metadata().is_ok_and(|m| m.is_dir() && !safety::is_link_or_cloud(&m));
                    if ok && glob::matches(seg, &e.file_name().to_string_lossy()) {
                        next.push(e.path());
                    }
                }
            } else {
                next.push(base.join(seg));
            }
        }
        bases = next;
    }
    bases
        .into_iter()
        .filter(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir() && !safety::is_link_or_cloud(&m)))
        .collect()
}

/// Quét các thư mục gốc theo rule. Không bao giờ đi vào link/junction.
pub fn scan(rule: &Rule, roots: Vec<PathBuf>, bans: &BanList, now: SystemTime, cancel: &AtomicBool, c: &Counters) -> RuleScanData {
    let mut out = RuleScanData { roots: roots.clone(), ..Default::default() };
    for root in roots {
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            if cancel.load(Ordering::Relaxed) {
                return out;
            }
            let Ok(rd) = fs::read_dir(&dir) else {
                out.read_errors += 1;
                continue;
            };
            // Danh sách cấm tính một lần cho cả thư mục; bước kiểm tra lại trước khi xoá vẫn từng file.
            let dir_bans = bans.for_dir(&dir);
            for entry in rd {
                let Ok(entry) = entry else {
                    out.read_errors += 1;
                    continue;
                };
                // DirEntry::metadata trên Windows KHÔNG đi theo link.
                let Ok(meta) = entry.metadata() else {
                    out.read_errors += 1;
                    continue;
                };
                if safety::is_link_or_cloud(&meta) {
                    out.skipped_links += 1;
                    continue;
                }
                let path = entry.path();
                if meta.is_dir() {
                    if rule.recursive {
                        out.dirs.push(path.clone());
                        stack.push(path);
                    }
                    continue;
                }
                if !meta.is_file() || !rule.file_matches(&entry.file_name().to_string_lossy()) {
                    continue;
                }
                if dir_bans.check_file(&path).is_some() {
                    out.skipped_banned += 1;
                    continue;
                }
                if !safety::old_enough(&meta, now, rule.min_age_hours) {
                    out.skipped_young += 1;
                    continue;
                }
                c.files.fetch_add(1, Ordering::Relaxed);
                c.bytes.fetch_add(meta.len(), Ordering::Relaxed);
                out.files.push(Found { path, size: meta.len(), modified: meta.modified().ok() });
            }
        }
    }
    out
}

#[derive(Debug, Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteStats {
    pub deleted_files: u64,
    pub freed_bytes: u64,
    /// Đang được ứng dụng khác mở / không có quyền.
    pub skipped_locked: u64,
    /// Đã thay đổi, biến mất hoặc không còn thoả điều kiện từ lúc quét.
    pub skipped_changed: u64,
    pub errors: u64,
    pub first_error: Option<String>,
}

/// Vì sao một file KHÔNG được xoá ở bước kiểm tra lại (None = được xoá).
fn recheck(f: &Found, rule: &Rule, roots: &[PathBuf], bans: &BanList, now: SystemTime) -> Option<&'static str> {
    // Phải nằm bên trong (không phải chính) một thư mục gốc của rule.
    let inside = roots.iter().any(|r| f.path.starts_with(r) && f.path != *r);
    if !inside {
        return Some("ngoài phạm vi rule");
    }
    if bans.check(&f.path).is_some() {
        return Some("bị cấm");
    }
    let name = f.path.file_name()?.to_string_lossy();
    if !rule.file_matches(&name) {
        return Some("không khớp rule");
    }
    let meta = fs::symlink_metadata(&f.path).ok()?;
    if safety::is_link_or_cloud(&meta) || !meta.is_file() {
        return Some("đã thành link");
    }
    if meta.len() != f.size || meta.modified().ok() != f.modified {
        return Some("đã thay đổi");
    }
    if !safety::old_enough(&meta, now, rule.min_age_hours) {
        return Some("còn quá mới");
    }
    // Các thư mục cha cũng không được là link (ai đó thay thư mục bằng junction).
    let mut p = f.path.parent();
    while let Some(dir) = p {
        if roots.iter().any(|r| r == dir) {
            break;
        }
        if fs::symlink_metadata(dir).map_or(true, |m| safety::is_link_or_cloud(&m)) {
            return Some("thư mục cha là link");
        }
        p = dir.parent();
    }
    None
}

/// Xoá (hoặc giả lập nếu `dry_run`) đúng các file đã quét, sau khi kiểm tra lại từng file.
#[allow(clippy::too_many_arguments)]
pub fn delete(
    rule: &Rule,
    data: &RuleScanData,
    bans: &BanList,
    now: SystemTime,
    dry_run: bool,
    cancel: &AtomicBool,
    c: &Counters,
    tick: &dyn Fn(),
) -> DeleteStats {
    let mut st = DeleteStats::default();
    for (i, f) in data.files.iter().enumerate() {
        // Báo tiến độ theo số file (rule 100.000 file thì thanh vẫn chạy); `tick` tự giới hạn tần suất.
        if i % 256 == 255 {
            tick();
        }
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if recheck(f, rule, &data.roots, bans, now).is_some() {
            st.skipped_changed += 1;
            continue;
        }
        if dry_run {
            st.deleted_files += 1;
            st.freed_bytes += f.size;
        } else {
            match fs::remove_file(&f.path) {
                Ok(()) => {
                    st.deleted_files += 1;
                    st.freed_bytes += f.size;
                }
                // 5 = ACCESS_DENIED, 32 = SHARING_VIOLATION, 33 = LOCK_VIOLATION.
                Err(e) if matches!(e.raw_os_error(), Some(5 | 32 | 33)) => st.skipped_locked += 1,
                Err(e) if e.kind() == ErrorKind::NotFound => st.skipped_changed += 1,
                Err(e) => {
                    st.errors += 1;
                    st.first_error.get_or_insert_with(|| format!("{}: {e}", f.path.display()));
                }
            }
        }
        c.files.fetch_add(1, Ordering::Relaxed);
        c.bytes.fetch_add(f.size, Ordering::Relaxed);
    }
    if rule.remove_empty_dirs && !dry_run && !cancel.load(Ordering::Relaxed) {
        remove_empty_dirs(&data.dirs, &data.roots);
    }
    st
}

/// Xoá thư mục con rỗng, sâu nhất trước. `remove_dir` tự thất bại nếu còn file.
fn remove_empty_dirs(dirs: &[PathBuf], roots: &[PathBuf]) {
    let mut sorted: Vec<&PathBuf> = dirs.iter().filter(|d| !roots.contains(d)).collect();
    sorted.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for d in sorted {
        let is_plain_dir = fs::symlink_metadata(d).is_ok_and(|m| m.is_dir() && !safety::is_link_or_cloud(&m));
        if is_plain_dir && roots.iter().any(|r| d.starts_with(r)) {
            let _ = fs::remove_dir(d);
        }
    }
}
