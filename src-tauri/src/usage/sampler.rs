//! Lấy mẫu tiến trình đang chạy mỗi 60s và ghi vào SQLite (nguồn thứ 3 cho
//! "lần dùng cuối"). Chỉ mở handle cho tiến trình MỚI để giữ chi phí thấp.
//! Ghi ngay exe MỚI xuất hiện; exe vẫn đang chạy chỉ làm mới `last_seen` tối đa 15 phút/lần
//! ("lần dùng cuối" chỉ cần chính xác theo ngày) — tránh 150–300 UPSERT mỗi phút.

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::store::Store;
use crate::sys::process;

/// Exe vẫn đang chạy: làm mới `last_seen` tối đa một lần mỗi khoảng này.
const REFRESH: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub struct Sampler {
    /// (pid, create_time) → đường dẫn exe (None nếu không đọc được).
    paths: HashMap<(u32, u64), Option<String>>,
    /// exe (chữ thường) → lần ghi gần nhất (ms). Chỉ giữ exe đang chạy: thoát rồi chạy lại = exe mới.
    written: HashMap<String, i64>,
}

impl Sampler {
    /// Một lần lấy mẫu. Trả về số exe đã ghi.
    pub fn tick(&mut self, store: &Store) -> usize {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        self.tick_at(store, now)
    }

    /// exe cần ghi trong `running` (đã sắp xếp, không trùng): exe mới, hoặc đã ghi quá `REFRESH`.
    fn take_due(&mut self, running: &[String], now: i64) -> Vec<String> {
        let refresh_ms = REFRESH.as_millis() as i64;
        let due = running
            .iter()
            .filter(|e| self.written.get(*e).is_none_or(|&at| now - at >= refresh_ms))
            .cloned()
            .collect();
        // Quên exe đã thoát => lần sau xuất hiện lại được ghi ngay.
        self.written.retain(|e, _| running.binary_search(e).is_ok());
        due
    }

    fn mark(&mut self, written: &[String], now: i64) {
        for e in written {
            self.written.insert(e.clone(), now);
        }
    }

    fn tick_at(&mut self, store: &Store, now: i64) -> usize {
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
        let mut running: Vec<String> = self.paths.values().flatten().map(|p| p.to_lowercase()).collect();
        running.sort();
        running.dedup();
        let due = self.take_due(&running, now);
        if due.is_empty() {
            return 0;
        }
        match store.record_usage(&due, now) {
            Ok(()) => {
                self.mark(&due, now);
                due.len()
            }
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
    }

    #[test]
    fn running_exes_are_refreshed_at_most_every_15_minutes() {
        let v = |xs: &[&str]| xs.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        let mut s = Sampler::default();
        let t0 = 1_000_000_000_000;
        let min = 60_000;
        let due = s.take_due(&v(&["a", "b"]), t0);
        assert_eq!(due, v(&["a", "b"]), "lần đầu: ghi tất cả");
        s.mark(&due, t0);
        assert!(s.take_due(&v(&["a", "b"]), t0 + min).is_empty(), "không exe mới => không ghi dòng nào");
        let due = s.take_due(&v(&["a", "b", "c"]), t0 + 2 * min);
        assert_eq!(due, v(&["c"]), "chỉ exe mới");
        s.mark(&due, t0 + 2 * min);
        // 16 phút sau: a đã quá 15 phút => làm mới; c mới ghi 14 phút trước => chưa; b đã thoát => quên.
        let due = s.take_due(&v(&["a", "c"]), t0 + 16 * min);
        assert_eq!(due, v(&["a"]));
        s.mark(&due, t0 + 16 * min);
        assert_eq!(s.take_due(&v(&["a", "b", "c"]), t0 + 16 * min + 30_000), v(&["b"]), "b chạy lại = exe mới");
    }
}
