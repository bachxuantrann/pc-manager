//! Giá trị nhị phân trong `...\Explorer\StartupApproved\{Run,Run32,StartupFolder}`
//! — cơ chế bật/tắt của Task Manager.
//!
//! Định dạng (12 byte): 4 byte cờ + 8 byte FILETIME lúc tắt.
//! - Byte đầu chẵn (0x02, 0x06) = bật; lẻ (0x03, 0x07) = tắt.
//! - Không có giá trị = bật (Task Manager chưa từng đụng tới).

/// 100ns giữa 1601-01-01 và 1970-01-01.
const FILETIME_UNIX_EPOCH: u64 = 116_444_736_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovedState {
    pub enabled: bool,
    /// Thời điểm bị tắt (unix ms) nếu Task Manager/ta có ghi.
    pub disabled_at_ms: Option<i64>,
}

/// Đọc trạng thái. `None` (không có giá trị) hoặc dữ liệu rỗng => bật.
pub fn parse(bytes: Option<&[u8]>) -> ApprovedState {
    let Some(b) = bytes.filter(|b| !b.is_empty()) else {
        return ApprovedState { enabled: true, disabled_at_ms: None };
    };
    let enabled = b[0] & 1 == 0;
    let disabled_at_ms = if enabled { None } else { filetime_at(b, 4).and_then(filetime_to_unix_ms) };
    ApprovedState { enabled, disabled_at_ms }
}

/// Mã hoá giống Task Manager: bật = 02 + 11 byte 0; tắt = 03 00 00 00 + FILETIME.
pub fn encode(enabled: bool, now_filetime: u64) -> [u8; 12] {
    let mut out = [0u8; 12];
    if enabled {
        out[0] = 0x02;
    } else {
        out[0] = 0x03;
        out[4..].copy_from_slice(&now_filetime.to_le_bytes());
    }
    out
}

fn filetime_at(b: &[u8], offset: usize) -> Option<u64> {
    let raw: [u8; 8] = b.get(offset..offset + 8)?.try_into().ok()?;
    Some(u64::from_le_bytes(raw)).filter(|&v| v != 0)
}

pub fn filetime_to_unix_ms(ft: u64) -> Option<i64> {
    i64::try_from(ft.checked_sub(FILETIME_UNIX_EPOCH)? / 10_000).ok()
}

pub fn now_filetime() -> u64 {
    let since_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos() / 100).unwrap_or(u64::MAX));
    FILETIME_UNIX_EPOCH.saturating_add(since_unix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_empty_value_means_enabled() {
        assert!(parse(None).enabled);
        assert!(parse(Some(&[])).enabled);
    }

    /// Các mẫu cờ Task Manager dùng: chẵn = bật, lẻ = tắt.
    #[test]
    fn task_manager_flag_values() {
        assert!(parse(Some(&[0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])).enabled);
        assert!(parse(Some(&[0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])).enabled);
        assert!(!parse(Some(&[0x07, 0, 0, 0])).enabled);
        // 03 + FILETIME của 2024-01-01T00:00:00Z.
        let ft: u64 = FILETIME_UNIX_EPOCH + 1_704_067_200_000 * 10_000;
        let mut off = vec![0x03, 0, 0, 0];
        off.extend_from_slice(&ft.to_le_bytes());
        assert_eq!(
            parse(Some(&off)),
            ApprovedState { enabled: false, disabled_at_ms: Some(1_704_067_200_000) }
        );
    }

    #[test]
    fn encode_roundtrip_keeps_time() {
        // 2026-01-01T00:00:00Z
        let ft = FILETIME_UNIX_EPOCH + 1_767_225_600_000 * 10_000;
        let off = encode(false, ft);
        assert_eq!(&off[..4], &[0x03, 0, 0, 0]);
        let s = parse(Some(&off));
        assert!(!s.enabled);
        assert_eq!(s.disabled_at_ms, Some(1_767_225_600_000));

        let on = encode(true, ft);
        assert_eq!(on, [0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(parse(Some(&on)), ApprovedState { enabled: true, disabled_at_ms: None });
    }

    #[test]
    fn short_disabled_value_has_no_time() {
        assert_eq!(parse(Some(&[0x03])), ApprovedState { enabled: false, disabled_at_ms: None });
    }

    #[test]
    fn now_is_after_2024() {
        assert!(filetime_to_unix_ms(now_filetime()).unwrap() > 1_704_067_200_000);
    }
}
