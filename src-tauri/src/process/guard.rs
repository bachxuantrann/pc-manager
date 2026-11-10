//! Chặn thao tác nguy hiểm lên tiến trình hệ thống (CLAUDE.md, AN TOÀN #5).
//! Logic thuần để test được; tầng gọi truyền vào kết quả `IsProcessCritical`.

/// Tiến trình hệ thống quan trọng — kill/suspend sẽ làm treo hoặc BSOD.
/// So sánh không phân biệt hoa thường theo tên file.
pub const PROTECTED_NAMES: &[&str] = &[
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "lsass.exe",
    "smss.exe",
    "services.exe",
    // Tiến trình giả của kernel.
    "system",
    "system idle process",
    "registry",
    "memory compression",
    "secure system",
];

/// PID 0 (Idle) và 4 (System).
pub const PROTECTED_PIDS: &[u32] = &[0, 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Kill,
    Suspend,
    Resume,
    SetPriority,
    Trim,
    OpenLocation,
}

impl Op {
    /// Thao tác có thể làm hỏng hệ thống nếu nhắm vào tiến trình được bảo vệ.
    fn is_dangerous(self) -> bool {
        matches!(self, Op::Kill | Op::Suspend | Op::SetPriority | Op::Trim)
    }
}

/// Lý do tĩnh (không cần mở handle) khiến tiến trình được bảo vệ — để UI khoá nút.
pub fn static_protection(pid: u32, name: &str, self_pid: u32) -> Option<&'static str> {
    if PROTECTED_PIDS.contains(&pid) {
        return Some("tiến trình nhân hệ thống");
    }
    let lower = name.to_ascii_lowercase();
    if PROTECTED_NAMES.contains(&lower.as_str()) {
        return Some("tiến trình hệ thống quan trọng");
    }
    if pid == self_pid {
        return Some("chính PC Manager");
    }
    None
}

/// Kiểm tra trước khi thực hiện `op`. `critical` = kết quả IsProcessCritical.
pub fn check(op: Op, pid: u32, name: &str, self_pid: u32, critical: bool) -> Result<(), String> {
    if !op.is_dangerous() {
        return Ok(());
    }
    if pid == self_pid {
        // Cho phép trim/đổi ưu tiên chính mình, chặn kill/suspend.
        return match op {
            Op::Kill | Op::Suspend => Err("không thể dừng chính PC Manager".into()),
            _ => Ok(()),
        };
    }
    if let Some(reason) = static_protection(pid, name, self_pid) {
        return Err(format!("{name} là {reason}, không được phép thao tác"));
    }
    if critical && matches!(op, Op::Kill | Op::Suspend) {
        return Err(format!("{name} được Windows đánh dấu critical — dừng nó sẽ gây màn hình xanh"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: u32 = 9999;

    #[test]
    fn all_listed_system_processes_are_blocked_for_kill() {
        for name in ["csrss.exe", "wininit.exe", "winlogon.exe", "lsass.exe", "smss.exe", "services.exe"] {
            assert!(check(Op::Kill, 500, name, ME, false).is_err(), "{name}");
            assert!(check(Op::Suspend, 500, name, ME, false).is_err(), "{name}");
        }
    }

    #[test]
    fn name_match_is_case_insensitive() {
        assert!(check(Op::Kill, 500, "LSASS.EXE", ME, false).is_err());
        assert!(check(Op::Kill, 500, "Csrss.exe", ME, false).is_err());
    }

    #[test]
    fn pid_0_and_4_always_blocked() {
        assert!(check(Op::Kill, 0, "whatever.exe", ME, false).is_err());
        assert!(check(Op::Kill, 4, "System", ME, false).is_err());
        assert!(check(Op::SetPriority, 4, "System", ME, false).is_err());
    }

    #[test]
    fn self_cannot_be_killed_or_suspended() {
        assert!(check(Op::Kill, ME, "pc-manager.exe", ME, false).is_err());
        assert!(check(Op::Suspend, ME, "pc-manager.exe", ME, false).is_err());
        assert!(check(Op::Trim, ME, "pc-manager.exe", ME, false).is_ok());
    }

    #[test]
    fn critical_flag_blocks_kill_even_with_unknown_name() {
        assert!(check(Op::Kill, 700, "svchost.exe", ME, true).is_err());
        assert!(check(Op::Kill, 700, "svchost.exe", ME, false).is_ok());
    }

    #[test]
    fn harmless_ops_always_allowed() {
        assert!(check(Op::Resume, 4, "System", ME, true).is_ok());
        assert!(check(Op::OpenLocation, 500, "lsass.exe", ME, true).is_ok());
    }

    #[test]
    fn normal_app_is_allowed() {
        assert!(check(Op::Kill, 1234, "chrome.exe", ME, false).is_ok());
        assert_eq!(static_protection(1234, "chrome.exe", ME), None);
    }
}
