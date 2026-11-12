//! Tự cập nhật (giai đoạn 7): cài đặt và lịch kiểm tra. Phần mạng/cài đặt nằm ở `commands::update`
//! (plugin `tauri-plugin-updater`: đọc `latest.json` của GitHub Release, kiểm chữ ký minisign rồi chạy bộ cài NSIS).
//!
//! - Chỉ kiểm tra khi cửa sổ đang mở (UI gọi), tối đa 1 lần/ngày khi tự động; không có luồng nền thường trực.
//! - Không bao giờ tự cài: người dùng bấm "Cập nhật", Windows hỏi UAC (bộ cài cài vào Program Files).

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::store::Store;

const KEY: &str = "update";
/// Tự kiểm tra tối đa 1 lần mỗi khoảng này.
pub const AUTO_INTERVAL_MS: i64 = 24 * 3600 * 1000;

/// Cài đặt cập nhật (lưu SQLite). Thiếu trường => mặc định (dữ liệu cũ vẫn đọc được).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateSettings {
    /// Tự kiểm tra khi mở cửa sổ (mặc định bật).
    pub auto: bool,
    pub last_check_ms: Option<i64>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self { auto: true, last_check_ms: None }
    }
}

impl UpdateSettings {
    pub fn load(store: &Store) -> Self {
        store
            .get_setting(KEY)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, store: &Store) -> AppResult<()> {
        let json = serde_json::to_string(self).map_err(|e| crate::error::AppError::Other(e.to_string()))?;
        store.set_setting(KEY, &json)
    }

    /// Có nên tự kiểm tra lúc `now_ms` không (bấm tay thì luôn kiểm tra).
    pub fn auto_due(&self, now_ms: i64) -> bool {
        self.auto && self.last_check_ms.is_none_or(|t| now_ms.saturating_sub(t) >= AUTO_INTERVAL_MS || t > now_ms)
    }
}

/// Khoá công khai trong `tauri.conf.json` còn là chỗ trống => bản build chưa cấu hình ký cập nhật,
/// không được cài gì (plugin cũng sẽ từ chối, nhưng báo sớm và rõ hơn).
pub fn pubkey_configured(pubkey: &str) -> bool {
    let k = pubkey.trim();
    !k.is_empty() && !k.contains("CHUA_CO_KHOA")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_check_at_most_once_a_day() {
        let day = AUTO_INTERVAL_MS;
        let s = UpdateSettings { auto: true, last_check_ms: None };
        assert!(s.auto_due(1_000), "chưa kiểm tra lần nào");
        let s = UpdateSettings { auto: true, last_check_ms: Some(10 * day) };
        assert!(!s.auto_due(10 * day + day - 1));
        assert!(s.auto_due(11 * day));
        assert!(s.auto_due(day), "đồng hồ máy bị lùi => kiểm tra lại");
        let off = UpdateSettings { auto: false, last_check_ms: None };
        assert!(!off.auto_due(1_000));
    }

    #[test]
    fn settings_roundtrip_and_defaults() {
        let store = Store::in_memory().unwrap();
        assert_eq!(UpdateSettings::load(&store), UpdateSettings::default());
        let s = UpdateSettings { auto: false, last_check_ms: Some(42) };
        s.save(&store).unwrap();
        assert_eq!(UpdateSettings::load(&store), s);
        store.set_setting(KEY, "{}").unwrap();
        assert!(UpdateSettings::load(&store).auto, "thiếu trường => mặc định bật");
    }

    #[test]
    fn placeholder_pubkey_is_not_configured() {
        assert!(!pubkey_configured(""));
        assert!(!pubkey_configured("CHUA_CO_KHOA — chạy scripts/new-updater-key.ps1"));
        assert!(pubkey_configured("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk="));
    }
}
