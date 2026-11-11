//! Cache kết quả kiểm tra chữ ký theo đường dẫn. WinVerifyTrust chậm (vài
//! chục ms/file, catalog còn lâu hơn) nên chạy trên 1 thread nền ưu tiên thấp;
//! UI thấy `pending` rồi tự cập nhật ở lần refresh sau.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::sys::process::enter_background_mode;
use crate::sys::signature::{self, Signature};
use crate::sync::LockExt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Publisher {
    /// Không đọc được đường dẫn (tiến trình được bảo vệ / đã thoát).
    Unknown,
    Pending,
    Signed { name: String, catalog: bool },
    Unsigned,
    Invalid { code: String },
}

impl From<Signature> for Publisher {
    fn from(s: Signature) -> Self {
        match s {
            Signature::Signed { name, catalog } => Publisher::Signed { name, catalog },
            Signature::Unsigned => Publisher::Unsigned,
            Signature::Invalid { code } => Publisher::Invalid { code },
        }
    }
}

type Map = Arc<Mutex<HashMap<PathBuf, Option<Signature>>>>;

pub struct PublisherCache {
    /// `None` = đang chờ kiểm tra.
    map: Map,
    tx: Mutex<Option<Sender<PathBuf>>>,
}

impl Default for PublisherCache {
    fn default() -> Self {
        Self { map: Arc::new(Mutex::new(HashMap::new())), tx: Mutex::new(None) }
    }
}

impl PublisherCache {
    /// Kết quả đã có, hoặc `Pending` và xếp hàng kiểm tra.
    pub fn get(&self, path: &Path) -> Publisher {
        let mut map = self.map.lock_recover();
        match map.get(path) {
            Some(Some(sig)) => return sig.clone().into(),
            Some(None) => return Publisher::Pending,
            None => {}
        }
        map.insert(path.to_path_buf(), None);
        drop(map);
        self.enqueue(path.to_path_buf());
        Publisher::Pending
    }

    fn enqueue(&self, path: PathBuf) {
        let mut tx = self.tx.lock_recover();
        if tx.is_none() {
            *tx = self.spawn_worker();
        }
        if let Some(sender) = tx.as_ref() {
            if sender.send(path).is_err() {
                *tx = None; // worker đã chết — lần sau tạo lại.
            }
        }
    }

    fn spawn_worker(&self) -> Option<Sender<PathBuf>> {
        let (tx, rx) = channel::<PathBuf>();
        let map = Arc::clone(&self.map);
        std::thread::Builder::new()
            .name("publisher-verify".into())
            .spawn(move || {
                enter_background_mode();
                for path in rx {
                    // Một file lỗi khi kiểm tra chữ ký không làm chết cả luồng kiểm tra.
                    let Some(sig) = crate::sync::survive("publisher-verify", || signature::verify(&path)) else { continue };
                    map.lock_recover().insert(path, Some(sig));
                }
            })
            .ok()?;
        Some(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn first_call_is_pending_then_resolves_in_background() {
        let cache = PublisherCache::default();
        let p = Path::new(r"C:\Windows\explorer.exe");
        assert_eq!(cache.get(p), Publisher::Pending);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match cache.get(p) {
                Publisher::Pending if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
                Publisher::Signed { name, .. } => {
                    assert!(name.contains("Microsoft"));
                    break;
                }
                other => panic!("kết quả bất ngờ: {other:?}"),
            }
        }
    }
}
