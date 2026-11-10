//! Tính CPU% từ 2 lần lấy mẫu (logic thuần).

/// CPU% của một tiến trình giữa 2 mẫu, chuẩn hoá theo tổng số CPU (0..=100,
/// giống cột CPU của Task Manager).
///
/// - `prev`, `now`: tổng user+kernel time (100ns).
/// - `elapsed_100ns`: thời gian thực giữa 2 mẫu (100ns).
pub fn cpu_pct(prev: u64, now: u64, elapsed_100ns: u64, cpus: u32) -> f64 {
    if elapsed_100ns == 0 || cpus == 0 {
        return 0.0;
    }
    // now < prev chỉ xảy ra khi PID bị tái dùng — đã được loại bằng create_time,
    // nhưng vẫn saturate cho chắc.
    let used = now.saturating_sub(prev) as f64;
    let pct = used * 100.0 / (elapsed_100ns as f64 * f64::from(cpus));
    pct.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEC: u64 = 10_000_000;

    #[test]
    fn one_full_core_of_four_is_25_pct() {
        assert!((cpu_pct(0, SEC, SEC, 4) - 25.0).abs() < 1e-9);
    }

    #[test]
    fn idle_is_zero_and_never_negative() {
        assert_eq!(cpu_pct(5 * SEC, 5 * SEC, SEC, 8), 0.0);
        assert_eq!(cpu_pct(5 * SEC, SEC, SEC, 8), 0.0);
    }

    #[test]
    fn clamps_to_100_and_handles_zero_inputs() {
        assert_eq!(cpu_pct(0, 100 * SEC, SEC, 4), 100.0);
        assert_eq!(cpu_pct(0, SEC, 0, 4), 0.0);
        assert_eq!(cpu_pct(0, SEC, SEC, 0), 0.0);
    }
}
