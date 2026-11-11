//! Ứng dụng khởi động cùng Windows.
//!
//! Nguồn: Run/RunOnce (HKCU, HKLM, WOW6432Node), thư mục Startup (user + common),
//! scheduled task trigger logon (chỉ đọc), service auto-start (chỉ đọc).
//!
//! Bật/tắt CHỈ qua `Explorer\StartupApproved` giống Task Manager — không bao giờ
//! sửa hay xoá entry gốc trong Run / thư mục Startup (CLAUDE.md, AN TOÀN #6).

pub mod approved;
pub mod command;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use windows_registry::{Key, Type, LOCAL_MACHINE};

use crate::error::{AppError, AppResult};
use crate::process::publisher::{Publisher, PublisherCache};
use crate::store::Store;
use crate::sys::com::ComGuard;
use crate::sys::{shell, tasks};

const APPROVED_BASE: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN32: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
const RUN_ONCE: &str = r"Software\Microsoft\Windows\CurrentVersion\RunOnce";
const RUN_ONCE32: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\RunOnce";
const SERVICES: &str = r"SYSTEM\CurrentControlSet\Services";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    RunUser,
    RunUser32,
    RunMachine,
    RunMachine32,
    RunOnceUser,
    RunOnceMachine,
    RunOnceMachine32,
    StartupFolderUser,
    StartupFolderCommon,
    ScheduledTask,
    Service,
}

#[derive(Clone, Copy)]
enum Hive {
    User,
    Machine,
}

impl Hive {
    /// Gốc registry. `User` là HKCU của user đang đăng nhập (`HKU<SID>` khi app được nâng
    /// quyền bằng tài khoản admin khác — Run/StartupApproved của admin không phải của người dùng).
    fn key(self) -> windows_registry::Result<Key> {
        match self {
            Hive::User => crate::user::hkcu(),
            Hive::Machine => LOCAL_MACHINE.open(""),
        }
    }
    fn prefix(self) -> &'static str {
        match self {
            Hive::User => "HKCU",
            Hive::Machine => "HKLM",
        }
    }
}

/// Nguồn registry: (hive, key Run, tên subkey StartupApproved nếu bật/tắt được).
fn reg_source(s: Source) -> Option<(Hive, &'static str, Option<&'static str>)> {
    Some(match s {
        Source::RunUser => (Hive::User, RUN, Some("Run")),
        Source::RunUser32 => (Hive::User, RUN32, Some("Run32")),
        Source::RunMachine => (Hive::Machine, RUN, Some("Run")),
        Source::RunMachine32 => (Hive::Machine, RUN32, Some("Run32")),
        // RunOnce chạy 1 lần rồi tự xoá — Task Manager không quản lý, ta chỉ hiển thị.
        Source::RunOnceUser => (Hive::User, RUN_ONCE, None),
        Source::RunOnceMachine => (Hive::Machine, RUN_ONCE, None),
        Source::RunOnceMachine32 => (Hive::Machine, RUN_ONCE32, None),
        _ => return None,
    })
}

/// Thư mục Startup và hive chứa StartupApproved\StartupFolder tương ứng.
fn folder_source(s: Source) -> Option<(PathBuf, Hive)> {
    let (var, rel, hive) = match s {
        Source::StartupFolderUser => ("APPDATA", r"Microsoft\Windows\Start Menu\Programs\Startup", Hive::User),
        Source::StartupFolderCommon => ("ProgramData", r"Microsoft\Windows\Start Menu\Programs\StartUp", Hive::Machine),
        _ => return None,
    };
    Some((PathBuf::from(crate::user::env(var)?).join(rel), hive))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEntry {
    pub id: String,
    pub name: String,
    /// Tên dùng khi bật/tắt: tên value trong Run, hoặc tên file (kèm .lnk) trong thư mục Startup.
    pub key_name: String,
    pub source: Source,
    /// Nơi khai báo: đường dẫn key registry / thư mục / task.
    pub location: String,
    pub command: String,
    pub path: Option<String>,
    pub publisher: Publisher,
    pub enabled: bool,
    pub can_toggle: bool,
    pub disabled_at_ms: Option<i64>,
    pub note: Option<String>,
}

/// Môi trường dùng để phân tích dòng lệnh (tiêm vào để test).
struct Ctx<'a> {
    publishers: &'a PublisherCache,
    windows_dir: String,
    search_dirs: Vec<PathBuf>,
}

impl<'a> Ctx<'a> {
    fn new(publishers: &'a PublisherCache) -> Self {
        let windows_dir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let search_dirs = vec![PathBuf::from(&windows_dir).join("System32"), PathBuf::from(&windows_dir)];
        Self { publishers, windows_dir, search_dirs }
    }

    fn expand(&self, s: &str) -> String {
        command::expand_env(s, crate::user::env)
    }

    fn exe(&self, cmd: &str) -> Option<PathBuf> {
        let exists = |p: &Path| p.is_file();
        command::executable_path(&self.expand(cmd), &exists, &self.search_dirs)
    }

    fn publisher(&self, path: Option<&Path>) -> Publisher {
        match path {
            Some(p) if p.is_file() => self.publishers.get(p),
            _ => Publisher::Unknown,
        }
    }
}

/// Liệt kê mọi nguồn. Nguồn nào lỗi (thiếu quyền, không tồn tại) thì bỏ qua.
pub fn list(publishers: &PublisherCache) -> AppResult<Vec<StartupEntry>> {
    let ctx = Ctx::new(publishers);
    let com = ComGuard::init()?;
    let mut out = Vec::new();
    for s in [
        Source::RunUser,
        Source::RunUser32,
        Source::RunMachine,
        Source::RunMachine32,
        Source::RunOnceUser,
        Source::RunOnceMachine,
        Source::RunOnceMachine32,
    ] {
        if let Some((hive, run, approved)) = reg_source(s) {
            let approved_path = approved.map(|a| format!(r"{APPROVED_BASE}\{a}"));
            if let Ok(root) = hive.key() {
                out.extend(read_run(&ctx, s, hive, &root, run, approved_path.as_deref()));
            }
        }
    }
    for s in [Source::StartupFolderUser, Source::StartupFolderCommon] {
        out.extend(read_folder(&ctx, &com, s));
    }
    out.extend(read_tasks(&ctx, &com));
    out.extend(read_services(&ctx));
    Ok(out)
}

/// Số app đang bật khởi động cùng Windows (tab "Ứng dụng": Run/Run32 HKCU+HKLM + thư mục Startup).
/// Bản nhẹ cho Smart Diagnosis: chỉ đọc registry/thư mục, KHÔNG giải shortcut và KHÔNG kiểm tra chữ ký.
pub fn enabled_app_count() -> usize {
    let mut n = 0;
    for s in [Source::RunUser, Source::RunUser32, Source::RunMachine, Source::RunMachine32] {
        let Some((hive, run, Some(approved))) = reg_source(s) else { continue };
        let Ok(root) = hive.key() else { continue };
        let Ok(key) = root.open(run) else { continue };
        let Ok(values) = key.values() else { continue };
        let states = read_approved(&root, Some(&format!(r"{APPROVED_BASE}\{approved}")));
        n += values
            .filter(|(name, _)| !name.is_empty())
            .filter(|(name, _)| approved::parse(states.get(&name.to_lowercase()).map(Vec::as_slice)).enabled)
            .count();
    }
    for s in [Source::StartupFolderUser, Source::StartupFolderCommon] {
        let Some((dir, hive)) = folder_source(s) else { continue };
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let states = hive.key().map(|root| read_approved(&root, Some(&format!(r"{APPROVED_BASE}\StartupFolder")))).unwrap_or_default();
        n += rd
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.eq_ignore_ascii_case("desktop.ini"))
            .filter(|name| approved::parse(states.get(&name.to_lowercase()).map(Vec::as_slice)).enabled)
            .count();
    }
    n
}

fn read_approved(root: &Key, path: Option<&str>) -> HashMap<String, Vec<u8>> {
    let Some(key) = path.and_then(|p| root.open(p).ok()) else { return HashMap::new() };
    let Ok(values) = key.values() else { return HashMap::new() };
    values.map(|(name, v)| (name.to_lowercase(), v.to_vec())).collect()
}

fn read_run(
    ctx: &Ctx,
    source: Source,
    hive: Hive,
    root: &Key,
    run_path: &str,
    approved_path: Option<&str>,
) -> Vec<StartupEntry> {
    let Ok(key) = root.open(run_path) else { return Vec::new() };
    let Ok(values) = key.values() else { return Vec::new() };
    let approved = read_approved(root, approved_path);
    values
        .filter(|(name, _)| !name.is_empty())
        .filter_map(|(name, v)| Some((name, String::try_from(v).ok()?)))
        .map(|(name, cmd)| {
            let path = ctx.exe(&cmd);
            let state = approved::parse(approved.get(&name.to_lowercase()).map(Vec::as_slice));
            StartupEntry {
                id: format!("{source:?}:{name}"),
                location: format!(r"{}\{run_path}", hive.prefix()),
                publisher: ctx.publisher(path.as_deref()),
                path: path.map(|p| p.display().to_string()),
                command: cmd,
                source,
                enabled: state.enabled,
                can_toggle: approved_path.is_some(),
                disabled_at_ms: state.disabled_at_ms,
                note: approved_path.is_none().then(|| "Chạy một lần ở lần đăng nhập tới".to_owned()),
                key_name: name.clone(),
                name,
            }
        })
        .collect()
}

fn read_folder(ctx: &Ctx, com: &ComGuard, source: Source) -> Vec<StartupEntry> {
    let Some((dir, hive)) = folder_source(source) else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let approved_path = format!(r"{APPROVED_BASE}\StartupFolder");
    let approved = hive.key().map(|root| read_approved(&root, Some(&approved_path))).unwrap_or_default();
    rd.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let file = e.path();
            let name = file.file_name()?.to_string_lossy().into_owned();
            if name.eq_ignore_ascii_case("desktop.ini") {
                return None;
            }
            let is_lnk = file.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk"));
            let (path, command) = if is_lnk {
                match shell::resolve_lnk(com, &file) {
                    Ok(t) => {
                        let target = t.path.map(|p| PathBuf::from(ctx.expand(&p.to_string_lossy())));
                        let cmd = target.as_ref().map_or_else(String::new, |p| format!("\"{}\" {}", p.display(), t.args));
                        (target, cmd.trim().to_owned())
                    }
                    Err(_) => (None, String::new()),
                }
            } else {
                (Some(file.clone()), file.display().to_string())
            };
            let state = approved::parse(approved.get(&name.to_lowercase()).map(Vec::as_slice));
            Some(StartupEntry {
                id: format!("{source:?}:{name}"),
                name: name.trim_end_matches(".lnk").trim_end_matches(".LNK").to_owned(),
                source,
                location: dir.display().to_string(),
                command,
                publisher: ctx.publisher(path.as_deref()),
                path: path.map(|p| p.display().to_string()),
                enabled: state.enabled,
                can_toggle: true,
                disabled_at_ms: state.disabled_at_ms,
                note: None,
                key_name: name,
            })
        })
        .collect()
}

fn read_tasks(ctx: &Ctx, com: &ComGuard) -> Vec<StartupEntry> {
    let Ok(list) = tasks::logon_tasks(com) else { return Vec::new() };
    list.into_iter()
        .map(|t| {
            let (exe, args) = t.actions.first().cloned().unwrap_or_default();
            let command = format!("{exe} {args}").trim().to_owned();
            let path = ctx.exe(&command);
            StartupEntry {
                id: format!("ScheduledTask:{}", t.path),
                key_name: String::new(),
                name: t.name,
                source: Source::ScheduledTask,
                location: t.path,
                command,
                publisher: ctx.publisher(path.as_deref()),
                path: path.map(|p| p.display().to_string()),
                enabled: t.enabled,
                can_toggle: false,
                disabled_at_ms: None,
                note: (!t.author.is_empty()).then_some(t.author),
            }
        })
        .collect()
}

/// Service auto-start (Start = 2) chạy trong tiến trình user-mode.
fn read_services(ctx: &Ctx) -> Vec<StartupEntry> {
    let Ok(root) = LOCAL_MACHINE.open(SERVICES) else { return Vec::new() };
    let Ok(names) = root.keys() else { return Vec::new() };
    names
        .filter_map(|svc| {
            let key = root.open(&svc).ok()?;
            if key.get_u32("Start").ok()? != 2 {
                return None;
            }
            // SERVICE_WIN32_OWN_PROCESS (0x10) | SERVICE_WIN32_SHARE_PROCESS (0x20); bỏ driver.
            if key.get_u32("Type").ok()? & 0x30 == 0 {
                return None;
            }
            let image = key.get_string("ImagePath").ok()?;
            let command = command::normalize_image_path(&ctx.expand(&image), &ctx.windows_dir);
            // svchost chỉ là vỏ — file thật là ServiceDll.
            let dll = key
                .open("Parameters")
                .and_then(|p| p.get_string("ServiceDll"))
                .ok()
                .map(|d| PathBuf::from(ctx.expand(&d)));
            let path = dll.or_else(|| ctx.exe(&command));
            let display = key.get_string("DisplayName").ok().filter(|d| !d.starts_with('@')).unwrap_or_else(|| svc.clone());
            let delayed = key.get_u32("DelayedAutostart").unwrap_or(0) == 1;
            Some(StartupEntry {
                id: format!("Service:{svc}"),
                key_name: String::new(),
                name: display,
                source: Source::Service,
                location: format!(r"HKLM\{SERVICES}\{svc}"),
                command,
                publisher: ctx.publisher(path.as_deref()),
                path: path.map(|p| p.display().to_string()),
                enabled: true,
                can_toggle: false,
                disabled_at_ms: None,
                note: Some(if delayed { "Tự động (trì hoãn)" } else { "Tự động" }.to_owned()),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Bật / tắt.

/// Bật/tắt một entry. `name` là tên value trong Run, hoặc tên file trong thư mục Startup.
pub fn set_enabled(store: &Store, source: Source, name: &str, enabled: bool) -> AppResult<()> {
    let action = if enabled { "startup.enable" } else { "startup.disable" };
    let target = format!("{source:?}: {name}");
    let result = set_enabled_inner(source, name, enabled);
    let detail = match &result {
        Ok(()) => "ok".to_owned(),
        Err(e) => e.to_string(),
    };
    store.log_action_best_effort(action, &target, result.is_ok(), &detail);
    result
}

fn set_enabled_inner(source: Source, name: &str, enabled: bool) -> AppResult<()> {
    if let Some((hive, run, Some(approved))) = reg_source(source) {
        // Chỉ cho phép tên đang thực sự có trong Run (không ghi tên tuỳ ý từ UI).
        let root = hive.key()?;
        root.open(run)
            .and_then(|k| k.get_value(name))
            .map_err(|_| AppError::NotFound(format!("không có '{name}' trong {}\\{run}", hive.prefix())))?;
        return write_approved(&root, &format!(r"{APPROVED_BASE}\{approved}"), name, enabled);
    }
    if let Some((dir, hive)) = folder_source(source) {
        // Chống path traversal: `name` phải đúng là một tên file nằm trong thư mục.
        if Path::new(name).file_name().and_then(|n| n.to_str()) != Some(name) || !dir.join(name).is_file() {
            return Err(AppError::NotFound(format!("không có '{name}' trong thư mục Startup")));
        }
        return write_approved(&hive.key()?, &format!(r"{APPROVED_BASE}\StartupFolder"), name, enabled);
    }
    Err(AppError::Forbidden("nguồn này chỉ đọc trong v1".into()))
}

/// Ghi giá trị StartupApproved. `create` mở hoặc tạo subkey với quyền ghi
/// (Task Manager cũng tạo key này khi tắt lần đầu).
fn write_approved(root: &Key, approved_path: &str, name: &str, enabled: bool) -> AppResult<()> {
    let key = root.create(approved_path)?;
    key.set_bytes(name, Type::Bytes, &approved::encode(enabled, approved::now_filetime()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_registry::CURRENT_USER;

    /// Registry giả trong HKCU\Software\PcManagerTest-<pid>-<thread> — xoá cả cây khi drop.
    struct Sandbox(String);
    impl Sandbox {
        fn new() -> Self {
            let tid = format!("{:?}", std::thread::current().id()).replace(|c: char| !c.is_ascii_digit(), "");
            Self(format!(r"Software\PcManagerTest-{}-{tid}", std::process::id()))
        }
        fn run(&self) -> String {
            format!(r"{}\Run", self.0)
        }
        fn approved(&self) -> String {
            format!(r"{}\Approved", self.0)
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = CURRENT_USER.remove_tree(&self.0);
        }
    }

    #[test]
    fn toggle_writes_only_startup_approved_like_task_manager() {
        let sb = Sandbox::new();
        let cmd = r#""C:\Windows\System32\notepad.exe" /x"#;
        CURRENT_USER.create(sb.run()).unwrap().set_string("Demo", cmd).unwrap();
        let cache = PublisherCache::default();
        let ctx = Ctx::new(&cache);
        let read = || read_run(&ctx, Source::RunUser, Hive::User, CURRENT_USER, &sb.run(), Some(&sb.approved()));

        let e = &read()[0];
        assert_eq!(e.name, "Demo");
        assert!(e.enabled && e.can_toggle);
        assert_eq!(e.path.as_deref(), Some(r"C:\Windows\System32\notepad.exe"));

        write_approved(CURRENT_USER, &sb.approved(), "Demo", false).unwrap();
        let e = &read()[0];
        assert!(!e.enabled);
        assert!(e.disabled_at_ms.is_some());
        // Giá trị gốc trong Run KHÔNG bị đổi.
        assert_eq!(CURRENT_USER.open(sb.run()).unwrap().get_string("Demo").unwrap(), cmd);
        let raw = CURRENT_USER.open(sb.approved()).unwrap().get_value("Demo").unwrap();
        assert_eq!(raw[0], 0x03);
        assert_eq!(raw.len(), 12);

        write_approved(CURRENT_USER, &sb.approved(), "Demo", true).unwrap();
        assert!(read()[0].enabled);
    }

    #[test]
    fn run_once_is_read_only() {
        let sb = Sandbox::new();
        CURRENT_USER.create(sb.run()).unwrap().set_string("Once", "cmd.exe /c echo").unwrap();
        let cache = PublisherCache::default();
        let ctx = Ctx::new(&cache);
        let list = read_run(&ctx, Source::RunOnceUser, Hive::User, CURRENT_USER, &sb.run(), None);
        assert!(!list[0].can_toggle);
        assert!(list[0].note.is_some());
    }

    #[test]
    fn refuses_unknown_names_readonly_sources_and_path_traversal() {
        let store = Store::in_memory().unwrap();
        let name = "pcm-khong-ton-tai-\u{1F600}";
        assert_eq!(set_enabled(&store, Source::RunUser, name, false).unwrap_err().code(), "not_found");
        assert_eq!(set_enabled(&store, Source::Service, "Spooler", false).unwrap_err().code(), "forbidden");
        assert_eq!(set_enabled(&store, Source::ScheduledTask, "x", false).unwrap_err().code(), "forbidden");
        for evil in [r"..\..\evil.lnk", r"C:\Windows\x.lnk", "a/b.lnk"] {
            assert_eq!(set_enabled(&store, Source::StartupFolderUser, evil, false).unwrap_err().code(), "not_found");
        }
        // Mọi lần từ chối đều được ghi log.
        assert_eq!(store.recent_actions(20).unwrap().len(), 6);
    }

    #[test]
    fn real_list_works() {
        let cache = PublisherCache::default();
        let list = list(&cache).unwrap();
        // Máy nào cũng có service auto-start.
        assert!(list.iter().any(|e| e.source == Source::Service));
        assert!(list.iter().all(|e| !e.id.is_empty()));
    }
}
