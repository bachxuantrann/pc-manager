//! Tìm file trùng: nhóm theo size → blake3 4KB đầu → blake3 toàn bộ.
//! Mỗi bước chỉ giữ nhóm còn ≥ 2 file nên phần lớn file không bao giờ bị đọc.

use std::collections::HashMap;
use std::hash::Hash;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::walk::FileEntry;

pub const HEAD_BYTES: usize = 4096;
const CHUNK: usize = 256 * 1024;

/// Nhóm các file cùng kích thước (≥ `min_size`, > 0); bỏ nhóm chỉ có 1 file.
pub fn by_size(files: Vec<FileEntry>, min_size: u64) -> Vec<Vec<FileEntry>> {
    let mut map: HashMap<u64, Vec<FileEntry>> = HashMap::new();
    for f in files.into_iter().filter(|f| f.size > 0 && f.size >= min_size) {
        map.entry(f.size).or_default().push(f);
    }
    map.into_values().filter(|g| g.len() > 1).collect()
}

/// Tách nhóm theo khoá; file không tính được khoá (lỗi đọc, bị khoá) bị loại.
pub fn split_by<K: Eq + Hash>(group: Vec<FileEntry>, key: impl Fn(&FileEntry) -> Option<K>) -> Vec<Vec<FileEntry>> {
    let mut map: HashMap<K, Vec<FileEntry>> = HashMap::new();
    for f in group {
        if let Some(k) = key(&f) {
            map.entry(k).or_default().push(f);
        }
    }
    map.into_values().filter(|g| g.len() > 1).collect()
}

/// blake3 của tối đa `HEAD_BYTES` byte đầu.
pub fn hash_head(path: &Path) -> Option<[u8; 32]> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; HEAD_BYTES];
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(_) => return None,
        }
    }
    Some(*blake3::hash(&buf[..n]).as_bytes())
}

/// blake3 toàn bộ file; None khi lỗi hoặc bị huỷ.
pub fn hash_full(path: &Path, cancel: &AtomicBool) -> Option<[u8; 32]> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                h.update(&buf[..n]);
            }
            Err(_) => return None,
        }
    }
    Some(*h.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fe(p: &Path) -> FileEntry {
        FileEntry { path: p.to_path_buf(), size: std::fs::metadata(p).unwrap().len(), modified_ms: None }
    }

    #[test]
    fn three_stage_grouping_finds_only_true_duplicates() {
        let dir: PathBuf = std::env::temp_dir().join(format!("pcm-dup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let big = vec![7u8; 10_000];
        let mut same_head = big.clone();
        same_head[9_999] = 8; // giống 4KB đầu, khác ở cuối
        let w = |n: &str, d: &[u8]| {
            let p = dir.join(n);
            std::fs::write(&p, d).unwrap();
            fe(&p)
        };
        let files = vec![
            w("a.bin", &big),
            w("b.bin", &big),
            w("c.bin", &same_head),
            w("d.bin", &[1u8; 5000]), // size duy nhất
            w("e.txt", b"xy"),        // < min_size
            w("f.txt", b"xy"),
        ];
        let cancel = AtomicBool::new(false);
        let groups: Vec<Vec<FileEntry>> = by_size(files, 100)
            .into_iter()
            .flat_map(|g| split_by(g, |f| hash_head(&f.path)))
            .flat_map(|g| split_by(g, |f| hash_full(&f.path, &cancel)))
            .collect();
        assert_eq!(groups.len(), 1);
        let mut names: Vec<String> = groups[0].iter().map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["a.bin", "b.bin"]);
        assert!(hash_full(&dir.join("a.bin"), &AtomicBool::new(true)).is_none(), "huỷ => None");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
