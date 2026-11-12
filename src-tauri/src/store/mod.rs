//! Lưu trữ SQLite: nhật ký thao tác hệ thống + cài đặt key/value.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;
use crate::sync::LockExt;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS action_log (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    ts      INTEGER NOT NULL,          -- unix ms
    action  TEXT    NOT NULL,          -- vd: memory.boost
    target  TEXT    NOT NULL,          -- đối tượng tác động
    ok      INTEGER NOT NULL,          -- 1 = thành công
    detail  TEXT    NOT NULL           -- kết quả / lỗi
);
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
-- PC Manager tự lấy mẫu tiến trình đang chạy mỗi 60s (nguồn 'lần dùng cuối').
CREATE TABLE IF NOT EXISTS app_usage (
    exe_path   TEXT PRIMARY KEY,       -- viết thường
    first_seen INTEGER NOT NULL,       -- unix ms
    last_seen  INTEGER NOT NULL
);
";

/// Giữ bấy nhiêu dòng nhật ký mới nhất (quyết định của người dùng 2026-11-12).
const KEEP_ACTIONS: i64 = 5_000;
/// Xoá mẫu "lần dùng cuối" của exe không thấy chạy hơn 1 năm (vd. trình cài tên ngẫu nhiên trong Temp).
const USAGE_MAX_AGE_MS: i64 = 365 * 24 * 3600 * 1000;

/// Kết nối SQLite dùng chung. `Mutex` vì `rusqlite::Connection` không `Sync`.
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> AppResult<Self> {
        Self::init(Connection::open(path)?)
    }

    /// DB trong RAM — dùng cho test và khi không mở được file DB.
    pub fn in_memory() -> AppResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> AppResult<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch(SCHEMA)?;
        // Lỗi dọn bớt không được làm hỏng việc mở DB.
        if let Err(e) = prune(&conn, crate::settings::now_ms()) {
            eprintln!("[store] không dọn bớt được dữ liệu cũ: {e}");
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> AppResult<std::sync::MutexGuard<'_, Connection>> {
        // Mutex bị poison nếu một luồng panic khi đang giữ (release dùng panic = "unwind").
        // Kết nối SQLite vẫn dùng được (giao dịch dở dang đã tự rollback), nên lấy lại khoá.
        Ok(self.conn.lock_recover())
    }

    /// Ghi nhật ký một thao tác thay đổi hệ thống (CLAUDE.md, AN TOÀN #7).
    pub fn log_action(&self, action: &str, target: &str, ok: bool, detail: &str) -> AppResult<()> {
        self.conn()?.execute(
            "INSERT INTO action_log (ts, action, target, ok, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![crate::settings::now_ms(), action, target, ok, detail],
        )?;
        Ok(())
    }

    /// Như `log_action` nhưng không làm hỏng thao tác chính nếu ghi log lỗi.
    pub fn log_action_best_effort(&self, action: &str, target: &str, ok: bool, detail: &str) {
        if let Err(e) = self.log_action(action, target, ok, detail) {
            eprintln!("[store] không ghi được log {action}: {e}");
        }
    }

    pub fn recent_actions(&self, limit: u32) -> AppResult<Vec<ActionLogEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT ts, action, target, ok, detail FROM action_log ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(ActionLogEntry {
                ts: r.get(0)?,
                action: r.get(1)?,
                target: r.get(2)?,
                ok: r.get(3)?,
                detail: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get_setting(&self, key: &str) -> AppResult<Option<String>> {
        Ok(self
            .conn()?
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        self.conn()?.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Ghi nhận các exe đang chạy (một transaction cho cả lô).
    pub fn record_usage(&self, exe_paths: &[String], at_ms: i64) -> AppResult<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO app_usage (exe_path, first_seen, last_seen) VALUES (?1, ?2, ?2)
                 ON CONFLICT(exe_path) DO UPDATE SET last_seen = MAX(last_seen, excluded.last_seen)",
            )?;
            for p in exe_paths {
                stmt.execute(params![p.to_lowercase(), at_ms])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// (exe viết thường, last_seen ms).
    pub fn usage(&self) -> AppResult<Vec<(String, i64)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT exe_path, last_seen FROM app_usage")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Thời điểm bắt đầu theo dõi (mẫu đầu tiên), để UI giải thích "chưa thấy dùng".
    pub fn usage_since(&self) -> AppResult<Option<i64>> {
        Ok(self.conn()?.query_row("SELECT MIN(first_seen) FROM app_usage", [], |r| r.get(0))?)
    }

    /// Đọc setting rồi parse; thiếu hoặc sai định dạng thì dùng `default`.
    pub fn setting_or<T: std::str::FromStr>(&self, key: &str, default: T) -> T {
        self.get_setting(key)
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(default)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActionLogEntry {
    pub ts: i64,
    pub action: String,
    pub target: String,
    pub ok: bool,
    pub detail: String,
}


/// Chính sách lưu giữ, chạy mỗi lần mở DB: giữ `KEEP_ACTIONS` dòng nhật ký mới nhất và bỏ mẫu
/// `app_usage` cũ hơn `USAGE_MAX_AGE_MS` — DB không lớn dần mãi khi app chạy nền nhiều năm.
fn prune(conn: &Connection, now_ms: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM action_log WHERE id <= (SELECT id FROM action_log ORDER BY id DESC LIMIT 1 OFFSET ?1)",
        [KEEP_ACTIONS],
    )?;
    conn.execute("DELETE FROM app_usage WHERE last_seen < ?1", [now_ms.saturating_sub(USAGE_MAX_AGE_MS)])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopening_keeps_only_newest_actions_and_recent_usage() {
        let path = std::env::temp_dir().join(format!("pcm-store-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let s = Store::open(&path).unwrap();
            let conn = s.conn().unwrap();
            let n = KEEP_ACTIONS + 100;
            conn.execute(
                "WITH RECURSIVE c(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM c WHERE i < ?1)
                 INSERT INTO action_log (ts, action, target, ok, detail) SELECT i, 't', 'x', 1, '' FROM c",
                [n],
            )
            .unwrap();
            let now = crate::settings::now_ms();
            conn.execute("INSERT INTO app_usage VALUES ('old.exe', 0, ?1)", [now - USAGE_MAX_AGE_MS - 1]).unwrap();
            conn.execute("INSERT INTO app_usage VALUES ('new.exe', 0, ?1)", [now]).unwrap();
        }
        let s = Store::open(&path).unwrap();
        let count: i64 = s.conn().unwrap().query_row("SELECT COUNT(*) FROM action_log", [], |r| r.get(0)).unwrap();
        assert_eq!(count, KEEP_ACTIONS);
        let newest: i64 = s.conn().unwrap().query_row("SELECT MAX(ts) FROM action_log", [], |r| r.get(0)).unwrap();
        assert_eq!(newest, KEEP_ACTIONS + 100, "giữ dòng mới nhất");
        let usage: Vec<String> = s.usage().unwrap().into_iter().map(|(p, _)| p).collect();
        assert_eq!(usage, ["new.exe"]);
        drop(s);
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
    }

    #[test]
    fn logs_are_returned_newest_first() {
        let s = Store::in_memory().unwrap();
        s.log_action("memory.boost", "system", true, "freed 10MB").unwrap();
        s.log_action("memory.boost", "system", false, "no privilege").unwrap();
        let rows = s.recent_actions(10).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].ok);
        assert_eq!(rows[1].detail, "freed 10MB");
    }

    #[test]
    fn usage_keeps_latest_and_first_seen() {
        let s = Store::in_memory().unwrap();
        s.record_usage(&[r"C:\App\A.exe".into()], 1000).unwrap();
        s.record_usage(&[r"c:\app\a.exe".into(), r"C:\B.exe".into()], 5000).unwrap();
        s.record_usage(&[r"C:\App\A.exe".into()], 3000).unwrap(); // mẫu cũ đến muộn
        let mut u = s.usage().unwrap();
        u.sort();
        assert_eq!(u, vec![(r"c:\app\a.exe".into(), 5000), (r"c:\b.exe".into(), 5000)]);
        assert_eq!(s.usage_since().unwrap(), Some(1000));
    }

    #[test]
    fn settings_upsert_and_default() {
        let s = Store::in_memory().unwrap();
        assert_eq!(s.setting_or("x", 15u8), 15);
        s.set_setting("x", "20").unwrap();
        s.set_setting("x", "25").unwrap();
        assert_eq!(s.setting_or("x", 15u8), 25);
        s.set_setting("x", "rác").unwrap();
        assert_eq!(s.setting_or("x", 15u8), 15);
    }
}
