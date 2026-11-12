//! Test tích hợp Cleaner trên thư mục sandbox thật trong %TEMP%.

use std::fs::{self, File, FileTimes, OpenOptions};
use std::os::windows::fs::{FileTimesExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

use super::safety::{BanCtx, BanList};
use super::*;

static SEQ: AtomicU32 = AtomicU32::new(0);

/// Thư mục tạm riêng cho mỗi test; tự xoá khi drop (gỡ junction trước).
struct Sandbox {
    base: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        // %TEMP% của user đang đăng nhập: khi chạy test bằng tài khoản admin KHÁC, profile của tài khoản
        // admin bị cấm dọn (đúng thiết kế) nên sandbox không được nằm ở đó.
        let temp = crate::user::env("TEMP").map_or_else(std::env::temp_dir, PathBuf::from);
        let base = temp.join(format!(
            "pcm-cleaner-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("root")).unwrap();
        fs::create_dir_all(base.join("outside")).unwrap();
        Self { base }
    }
    fn root(&self) -> PathBuf {
        self.base.join("root")
    }
    fn outside(&self) -> PathBuf {
        self.base.join("outside")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // Gỡ junction bằng remove_dir (chỉ gỡ link, không đụng đích) trước khi xoá cây.
        if let Ok(rd) = fs::read_dir(self.root()) {
            for e in rd.flatten() {
                if e.metadata().is_ok_and(|m| safety::is_link_or_cloud(&m)) {
                    let _ = fs::remove_dir(e.path());
                }
            }
        }
        let _ = fs::remove_dir_all(&self.base);
    }
}

const DAY: Duration = Duration::from_secs(24 * 3600);

/// Tạo file với ngày tạo + ngày sửa lùi `age` về quá khứ.
fn make_file(path: &Path, content: &[u8], age: Duration) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(path, content).unwrap();
    let t = SystemTime::now() - age;
    let f = OpenOptions::new().write(true).open(path).unwrap();
    f.set_times(FileTimes::new().set_modified(t).set_accessed(t).set_created(t)).unwrap();
}

fn junction(link: &Path, target: &Path) {
    let ok = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(ok, "không tạo được junction");
}

fn rule_for(root: &Path, extra: &str) -> Rule {
    let toml = format!(
        "version = 1\n[[rule]]\nid = \"test\"\nname = \"Test\"\ngroup = \"g\"\ndescription = \"d\"\nrisk = \"low\"\n\
         paths = ['{}']\nmin_age_hours = 24\n{extra}\n",
        root.display()
    );
    rules::parse(&toml).unwrap().remove(0)
}

fn manager(rule: Rule, bans: BanList) -> CleanerManager {
    CleanerManager::new(vec![rule], bans, true)
}

fn ready_ids(r: &ScanResult) -> Vec<String> {
    r.rules.iter().filter(|x| matches!(x.status, RuleStatus::Ready)).map(|x| x.id.clone()).collect()
}

fn noop(_: Progress) {}

/// CLAUDE.md AN TOÀN #3: không đi theo junction — file ở đích phải còn nguyên.
#[test]
fn junction_is_never_followed_or_deleted() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("old.tmp"), b"rac", 3 * DAY);
    make_file(&sb.outside().join("quan-trong.docx"), b"du lieu that", 30 * DAY);
    make_file(&sb.outside().join("sub").join("anh.jpg"), b"anh", 30 * DAY);
    junction(&sb.root().join("link-vao-ngoai"), &sb.outside());

    let m = manager(rule_for(&sb.root(), "remove_empty_dirs = true"), BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    let r = &scan.rules[0];
    assert_eq!(r.files, 1, "chỉ được thấy old.tmp, không thấy file qua junction");
    assert!(r.skipped_links >= 1);

    let store = Store::in_memory().unwrap();
    let rep = m.clean(&store, scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    assert_eq!(rep.deleted_files, 1);
    assert!(!sb.root().join("old.tmp").exists());
    assert!(sb.outside().join("quan-trong.docx").exists());
    assert!(sb.outside().join("sub").join("anh.jpg").exists());
    // Bản thân junction cũng không bị gỡ.
    assert!(fs::symlink_metadata(sb.root().join("link-vao-ngoai")).is_ok());
}

#[test]
fn junction_as_rule_root_is_ignored() {
    let sb = Sandbox::new();
    make_file(&sb.outside().join("x.tmp"), b"x", 3 * DAY);
    let link = sb.root().join("j");
    junction(&link, &sb.outside());
    let m = manager(rule_for(&link, ""), BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    assert!(matches!(scan.rules[0].status, RuleStatus::Empty));
    assert!(sb.outside().join("x.tmp").exists());
}

#[test]
fn young_files_are_kept() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("moi.tmp"), b"moi", Duration::from_secs(3600));
    make_file(&sb.root().join("cu.tmp"), b"cu", 2 * DAY);
    let m = manager(rule_for(&sb.root(), ""), BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    assert_eq!(scan.rules[0].files, 1);
    assert_eq!(scan.rules[0].skipped_young, 1);
    m.clean(&Store::in_memory().unwrap(), scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    assert!(sb.root().join("moi.tmp").exists());
    assert!(!sb.root().join("cu.tmp").exists());
}

#[test]
fn locked_file_is_skipped_not_forced() {
    let sb = Sandbox::new();
    let p = sb.root().join("dang-dung.tmp");
    make_file(&p, b"lock", 3 * DAY);
    let m = manager(rule_for(&sb.root(), ""), BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    // Mở file với share_mode = 0 (không chia sẻ) như ứng dụng đang giữ file.
    let _lock = OpenOptions::new().read(true).share_mode(0).open(&p).unwrap();
    let rep = m.clean(&Store::in_memory().unwrap(), scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    assert_eq!(rep.skipped_locked, 1);
    assert_eq!(rep.deleted_files, 0);
    drop(_lock);
    assert!(p.exists());
}

#[test]
fn file_changed_after_scan_is_not_deleted() {
    let sb = Sandbox::new();
    let p = sb.root().join("doi.tmp");
    make_file(&p, b"v1", 3 * DAY);
    let m = manager(rule_for(&sb.root(), ""), BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    // Ứng dụng ghi tiếp vào file sau khi quét.
    make_file(&p, b"v2-dai-hon", 3 * DAY);
    let rep = m.clean(&Store::in_memory().unwrap(), scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    assert_eq!(rep.skipped_changed, 1);
    assert!(p.exists());
}

#[test]
fn file_appearing_after_scan_is_not_deleted() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("a.tmp"), b"a", 3 * DAY);
    let m = manager(rule_for(&sb.root(), ""), BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    make_file(&sb.root().join("moi-xuat-hien.tmp"), b"b", 3 * DAY);
    m.clean(&Store::in_memory().unwrap(), scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    // Chỉ xoá đúng những gì đã xem trước.
    assert!(sb.root().join("moi-xuat-hien.tmp").exists());
}

#[test]
fn dry_run_deletes_nothing_and_keeps_scan_for_real_clean() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("a.tmp"), b"12345", 3 * DAY);
    let m = manager(rule_for(&sb.root(), ""), BanList::from_env());
    let store = Store::in_memory().unwrap();
    let scan = m.scan(&noop).unwrap();
    let dry = m.clean(&store, scan.scan_id, &ready_ids(&scan), true, &noop).unwrap();
    assert!(dry.dry_run);
    assert_eq!((dry.deleted_files, dry.freed_bytes), (1, 5));
    assert!(sb.root().join("a.tmp").exists());

    let real = m.clean(&store, scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    assert_eq!(real.deleted_files, 1);
    assert!(!sb.root().join("a.tmp").exists());

    let actions: Vec<String> = store.recent_actions(10).unwrap().into_iter().map(|e| e.action).collect();
    assert_eq!(actions, ["cleaner.clean", "cleaner.dry_run"]);
}

#[test]
fn auto_clean_does_not_replace_users_preview() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("a.tmp"), b"12345", 3 * DAY);
    let m = manager(rule_for(&sb.root(), ""), BanList::from_env());
    let store = Store::in_memory().unwrap();
    let a = m.scan(&noop).unwrap();
    let auto = m.auto_clean(&store, &["test".into()]).unwrap();
    assert_eq!(auto.deleted_files, 1);
    assert!(!sb.root().join("a.tmp").exists());
    // Bản xem trước A của người dùng vẫn dùng được (file đã mất thì chỉ bỏ qua).
    let rep = m.clean(&store, a.scan_id, &ready_ids(&a), false, &noop).unwrap();
    assert_eq!(rep.deleted_files, 0);
}

#[test]
fn scan_is_single_use_and_ids_are_checked() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("a.tmp"), b"a", 3 * DAY);
    let m = manager(rule_for(&sb.root(), ""), BanList::from_env());
    let store = Store::in_memory().unwrap();
    let scan = m.scan(&noop).unwrap();
    assert_eq!(m.clean(&store, scan.scan_id + 99, &ready_ids(&scan), false, &noop).unwrap_err().code(), "invalid_input");
    // id sai đã làm mất kết quả quét => phải quét lại.
    assert!(m.clean(&store, scan.scan_id, &ready_ids(&scan), false, &noop).is_err());

    let scan = m.scan(&noop).unwrap();
    let rep = m.clean(&store, scan.scan_id, &["khong-ton-tai".into()], false, &noop).unwrap();
    assert_eq!(rep.deleted_files, 0);
    assert!(sb.root().join("a.tmp").exists());
}

#[test]
fn include_exclude_and_non_recursive() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("x.tmp"), b"x", 3 * DAY);
    make_file(&sb.root().join("giu.log"), b"x", 3 * DAY);
    make_file(&sb.root().join("giu-lai.tmp"), b"x", 3 * DAY);
    make_file(&sb.root().join("con").join("y.tmp"), b"x", 3 * DAY);
    let rule = rule_for(&sb.root(), "include = ['*.tmp']\nexclude = ['giu-*']\nrecursive = false");
    let m = manager(rule, BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    m.clean(&Store::in_memory().unwrap(), scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    assert!(!sb.root().join("x.tmp").exists());
    assert!(sb.root().join("giu.log").exists());
    assert!(sb.root().join("giu-lai.tmp").exists());
    assert!(sb.root().join("con").join("y.tmp").exists());
}

/// Danh sách cấm thắng rule: file trong thư mục cấm không bao giờ bị xoá.
#[test]
fn banned_folder_inside_rule_root_is_untouched() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("rac.tmp"), b"x", 3 * DAY);
    make_file(&sb.root().join("Documents").join("hop-dong.tmp"), b"x", 3 * DAY);
    let mut ctx = BanCtx::from_env();
    ctx.user_folders.push(sb.root().join("Documents").display().to_string());
    let m = manager(rule_for(&sb.root(), ""), BanList::new(&ctx));
    let scan = m.scan(&noop).unwrap();
    m.clean(&Store::in_memory().unwrap(), scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    assert!(!sb.root().join("rac.tmp").exists());
    assert!(sb.root().join("Documents").join("hop-dong.tmp").exists());
}

#[test]
fn empty_subdirs_removed_but_root_kept() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("a").join("b").join("c.tmp"), b"x", 3 * DAY);
    let m = manager(rule_for(&sb.root(), "remove_empty_dirs = true"), BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    m.clean(&Store::in_memory().unwrap(), scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    assert!(!sb.root().join("a").exists());
    assert!(sb.root().exists());
}

#[test]
fn needs_admin_and_running_app_block_rule() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("a.tmp"), b"x", 3 * DAY);
    let rule = rule_for(&sb.root(), "requires_admin = true");
    let m = CleanerManager::new(vec![rule], BanList::from_env(), false);
    assert!(matches!(m.scan(&noop).unwrap().rules[0].status, RuleStatus::NeedsAdmin));

    // Tên tiến trình của chính bộ test luôn đang chạy.
    let me = std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().into_owned();
    let rule = rule_for(&sb.root(), &format!("skip_if_running = ['{me}']"));
    let m = manager(rule, BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    assert!(matches!(scan.rules[0].status, RuleStatus::Blocked { .. }));
    assert!(ready_ids(&scan).is_empty());
    assert!(sb.root().join("a.tmp").exists());
}

#[test]
fn wildcard_profile_dirs_are_expanded() {
    let sb = Sandbox::new();
    make_file(&sb.root().join("Default").join("Cache").join("1"), b"x", 3 * DAY);
    make_file(&sb.root().join("Profile 1").join("Cache").join("2"), b"x", 3 * DAY);
    make_file(&sb.root().join("Profile 1").join("Bookmarks"), b"x", 3 * DAY);
    let tpl = format!(r"{}\*\Cache", sb.root().display());
    let roots = engine::expand_roots(&tpl, &|k| std::env::var(k).ok());
    assert_eq!(roots.len(), 2);
    let m = manager(rule_for(Path::new(&tpl), ""), BanList::from_env());
    let scan = m.scan(&noop).unwrap();
    assert_eq!(scan.rules[0].files, 2);
    m.clean(&Store::in_memory().unwrap(), scan.scan_id, &ready_ids(&scan), false, &noop).unwrap();
    assert!(sb.root().join("Profile 1").join("Bookmarks").exists(), "chỉ xoá trong Cache");
}

#[test]
fn every_embedded_rule_root_is_outside_ban_list() {
    // Gốc của rule thật không được nằm trong vùng cấm (nếu có thì rule vô dụng/nguy hiểm).
    let bans = BanList::from_env();
    for r in rules::embedded().unwrap() {
        for p in &r.paths {
            for root in engine::expand_roots(p, &crate::user::env) {
                let probe = root.join("probe.tmp");
                assert!(bans.check(&probe).is_none(), "rule {} trỏ vào vùng cấm: {}", r.id, root.display());
            }
        }
    }
}

#[test]
fn real_scan_with_embedded_rules_is_read_only() {
    // Quét thật trên máy (không dọn) — chỉ kiểm tra chạy được và không panic.
    let m = CleanerManager::from_system();
    let scan = m.scan(&noop).unwrap();
    assert!(!scan.cancelled);
    assert!(scan.rules.iter().any(|r| r.id == "user-temp"));
}

#[test]
fn cancel_stops_scan_and_result_cannot_be_cleaned() {
    let sb = Sandbox::new();
    for i in 0..50 {
        make_file(&sb.root().join(format!("{i}.tmp")), b"x", 3 * DAY);
    }
    let m = manager(rule_for(&sb.root(), ""), BanList::from_env());
    let scan = m.scan(&|_| m.cancel()).unwrap();
    assert!(scan.cancelled);
    assert!(m.clean(&Store::in_memory().unwrap(), scan.scan_id, &["test".into()], false, &noop).is_err());
    assert!(sb.root().join("0.tmp").exists());
    let _ = File::open(sb.root().join("1.tmp")).unwrap();
}
