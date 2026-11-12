//! Tính toán thuần (không gọi OS) — dễ test.

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::sys::memory::{BasicMemory, MemoryLists};

/// Ảnh chụp bộ nhớ gửi lên UI. Mọi giá trị tính bằng byte.
///
/// `free`/`standby`/`modified` là `None` khi không có quyền admin
/// (cần `SeProfileSingleProcessPrivilege` để đọc memory list).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatus {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub free: Option<u64>,
    pub standby: Option<u64>,
    pub modified: Option<u64>,
    pub commit_total: u64,
    pub commit_limit: u64,
    pub system_cache: u64,
    pub load_pct: u32,
}

pub fn status_from(basic: BasicMemory, lists: Option<MemoryLists>) -> MemoryStatus {
    let page = basic.page_size;
    let bytes = |pages: u64| pages.saturating_mul(page);
    // Dữ liệu từ OS: không tin là luôn hợp lệ, kẹp lại cho chắc.
    let total = basic.total_phys;
    let available = basic.avail_phys.min(total);
    MemoryStatus {
        total,
        used: total - available,
        available,
        free: lists.map(|l| bytes(l.free_pages.saturating_add(l.zero_pages))),
        standby: lists.map(|l| bytes(l.standby_pages)),
        modified: lists.map(|l| bytes(l.modified_pages)),
        commit_total: basic.commit_total,
        commit_limit: basic.commit_limit,
        system_cache: basic.system_cache,
        load_pct: basic.load_pct.min(100),
    }
}

/// Kết quả Boost, tách theo ý nghĩa thật để báo cáo trung thực.
///
/// Standby là cache mà Windows đã tính vào *available* và nhường ngay khi app
/// cần. Purge standby chỉ chuyển trang sang *free* — RAM app dùng được KHÔNG
/// tăng, đổi lại mất cache (lần mở file/app sau phải đọc lại từ đĩa).
/// Lợi ích thật là `available_gain`, chủ yếu đến từ flush modified list.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BoostGain {
    /// RAM khả dụng tăng thêm — con số chính hiển thị cho người dùng.
    pub available_gain: u64,
    /// Cache standby đã bị xoá (None khi không đọc được memory list).
    pub standby_cleared: Option<u64>,
    /// Trang modified đã được ghi ra đĩa.
    pub modified_flushed: Option<u64>,
    /// Free list tăng (≈ standby_cleared + phần available_gain).
    pub free_gain: Option<u64>,
}

/// Tính kết quả từ 3 ảnh chụp: trước Boost, giữa (sau flush, trước purge), sau.
///
/// Phải đo từng bước riêng: flush chuyển trang modified *vào* standby, nên
/// nếu lấy `standby trước − standby sau` cho cả quá trình thì phần purge bị
/// phần flush bù mất (thực tế đo được: purge ~520 MB nhưng hiện 0 MB).
pub fn boost_gain(before: &MemoryStatus, mid: &MemoryStatus, after: &MemoryStatus) -> BoostGain {
    let drop = |b: Option<u64>, a: Option<u64>| Some(b?.saturating_sub(a?));
    BoostGain {
        available_gain: after.available.saturating_sub(before.available),
        standby_cleared: drop(mid.standby, after.standby),
        modified_flushed: drop(before.modified, mid.modified),
        free_gain: drop(after.free, before.free),
    }
}

/// Phần trăm RAM còn trống (available / total), 0..=100.
pub fn available_pct(s: &MemoryStatus) -> f64 {
    if s.total == 0 {
        return 0.0;
    }
    s.available as f64 * 100.0 / s.total as f64
}

/// Smart Boost chỉ chạy khi available < ngưỡng % (CLAUDE.md, mặc định 15%).
pub fn should_smart_boost(s: &MemoryStatus, threshold: SmartThreshold) -> bool {
    if s.total == 0 {
        return false;
    }
    // So sánh số nguyên, u128 để không tràn: available * 100 < total * pct.
    u128::from(s.available) * 100 < u128::from(s.total) * u128::from(threshold.get())
}

/// Ngưỡng Smart Boost đã được kiểm tra hợp lệ (newtype: giá trị sai không tạo được, nên hàm nhận nó khỏi phải kiểm tra lại).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct SmartThreshold(u8);

impl SmartThreshold {
    pub const MIN: u8 = 5;
    pub const MAX: u8 = 50;
    pub const DEFAULT: Self = Self(15);

    pub fn new(pct: u8) -> AppResult<Self> {
        if (Self::MIN..=Self::MAX).contains(&pct) {
            Ok(Self(pct))
        } else {
            Err(AppError::InvalidInput(format!(
                "ngưỡng Smart Boost phải trong {}–{}%, nhận {pct}%",
                Self::MIN,
                Self::MAX
            )))
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1 << 30;

    fn basic(total: u64, avail: u64) -> BasicMemory {
        BasicMemory {
            total_phys: total,
            avail_phys: avail,
            load_pct: 50,
            commit_total: 10 * GB,
            commit_limit: 20 * GB,
            system_cache: GB,
            page_size: 4096,
        }
    }

    fn status(total: u64, avail: u64) -> MemoryStatus {
        status_from(basic(total, avail), None)
    }

    #[test]
    fn used_is_total_minus_available() {
        let s = status(16 * GB, 6 * GB);
        assert_eq!(s.used, 10 * GB);
        assert_eq!(s.free, None);
    }

    #[test]
    fn available_is_clamped_to_total() {
        let s = status(8 * GB, 9 * GB);
        assert_eq!(s.available, 8 * GB);
        assert_eq!(s.used, 0);
    }

    #[test]
    fn lists_are_converted_from_pages() {
        let lists = MemoryLists { zero_pages: 10, free_pages: 20, modified_pages: 5, standby_pages: 100 };
        let s = status_from(basic(16 * GB, 6 * GB), Some(lists));
        assert_eq!(s.free, Some(30 * 4096));
        assert_eq!(s.modified, Some(5 * 4096));
        assert_eq!(s.standby, Some(100 * 4096));
    }

    /// Kịch bản giống máy thật: purge 15 GB standby → free tăng 15 GB nhưng
    /// available gần như không đổi. Báo cáo không được gọi đó là "giải phóng".
    #[test]
    fn purging_standby_is_reported_as_cache_not_gain() {
        let mut before = status(32 * GB, 15 * GB);
        before.free = Some(GB);
        before.standby = Some(14 * GB);
        before.modified = Some(GB / 2);
        // Sau flush: 256 MB modified thành standby → available +256 MB.
        let mut mid = before;
        mid.available = 15 * GB + GB / 4;
        mid.standby = Some(14 * GB + GB / 4);
        mid.modified = Some(GB / 4);
        // Sau purge toàn bộ standby.
        let mut after = mid;
        after.free = Some(15 * GB + GB / 4);
        after.standby = Some(0);

        let g = boost_gain(&before, &mid, &after);
        assert_eq!(g.available_gain, GB / 4);
        assert_eq!(g.standby_cleared, Some(14 * GB + GB / 4));
        assert_eq!(g.modified_flushed, Some(GB / 4));
        assert_eq!(g.free_gain, Some(14 * GB + GB / 4));
    }

    /// Tái hiện số đo thật của Boost nhẹ: flush 804 MB đổ vào standby, purge
    /// ưu tiên thấp xoá ~520 MB → standby ròng TĂNG. Vẫn phải báo đã xoá ~520 MB.
    #[test]
    fn light_boost_purge_not_hidden_by_flush() {
        const MB: u64 = 1 << 20;
        let mut before = status(32 * GB, 13 * GB);
        before.free = Some(GB);
        before.standby = Some(10 * GB);
        before.modified = Some(804 * MB);
        let mut mid = before;
        mid.standby = Some(10 * GB + 804 * MB);
        mid.modified = Some(0);
        let mut after = mid;
        after.standby = Some(10 * GB + 284 * MB);
        after.free = Some(GB + 520 * MB);

        let g = boost_gain(&before, &mid, &after);
        assert_eq!(g.modified_flushed, Some(804 * MB));
        assert_eq!(g.standby_cleared, Some(520 * MB));
        // Cách tính cũ (trước − sau cho cả quá trình) sẽ ra 0.
        assert_eq!(before.standby.unwrap().saturating_sub(after.standby.unwrap()), 0);
    }

    #[test]
    fn gain_never_negative_and_none_without_lists() {
        let before = status(16 * GB, 6 * GB);
        let after = status(16 * GB, 5 * GB);
        let g = boost_gain(&before, &before, &after);
        assert_eq!(g.available_gain, 0);
        assert_eq!((g.free_gain, g.standby_cleared, g.modified_flushed), (None, None, None));
    }

    #[test]
    fn smart_boost_threshold() {
        let t = SmartThreshold::DEFAULT; // 15%
        assert!(should_smart_boost(&status(100 * GB, 14 * GB), t));
        assert!(!should_smart_boost(&status(100 * GB, 15 * GB), t));
        assert!(!should_smart_boost(&status(0, 0), t));
    }

    #[test]
    fn smart_threshold_validation() {
        assert!(SmartThreshold::new(4).is_err());
        assert!(SmartThreshold::new(51).is_err());
        assert_eq!(SmartThreshold::new(20).unwrap().get(), 20);
    }

    #[test]
    fn available_pct_handles_zero_total() {
        assert_eq!(available_pct(&status(0, 0)), 0.0);
        assert!((available_pct(&status(200, 50)) - 25.0).abs() < f64::EPSILON);
    }
}
