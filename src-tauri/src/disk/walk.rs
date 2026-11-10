//! Duyệt cây thư mục song song bằng jwalk trên pool nền (ưu tiên thấp).
//!
//! Metadata được đọc ngay trong `process_read_dir` (chạy song song trên pool)
//! để luồng gọi chỉ việc gom kết quả. Không đi vào symlink/junction/file đám mây.

use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use jwalk::{Parallelism, WalkDirGeneric};

use crate::cleaner::safety::is_link_or_cloud;
use crate::error::AppResult;

/// File tìm thấy khi duyệt.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub modified_ms: Option<i64>,
}

/// Thư mục hệ thống không bao giờ đi vào (không có quyền hoặc không nên đụng).
fn skip_dir(name: &str) -> bool {
    const SKIP: &[&str] = &["system volume information", "$recycle.bin", "$windows.~bt", "$windows.~ws", "$winreagent", "config.msi"];
    SKIP.contains(&name.to_lowercase().as_str())
}

type Meta = Option<(u64, Option<i64>)>;

/// Gọi `on_file` cho mọi file thường dưới `root`. Dừng sớm khi `cancel` bật.
pub fn walk(root: &Path, cancel: &AtomicBool, mut on_file: impl FnMut(FileEntry)) -> AppResult<()> {
    let pool = crate::bg::shared()?.clone();
    let walker = WalkDirGeneric::<((), Meta)>::new(root)
        .skip_hidden(false)
        .follow_links(false)
        .sort(false)
        // busy_timeout None: luồng gọi KHÔNG nằm trong pool nên không thể deadlock.
        .parallelism(Parallelism::RayonExistingPool { pool, busy_timeout: None })
        .process_read_dir(|_, _, _, children| {
            children.retain_mut(|r| {
                let Ok(e) = r else { return false };
                let name = e.file_name().to_string_lossy();
                if e.file_type.is_dir() && skip_dir(&name) {
                    return false;
                }
                let Ok(m) = e.metadata() else { return false };
                if is_link_or_cloud(&m) {
                    e.read_children_path = None;
                    return false;
                }
                if m.is_file() {
                    let mtime = m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64);
                    e.client_state = Some((m.file_size(), mtime));
                }
                true
            });
        });
    for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(e) = entry else { continue };
        if let Some((size, modified_ms)) = e.client_state {
            on_file(FileEntry { path: e.path(), size, modified_ms });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_files_and_skips_junctions() {
        let root = std::env::temp_dir().join(format!("pcm-walk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/x.bin"), vec![0u8; 10]).unwrap();
        std::fs::write(root.join("y.bin"), vec![0u8; 3]).unwrap();
        // Junction trỏ ra thư mục khác — không được đi vào.
        let target = std::env::temp_dir().join(format!("pcm-walk-target-{}", std::process::id()));
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("outside.bin"), b"zz").unwrap();
        let ok = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(root.join("jn"))
            .arg(&target)
            .output()
            .is_ok_and(|o| o.status.success());
        let mut seen = Vec::new();
        walk(&root, &AtomicBool::new(false), |f| seen.push((f.path.file_name().unwrap().to_string_lossy().into_owned(), f.size))).unwrap();
        seen.sort();
        assert_eq!(seen, vec![("x.bin".to_owned(), 10), ("y.bin".to_owned(), 3)], "junction tạo được: {ok}");
        let _ = std::fs::remove_dir(root.join("jn"));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&target);
    }

    #[test]
    fn cancel_stops_early() {
        let mut n = 0;
        walk(&std::env::temp_dir(), &AtomicBool::new(true), |_| n += 1).unwrap();
        assert_eq!(n, 0);
    }
}
