//! Lấy mẫu tiến trình đang chạy mỗi 60s và ghi vào SQLite (nguồn thứ 3 cho
//! "lần dùng cuối"). Chỉ mở handle cho tiến trình MỚI để giữ chi phí thấp.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::store::Store;
use crate::sys::process;

#[derive(Default)]
pub struct Sampler {
    /// (pid, create_time) → đường dẫn exe (None nếu không đọc được).
    paths: HashMap<(u32, u64), Option<String>>,
}

impl Sampler {
    /// Một lần lấy mẫu. Trả về số exe đã ghi.
    pub fn tick(&mut self, store: &Store) -> usize {
        let Ok(list) = process::enumerate() else { return 0 };
        let mut next = HashMap::with_capacity(list.len());
        for p in &list {
            if p.pid <= 4 {
                continue;
            }
            let key = (p.pid, p.create_time);
            let path = self
                .paths
                .remove(&key)
                .unwrap_or_else(|| process::image_path(p.pid).map(|x| x.display().to_string()));
            next.insert(key, path);
        }
        self.paths = next;
        let mut exes: Vec<String> = self.paths.values().flatten().map(|p| p.to_lowercase()).collect();
        exes.sort();
        exes.dedup();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        match store.record_usage(&exes, now) {
            Ok(()) => exes.len(),
            Err(e) => {
                eprintln!("[usage] không ghi được mẫu: {e}");
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_records_running_exes_including_self() {
        let store = Store::in_memory().unwrap();
        let mut s = Sampler::default();
        assert!(s.tick(&store) > 0);
        let me = std::env::current_exe().unwrap().display().to_string().to_lowercase();
        assert!(store.usage().unwrap().iter().any(|(p, _)| *p == me));
        // Lần 2 dùng lại cache đường dẫn, không lỗi.
        assert!(s.tick(&store) > 0);
    }
}
