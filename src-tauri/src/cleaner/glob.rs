//! So khớp mẫu tên file kiểu Windows: `*` (0+ ký tự), `?` (1 ký tự),
//! không phân biệt hoa thường. Không hỗ trợ `**` hay `[...]` — cố ý tối giản.

pub fn matches(pattern: &str, name: &str) -> bool {
    matches_lower(&pattern.to_lowercase(), name)
}

/// Như `matches` nhưng `pattern` đã viết thường sẵn (rule hạ chữ thường mẫu một lần lúc nạp,
/// không phải ở mỗi file).
pub fn matches_lower(pattern: &str, name: &str) -> bool {
    // Mẫu mặc định của hầu hết rule: khớp mọi tên, không cần cấp phát cho từng file.
    if pattern == "*" {
        return true;
    }
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    // Thuật toán tham lam với backtrack 1 điểm (O(n*m) xấu nhất, không đệ quy).
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

pub fn has_wildcard(s: &str) -> bool {
    s.contains(['*', '?'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        assert!(matches("*", "anything.tmp"));
        assert!(matches("*.dmp", "chrome.exe.1234.DMP"));
        assert!(matches("thumbcache_*.db", "thumbcache_1024.db"));
        assert!(!matches("thumbcache_*.db", "iconcache_1024.db"));
        assert!(matches("MEMORY.DMP", "memory.dmp"));
        assert!(!matches("MEMORY.DMP", "memory.dmp.bak"));
        assert!(matches("a?c", "abc"));
        assert!(!matches("a?c", "ac"));
        assert!(matches("*a*b*", "xxaYYbzz"));
        assert!(!matches("*.log", "log"));
        assert!(matches("", ""));
        assert!(!matches("", "x"));
    }

    #[test]
    fn unicode_names() {
        assert!(matches("*.txt", "tài liệu.TXT"));
        assert!(matches("Ảnh*", "ảnh 1.png"));
    }
}
