//! Ổ đĩa: dung lượng, file lớn, file trùng.
//!
//! Ngoại lệ CLAUDE.md AN TOÀN #1: người dùng chọn file từ kết quả quét để
//! **chuyển vào Thùng rác** (FOF_ALLOWUNDO), không bao giờ xoá vĩnh viễn.
//! Kết quả quét giữ ở backend; UI chỉ gửi `scanId` + id file.

pub mod dup;
pub mod walk;

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

use rayon::prelude::*;
use serde::Serialize;

use crate::cleaner::safety::{is_link_or_cloud, norm, BanCtx};
use crate::error::{AppError, AppResult};
use crate::store::Store;
use crate::sys;
use walk::FileEntry;
use crate::sync::LockExt;

const SCAN_TTL: Duration = Duration::from_secs(30 * 60);
pub const LARGE_LIMIT: usize = 300;
pub const DUP_GROUP_LIMIT: usize = 300;

// ---- Kiểu trả về UI ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub root: String,
    /// fixed | removable | network | cdRom | ramDisk | unknown
    pub kind: &'static str,
    pub label: String,
    pub fs: String,
    pub total: u64,
    pub free: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    pub id: u32,
    pub path: String,
    pub size: u64,
    pub modified_ms: Option<i64>,
    /// Lý do không được chuyển vào Thùng rác (file hệ thống, thư mục cài app…).
    pub blocked: Option<String>,
    /// Được phép nhưng nên cân nhắc (vd. dữ liệu của ứng dụng trong AppData).
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LargeScan {
    pub scan_id: u64,
    pub root: String,
    pub files: Vec<FileItem>,
    pub scanned_files: u64,
    pub scanned_bytes: u64,
    pub duration_ms: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupGroup {
    pub size: u64,
    pub files: Vec<FileItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupScan {
    pub scan_id: u64,
    pub root: String,
    pub groups: Vec<DupGroup>,
    /// Dung lượng lấy lại được nếu chỉ giữ 1 bản mỗi nhóm.
    pub wasted_bytes: u64,
    pub scanned_files: u64,
    pub duration_ms: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskProgress {
    /// walk | head | full
    pub phase: &'static str,
    pub files: u64,
    pub bytes: u64,
    /// Tiến độ băm: đã xong / tổng (0 khi đang duyệt).
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleOutcome {
    pub id: u32,
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleReport {
    pub recycled: u32,
    pub recycled_bytes: u64,
    pub failed: u32,
    pub items: Vec<RecycleOutcome>,
}

pub fn drives() -> Vec<DriveInfo> {
    sys::disk::letters()
        .into_iter()
        .map(sys::disk::drive)
        .map(|d| DriveInfo {
            kind: match d.kind {
                sys::disk::DriveType::Fixed => "fixed",
                sys::disk::DriveType::Removable => "removable",
                sys::disk::DriveType::Network => "network",
                sys::disk::DriveType::CdRom => "cdRom",
                sys::disk::DriveType::RamDisk => "ramDisk",
                sys::disk::DriveType::Unknown => "unknown",
            },
            root: d.root,
            label: d.label,
            fs: d.fs,
            total: d.total,
            free: d.free,
        })
        .collect()
}

// ---- Lớp an toàn cho thao tác chuyển vào Thùng rác ---------------------------------

/// Chặn file hệ thống / ứng dụng; cho phép dữ liệu người dùng (Documents, Downloads…).
pub struct DiskGuard {
    /// Cấm mọi thứ bên trong.
    under: Vec<(String, &'static str)>,
    user_profile: String,
    app_data: String,
}

impl DiskGuard {
    pub fn new(ctx: &BanCtx) -> Self {
        let mut under = vec![(norm(&ctx.windows), "thư mục Windows")];
        under.extend(ctx.program_files.iter().filter(|p| !p.is_empty()).map(|p| (norm(p), "thư mục cài ứng dụng")));
        if !ctx.program_data.is_empty() {
            under.push((norm(&ctx.program_data), "dữ liệu hệ thống ProgramData"));
        }
        let app_data = if ctx.user_profile.is_empty() { String::new() } else { norm(&format!(r"{}\AppData", ctx.user_profile)) };
        Self { under, user_profile: norm(&ctx.user_profile), app_data }
    }

    /// Cảnh báo (không chặn) cho file dữ liệu ứng dụng: ổ đĩa ảo, database…
    pub fn warn(&self, path: &Path) -> Option<String> {
        let s = norm(&path.to_string_lossy());
        let in_app_data = !self.app_data.is_empty() && s.starts_with(&format!("{}\\", self.app_data));
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let disk_image = matches!(ext.as_str(), "vhdx" | "vhd" | "vmdk" | "vdi" | "img" | "qcow2");
        match (in_app_data, disk_image) {
            (_, true) => Some("ổ đĩa ảo của máy ảo/WSL/Docker — xoá sẽ mất toàn bộ dữ liệu bên trong".into()),
            (true, false) => Some("dữ liệu của ứng dụng (AppData) — ứng dụng có thể lỗi hoặc tạo lại".into()),
            _ => None,
        }
    }

    pub fn from_env() -> Self {
        Self::new(&BanCtx::from_env())
    }

    pub fn check(&self, path: &Path) -> Option<&'static str> {
        let mut normals = 0usize;
        for c in path.components() {
            match c {
                Component::Prefix(p) if !matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) => {
                    return Some("ổ mạng / đường dẫn đặc biệt")
                }
                Component::CurDir | Component::ParentDir => return Some("đường dẫn không hợp lệ"),
                Component::Normal(n) => {
                    let n = n.to_string_lossy().to_lowercase();
                    if n == "$recycle.bin" || n == "system volume information" {
                        return Some("thư mục hệ thống");
                    }
                    normals += 1;
                }
                _ => {}
            }
        }
        if !path.is_absolute() || normals <= 1 {
            return Some("file ở gốc ổ đĩa (pagefile, hiberfil…)");
        }
        let s = norm(&path.to_string_lossy());
        if crate::settings::is_protected(&s) {
            return Some("nằm trong danh sách bảo vệ của bạn");
        }
        if let Some((_, why)) = self.under.iter().find(|(b, _)| s == *b || s.strip_prefix(b.as_str()).is_some_and(|r| r.starts_with('\\'))) {
            return Some(why);
        }
        let parent = path.parent().map(|p| norm(&p.to_string_lossy())).unwrap_or_default();
        if !self.user_profile.is_empty() && parent == self.user_profile {
            return Some("file hồ sơ người dùng (ntuser.dat…)");
        }
        None
    }
}

// ---- Quét & lưu kết quả ---------------------------------------------------------------

#[derive(Debug, Clone)]
struct Stored {
    path: PathBuf,
    size: u64,
    modified_ms: Option<i64>,
    /// Nhóm trùng (chỉ với quét file trùng).
    group: Option<u32>,
    blocked: bool,
}

struct StoredScan {
    id: u64,
    at: Instant,
    files: HashMap<u32, Stored>,
}

pub struct DiskManager {
    cancel: Arc<AtomicBool>,
    running: Mutex<()>,
    last: Mutex<Option<StoredScan>>,
    next_id: AtomicU64,
}

impl Default for DiskManager {
    fn default() -> Self {
        Self { cancel: Arc::new(AtomicBool::new(false)), running: Mutex::new(()), last: Mutex::new(None), next_id: AtomicU64::new(1) }
    }
}

/// Thư mục quét: đường dẫn tuyệt đối tồn tại trên ổ cục bộ. (Chỉ đọc — không ảnh hưởng xoá.)
pub fn validate_root(root: &str) -> AppResult<PathBuf> {
    let p = PathBuf::from(root.trim());
    let local = matches!(p.components().next(), Some(Component::Prefix(x)) if matches!(x.kind(), Prefix::Disk(_)));
    if !local || !p.is_absolute() || !p.is_dir() {
        return Err(AppError::InvalidInput(format!("'{root}' không phải thư mục trên ổ đĩa cục bộ")));
    }
    Ok(p)
}

/// Gửi tiến độ tối đa ~8 lần/giây.
struct Throttle {
    last: Mutex<Instant>,
}

impl Throttle {
    fn new() -> Self {
        Self { last: Mutex::new(Instant::now()) }
    }
    fn ready(&self) -> bool {
        let Some(mut l) = self.last.try_lock_recover() else { return false };
        if l.elapsed() >= Duration::from_millis(125) {
            *l = Instant::now();
            true
        } else {
            false
        }
    }
}

impl DiskManager {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn begin(&self) -> AppResult<std::sync::MutexGuard<'_, ()>> {
        let g = self.running.try_lock_recover().ok_or_else(|| AppError::Busy("đang có một lần quét đĩa khác".into()))?;
        self.cancel.store(false, Ordering::Relaxed);
        Ok(g)
    }

    fn store(&self, files: HashMap<u32, Stored>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        *self.last.lock_recover() = Some(StoredScan { id, at: Instant::now(), files });
        id
    }

    /// `limit` file lớn nhất (≥ `min_size`) dưới `root`.
    pub fn scan_large(&self, root: &Path, min_size: u64, on_progress: &dyn Fn(DiskProgress)) -> AppResult<LargeScan> {
        let _g = self.begin()?;
        let started = Instant::now();
        let throttle = Throttle::new();
        let (mut n, mut bytes) = (0u64, 0u64);
        // Min-heap giữ top N theo kích thước.
        let mut heap: BinaryHeap<Reverse<(u64, PathBuf, Option<i64>)>> = BinaryHeap::new();
        walk::walk(root, &self.cancel, |f| {
            n += 1;
            bytes += f.size;
            if f.size >= min_size {
                heap.push(Reverse((f.size, f.path, f.modified_ms)));
                if heap.len() > LARGE_LIMIT {
                    heap.pop();
                }
            }
            if throttle.ready() {
                on_progress(DiskProgress { phase: "walk", files: n, bytes, done: 0, total: 0 });
            }
        })?;
        let guard = DiskGuard::from_env();
        let mut list: Vec<(u64, PathBuf, Option<i64>)> = heap.into_iter().map(|Reverse(x)| x).collect();
        list.sort_by_key(|f| Reverse(f.0));
        let mut stored = HashMap::new();
        let files = list
            .into_iter()
            .enumerate()
            .map(|(i, (size, path, modified_ms))| {
                let blocked = guard.check(&path).map(str::to_owned);
                stored.insert(i as u32, Stored { path: path.clone(), size, modified_ms, group: None, blocked: blocked.is_some() });
                let warning = guard.warn(&path);
                FileItem { id: i as u32, path: path.display().to_string(), size, modified_ms, blocked, warning }
            })
            .collect();
        let cancelled = self.cancel.load(Ordering::Relaxed);
        let scan_id = self.store(stored);
        Ok(LargeScan {
            scan_id,
            root: root.display().to_string(),
            files,
            scanned_files: n,
            scanned_bytes: bytes,
            duration_ms: started.elapsed().as_millis() as u64,
            cancelled,
        })
    }

    /// Nhóm file trùng nội dung (≥ `min_size`) dưới `root`.
    pub fn scan_duplicates(&self, root: &Path, min_size: u64, on_progress: &(dyn Fn(DiskProgress) + Sync)) -> AppResult<DupScan> {
        let _g = self.begin()?;
        let started = Instant::now();
        let throttle = Throttle::new();
        let cancel = &*self.cancel;
        let mut all: Vec<FileEntry> = Vec::new();
        let (mut n, mut bytes) = (0u64, 0u64);
        walk::walk(root, cancel, |f| {
            n += 1;
            bytes += f.size;
            if f.size >= min_size.max(1) {
                all.push(f);
            }
            if throttle.ready() {
                on_progress(DiskProgress { phase: "walk", files: n, bytes, done: 0, total: 0 });
            }
        })?;

        let pool = crate::bg::pool()?;
        let groups = pool.install(|| {
            let by_size = dup::by_size(all, min_size);
            let stage = |phase: &'static str, groups: Vec<Vec<FileEntry>>, full: bool| -> Vec<Vec<FileEntry>> {
                let total: u64 = groups.iter().map(|g| g.len() as u64).sum();
                let done = AtomicU64::new(0);
                groups
                    .into_par_iter()
                    .flat_map_iter(|g| {
                        if cancel.load(Ordering::Relaxed) {
                            return Vec::new();
                        }
                        dup::split_by(g, |f| {
                            let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                            if throttle.ready() {
                                on_progress(DiskProgress { phase, files: n, bytes, done: d, total });
                            }
                            if full { dup::hash_full(&f.path, cancel) } else { dup::hash_head(&f.path) }
                        })
                    })
                    .collect()
            };
            let head = stage("head", by_size, false);
            // File ≤ 4KB: hash đầu đã là hash toàn bộ.
            let (small, big): (Vec<_>, Vec<_>) = head.into_iter().partition(|g| g[0].size <= dup::HEAD_BYTES as u64);
            let mut out = small;
            out.extend(stage("full", big, true));
            out
        });

        // Hard link: nhiều đường dẫn cùng một file — không phải bản sao, bỏ đi.
        let mut groups: Vec<Vec<FileEntry>> = groups
            .into_iter()
            .filter_map(|g| {
                let mut seen = std::collections::HashSet::new();
                let g: Vec<FileEntry> = g.into_iter().filter(|f| sys::disk::file_id(&f.path).is_none_or(|id| seen.insert(id))).collect();
                (g.len() > 1).then_some(g)
            })
            .collect();
        groups.sort_by_key(|g| Reverse(g[0].size * (g.len() as u64 - 1)));
        groups.truncate(DUP_GROUP_LIMIT);
        let guard = DiskGuard::from_env();
        let mut stored = HashMap::new();
        let mut next = 0u32;
        let mut wasted = 0u64;
        let out: Vec<DupGroup> = groups
            .into_iter()
            .enumerate()
            .map(|(gi, mut g)| {
                g.sort_by(|a, b| a.path.cmp(&b.path));
                wasted += g[0].size * (g.len() as u64 - 1);
                let files = g
                    .into_iter()
                    .map(|f| {
                        let id = next;
                        next += 1;
                        let blocked = guard.check(&f.path).map(str::to_owned);
                        stored.insert(id, Stored { path: f.path.clone(), size: f.size, modified_ms: f.modified_ms, group: Some(gi as u32), blocked: blocked.is_some() });
                        let warning = guard.warn(&f.path);
                        FileItem { id, path: f.path.display().to_string(), size: f.size, modified_ms: f.modified_ms, blocked, warning }
                    })
                    .collect::<Vec<_>>();
                DupGroup { size: files[0].size, files }
            })
            .collect();
        let cancelled = self.cancel.load(Ordering::Relaxed);
        let scan_id = self.store(stored);
        Ok(DupScan {
            scan_id,
            root: root.display().to_string(),
            groups: out,
            wasted_bytes: wasted,
            scanned_files: n,
            duration_ms: started.elapsed().as_millis() as u64,
            cancelled,
        })
    }

    /// Chuyển các file đã chọn vào Thùng rác. Với nhóm trùng: luôn giữ lại ít nhất 1 bản.
    pub fn recycle(&self, store: &Store, scan_id: u64, ids: &[u32]) -> AppResult<RecycleReport> {
        let _g = self.begin()?;
        let scan = {
            let mut last = self.last.lock_recover();
            match last.take() {
                Some(s) if s.id == scan_id && s.at.elapsed() < SCAN_TTL => s,
                other => {
                    *last = other;
                    return Err(AppError::InvalidInput("kết quả quét đã cũ — hãy quét lại".into()));
                }
            }
        };
        let guard = DiskGuard::from_env();
        let selected: Vec<(u32, &Stored)> = ids.iter().filter_map(|id| scan.files.get(id).map(|s| (*id, s))).collect();
        let refused = keep_one_per_group(&scan.files, ids);
        let mut report = RecycleReport { recycled: 0, recycled_bytes: 0, failed: 0, items: Vec::new() };
        for (id, f) in selected {
            let result = if refused.contains(&id) {
                Err("phải giữ lại ít nhất một bản trong nhóm".to_owned())
            } else {
                recycle_one(&guard, f, &scan.files, id)
            };
            let ok = result.is_ok();
            store.log_action_best_effort("disk.recycle", &f.path.display().to_string(), ok, result.as_ref().err().map_or("", String::as_str));
            if ok {
                report.recycled += 1;
                report.recycled_bytes += f.size;
            } else {
                report.failed += 1;
            }
            report.items.push(RecycleOutcome { id, ok, message: result.err() });
        }
        // Giữ kết quả quét (bỏ file đã dọn) để người dùng tiếp tục chọn.
        let mut files = scan.files;
        for o in report.items.iter().filter(|o| o.ok) {
            files.remove(&o.id);
        }
        *self.last.lock_recover() = Some(StoredScan { id: scan.id, at: scan.at, files });
        Ok(report)
    }
}

/// Id bị từ chối vì người dùng chọn TẤT CẢ bản trong một nhóm trùng (giữ bản đầu tiên).
fn keep_one_per_group(files: &HashMap<u32, Stored>, ids: &[u32]) -> Vec<u32> {
    let mut per_group: HashMap<u32, (usize, Vec<u32>)> = HashMap::new();
    for (id, f) in files {
        if let Some(g) = f.group {
            let e = per_group.entry(g).or_default();
            e.0 += 1;
            if ids.contains(id) {
                e.1.push(*id);
            }
        }
    }
    let mut refused = Vec::new();
    for (_, (total, mut chosen)) in per_group {
        if chosen.len() >= total {
            chosen.sort_unstable();
            refused.push(chosen[0]);
        }
    }
    refused
}

fn mtime_ms(m: &std::fs::Metadata) -> Option<i64> {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64)
}

fn recycle_one(guard: &DiskGuard, f: &Stored, all: &HashMap<u32, Stored>, id: u32) -> Result<(), String> {
    if f.blocked {
        return Err("file này không được phép xoá".into());
    }
    if let Some(why) = guard.check(&f.path) {
        return Err(format!("bị chặn: {why}"));
    }
    let meta = std::fs::symlink_metadata(&f.path).map_err(|_| "file không còn tồn tại".to_owned())?;
    if is_link_or_cloud(&meta) || !meta.is_file() {
        return Err("không phải file thường".into());
    }
    if meta.file_size() != f.size || mtime_ms(&meta) != f.modified_ms {
        return Err("file đã thay đổi từ lúc quét — hãy quét lại".into());
    }
    // File trùng: bản giữ lại phải vẫn còn nguyên.
    if let Some(g) = f.group {
        let kept = all.iter().any(|(oid, o)| {
            *oid != id && o.group == Some(g) && std::fs::symlink_metadata(&o.path).is_ok_and(|m| m.is_file() && m.file_size() == f.size)
        });
        if !kept {
            return Err("không còn bản nào khác trong nhóm — giữ lại file này".into());
        }
    }
    crate::user::recycle::move_to_recycle_bin(&f.path)?;
    if f.path.exists() {
        return Err("chưa chuyển được (file đang mở?)".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> DiskGuard {
        DiskGuard::new(&BanCtx {
            windows: r"C:\Windows".into(),
            user_profile: r"C:\Users\An".into(),
            users_root: r"C:\Users".into(),
            program_data: r"C:\ProgramData".into(),
            program_files: vec![r"C:\Program Files".into(), r"C:\Program Files (x86)".into()],
            user_folders: vec![],
        })
    }

    #[test]
    fn guard_allows_user_data_blocks_system() {
        let g = guard();
        for ok in [r"C:\Users\An\Downloads\iso\win.iso", r"C:\Users\An\Documents\video.mp4", r"D:\Backup\old.zip", r"C:\Users\An\AppData\Local\Temp\big.tmp"] {
            assert_eq!(g.check(Path::new(ok)), None, "{ok}");
        }
        assert!(g.warn(Path::new(r"C:\Users\An\AppData\Local\Docker\wsl\disk.vhdx")).is_some());
        assert!(g.warn(Path::new(r"C:\Users\An\AppData\Local\x\cache.bin")).is_some());
        assert!(g.warn(Path::new(r"C:\Users\An\Downloads\a.iso")).is_none());
        for bad in [
            r"C:\pagefile.sys",
            r"C:\hiberfil.sys",
            r"C:\Windows\System32\x.dll",
            r"C:\Windows\Installer\abc.msi",
            r"C:\Program Files\App\app.exe",
            r"C:\ProgramData\x\y.dat",
            r"C:\Users\An\NTUSER.DAT",
            r"D:\$RECYCLE.BIN\S-1\x",
            r"D:\System Volume Information\x",
            r"\\nas\share\movie.mkv",
        ] {
            assert!(g.check(Path::new(bad)).is_some(), "{bad}");
        }
    }

    fn st(group: Option<u32>) -> Stored {
        Stored { path: PathBuf::new(), size: 1, modified_ms: None, group, blocked: false }
    }

    #[test]
    fn must_keep_one_copy_per_group() {
        let files = HashMap::from([(0, st(Some(0))), (1, st(Some(0))), (2, st(Some(1))), (3, st(Some(1))), (4, st(Some(1))), (5, st(None))]);
        assert!(keep_one_per_group(&files, &[0, 2, 3, 5]).is_empty());
        assert_eq!(keep_one_per_group(&files, &[0, 1]), vec![0]);
        let mut r = keep_one_per_group(&files, &[0, 1, 2, 3, 4]);
        r.sort_unstable();
        assert_eq!(r, vec![0, 2]);
    }

    #[test]
    fn scan_large_and_duplicates_in_temp() {
        let dir = std::env::temp_dir().join(format!("pcm-disk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("big.bin"), vec![1u8; 300_000]).unwrap();
        std::fs::write(dir.join("sub").join("big-copy.bin"), vec![1u8; 300_000]).unwrap();
        std::fs::write(dir.join("small.txt"), b"hi").unwrap();
        let m = DiskManager::default();
        let large = m.scan_large(&dir, 100_000, &|_| {}).unwrap();
        assert_eq!(large.files.len(), 2);
        assert_eq!(large.scanned_files, 3);
        let dups = m.scan_duplicates(&dir, 1, &|_| {}).unwrap();
        assert_eq!(dups.groups.len(), 1);
        assert_eq!(dups.wasted_bytes, 300_000);
        // Chọn cả 2 bản => bản đầu bị từ chối, không có gì bị xoá (không gọi Thùng rác thật).
        let store = Store::in_memory().unwrap();
        let ids: Vec<u32> = dups.groups[0].files.iter().map(|f| f.id).collect();
        let first = ids[0];
        std::fs::write(dir.join("sub").join("big-copy.bin"), vec![2u8; 300_001]).unwrap(); // đổi size => bản còn lại cũng bị từ chối
        let r = m.recycle(&store, dups.scan_id, &ids).unwrap();
        assert_eq!(r.recycled, 0);
        assert!(r.items.iter().any(|o| o.id == first && o.message.as_deref() == Some("phải giữ lại ít nhất một bản trong nhóm")));
        assert!(dir.join("big.bin").exists() && dir.join("sub").join("big-copy.bin").exists());
        assert!(m.recycle(&store, 999, &[0]).is_err(), "scan id sai");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod real {
    use super::*;

    /// Chẩn đoán: `cargo test --release --lib print_disk -- --ignored --nocapture` (PCM_ROOT để đổi thư mục).
    #[test]
    #[ignore = "chỉ để xem dữ liệu thật"]
    fn print_disk() {
        let root = std::env::var("PCM_ROOT").unwrap_or_else(|_| std::env::var("USERPROFILE").unwrap());
        let m = DiskManager::default();
        let l = m.scan_large(Path::new(&root), 100 << 20, &|_| {}).unwrap();
        println!("large: {} file / {} MB trong {} ms", l.scanned_files, l.scanned_bytes >> 20, l.duration_ms);
        for f in l.files.iter().take(5) {
            println!("  {:>8} MB {} {:?}", f.size >> 20, f.path, f.blocked);
        }
        let d = m.scan_duplicates(Path::new(&root), 1 << 20, &|_| {}).unwrap();
        println!("dup: {} nhóm, lãng phí {} MB trong {} ms", d.groups.len(), d.wasted_bytes >> 20, d.duration_ms);
        for g in d.groups.iter().take(3) {
            println!("  {} MB x{}: {}", g.size >> 20, g.files.len(), g.files[0].path);
        }
    }
}
