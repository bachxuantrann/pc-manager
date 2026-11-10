//! Rule dọn rác — nhúng từ `rules/cleaner.toml` lúc biên dịch.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const EMBEDDED: &str = include_str!("../../../rules/cleaner.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    #[default]
    Files,
    RecycleBin,
}

fn yes() -> bool {
    true
}
fn all() -> Vec<String> {
    vec!["*".into()]
}
fn day() -> u32 {
    24
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub group: String,
    pub description: String,
    pub risk: Risk,
    #[serde(default)]
    pub kind: RuleKind,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default = "all")]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default = "yes")]
    pub recursive: bool,
    /// Mặc định 24h nếu rule không ghi — an toàn hơn 0.
    #[serde(default = "day")]
    pub min_age_hours: u32,
    #[serde(default)]
    pub remove_empty_dirs: bool,
    #[serde(default)]
    pub requires_admin: bool,
    #[serde(default)]
    pub skip_if_running: Vec<String>,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default = "yes")]
    pub default_selected: bool,
}

impl Rule {
    pub fn file_matches(&self, name: &str) -> bool {
        self.include.iter().any(|p| super::glob::matches(p, name)) && !self.exclude.iter().any(|p| super::glob::matches(p, name))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleFile {
    version: u32,
    #[serde(rename = "rule")]
    rules: Vec<Rule>,
}

/// Parse + kiểm tra cấu trúc. Lỗi ở đây là lỗi lập trình (file nhúng sẵn) —
/// test `embedded_rules_are_valid` đảm bảo không bao giờ xảy ra ở bản build.
pub fn parse(src: &str) -> AppResult<Vec<Rule>> {
    let file: RuleFile = toml::from_str(src).map_err(|e| AppError::Other(format!("cleaner.toml: {e}")))?;
    if file.version != 1 {
        return Err(AppError::Other(format!("cleaner.toml: version {} không hỗ trợ", file.version)));
    }
    let mut seen = std::collections::HashSet::new();
    for r in &file.rules {
        let bad = |m: &str| Err(AppError::Other(format!("cleaner.toml rule '{}': {m}", r.id)));
        if r.id.is_empty() || !r.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return bad("id chỉ gồm chữ, số và '-'");
        }
        if !seen.insert(r.id.clone()) {
            return bad("id bị trùng");
        }
        match r.kind {
            RuleKind::Files => {
                if r.paths.is_empty() {
                    return bad("rule files phải có ít nhất một path");
                }
                if r.include.is_empty() {
                    return bad("include rỗng");
                }
            }
            RuleKind::RecycleBin => {
                if !r.paths.is_empty() {
                    return bad("rule recycle_bin không được có path");
                }
            }
        }
        for p in &r.paths {
            // Gốc phải bắt đầu bằng %BIẾN% hệ thống hoặc ổ đĩa — không đường dẫn tương đối.
            let rooted = p.starts_with('%') || p.chars().nth(1) == Some(':');
            if !rooted || p.split(['\\', '/']).any(|seg| seg == ".." || seg == ".") {
                return bad("path phải tuyệt đối và không chứa '.' hoặc '..'");
            }
        }
    }
    Ok(file.rules)
}

pub fn embedded() -> AppResult<Vec<Rule>> {
    parse(EMBEDDED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_rules_are_valid() {
        let rules = embedded().unwrap();
        assert!(rules.len() >= 10);
        assert!(rules.iter().any(|r| r.kind == RuleKind::RecycleBin));
        // Thùng rác là dữ liệu người dùng: không bao giờ được chọn sẵn.
        let rb = rules.iter().find(|r| r.kind == RuleKind::RecycleBin).unwrap();
        assert!(!rb.default_selected);
        assert_eq!(rb.risk, Risk::High);
    }

    #[test]
    fn rejects_relative_and_dotdot_paths() {
        let base = r#"version = 1
[[rule]]
id = "x"
name = "x"
group = "g"
description = "d"
risk = "low"
"#;
        for p in [r#"paths = ["Temp"]"#, r#"paths = ["%TEMP%\\..\\.."]"#, r#"paths = ["C:\\Users\\.\\x"]"#] {
            assert!(parse(&format!("{base}{p}")).is_err(), "{p}");
        }
        assert!(parse(&format!("{base}paths = [\"%TEMP%\"]")).is_ok());
    }

    #[test]
    fn rejects_unknown_fields_and_duplicates() {
        let r = r#"version = 1
[[rule]]
id = "x"
name = "x"
group = "g"
description = "d"
risk = "low"
paths = ["%TEMP%"]
follow_symlinks = true
"#;
        assert!(parse(r).is_err(), "trường lạ phải bị từ chối");
        let dup = r#"version = 1
[[rule]]
id = "x"
name = "x"
group = "g"
description = "d"
risk = "low"
paths = ["%TEMP%"]
[[rule]]
id = "x"
name = "y"
group = "g"
description = "d"
risk = "low"
paths = ["%TEMP%"]
"#;
        assert!(parse(dup).is_err());
    }

    #[test]
    fn default_min_age_is_24h() {
        let r = parse(
            r#"version = 1
[[rule]]
id = "x"
name = "x"
group = "g"
description = "d"
risk = "low"
paths = ["%TEMP%"]
"#,
        )
        .unwrap();
        assert_eq!(r[0].min_age_hours, 24);
        assert!(r[0].file_matches("anything.tmp"));
    }
}
