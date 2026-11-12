//! "Dọn phần còn sót" sau khi gỡ ứng dụng — ngoại lệ của CLAUDE.md AN TOÀN #1.
//!
//! - Backend tự tìm ứng viên; UI chỉ gửi `scanId` + id mục đã chọn.
//! - Mọi file/thư mục chỉ được chuyển vào Thùng rác (khôi phục được).
//! - Key registry được export `.reg` trước khi xoá.
//! - Mỗi đường dẫn qua `Guard` lúc quét VÀ ngay trước khi dọn.

use std::collections::{HashMap, HashSet};
use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::installed::{self, Installed};
use super::{dir_size, parse_uninstall, usable_location};
use crate::cleaner::safety::{is_link_or_cloud, norm, BanCtx};
use crate::error::{AppError, AppResult};
use crate::store::Store;
use crate::sync::LockExt;
use crate::sys::com::ComGuard;
use crate::sys::{process, shell};

const SCAN_TTL: Duration = Duration::from_secs(30 * 60);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LeftoverKind {
    InstallDir,
    DataDir,
    Shortcut,
    Registry,
}

/// Strong: chắc chắn thuộc app (mặc định tick). Possible: khớp tên gần đúng (mặc định không tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    Strong,
    Possible,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub id: u32,
    pub kind: LeftoverKind,
    pub path: String,
    pub bytes: Option<u64>,
    pub confidence: Confidence,
    pub reason: String,
    /// Lý do không thể dọn (vd. đang có tiến trình chạy) — UI hiện nhưng không cho chọn.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverScan {
    pub scan_id: u64,
    pub app_name: String,
    /// Trình gỡ của app vẫn còn trên đĩa → nên thử nó trước.
    pub uninstaller_exists: bool,
    pub items: Vec<Leftover>,
    /// Service/driver/task trỏ vào các thư mục trên — nên dùng trình gỡ của hãng.
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverOutcome {
    pub id: u32,
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverProgress {
    /// Số mục đã xử lý / tổng số mục được chọn.
    pub done: u32,
    pub total: u32,
    /// Mục đang xử lý (đường dẫn hoặc key registry).
    pub current: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverReport {
    pub recycled: u32,
    pub recycled_bytes: u64,
    pub registry_removed: bool,
    /// File `.reg` sao lưu key Uninstall (nếu đã xoá key).
    pub backup: Option<String>,
    pub failed: u32,
    pub items: Vec<LeftoverOutcome>,
}

// ---- So khớp tên -------------------------------------------------------------

/// Token chỉ phiên bản / kiến trúc — bỏ khi so tên ("NetBeans-25" ≈ "NetBeans").
fn is_version_token(t: &str) -> bool {
    const ARCH: &[&str] = &["x64", "x86", "amd64", "arm64", "win64", "win32", "64bit", "32bit", "bit"];
    t.bytes().all(|b| b.is_ascii_digit())
        || (t.len() > 1 && t.starts_with('v') && t[1..].bytes().all(|b| b.is_ascii_digit()))
        || ARCH.contains(&t)
}

fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|t| !t.is_empty() && !is_version_token(t))
        .collect()
}

/// Tên rút gọn để so: chữ thường, bỏ ký tự đặc biệt và số phiên bản.
fn base(s: &str) -> String {
    tokens(s).concat()
}

/// Từ quá chung để một mình nó đại diện cho app.
const GENERIC_WORDS: &[&str] = &[
    "apache", "microsoft", "software", "foundation", "inc", "llc", "ltd", "corp", "corporation", "company", "the",
    "and", "for", "app", "apps", "application", "ide", "tool", "tools", "studio", "edition", "community", "professional",
    "free", "update", "updater", "setup", "installer", "windows", "program", "programs", "files", "data", "common",
    "team", "project", "group", "technologies", "systems", "client", "desktop", "beta", "preview", "runtime", "service",
    "driver", "drivers", "plugin", "manager", "launcher", "helper", "user", "users", "local", "cache", "config",
    "settings", "google", "mozilla", "adobe", "oracle", "intel", "nvidia", "amd", "realtek", "jetbrains", "java",
];

/// Thư mục dùng chung — không bao giờ là ứng viên dù tên khớp.
const SHARED_DIRS: &[&str] = &[
    "microsoft", "windows", "packages", "temp", "tmp", "programs", "common files", "windowsapps", "windows defender",
    "windows nt", "internet explorer", "microsoft.net", "dotnet", "package cache", "microsoft shared", "windows kits",
    "reference assemblies", "msbuild", "crashdumps", "d3dscache", "virtualstore", "connecteddevicesplatform",
    "comms", "publishers", "google", "mozilla", "adobe", "oracle", "intel", "nvidia corporation", "nvidia", "amd",
    "jetbrains", "java", ".ssh", ".config", ".cache", ".local", ".gnupg", ".aws", ".azure", ".kube", ".nbi", ".m2",
    ".gradle", ".npm", ".nuget", ".dotnet", ".android",
];

#[derive(Debug, Default)]
struct Names {
    /// Tên rút gọn chắc chắn của app (tên hiển thị, tên thư mục cài).
    strong: HashSet<String>,
    /// Từ đặc trưng (≥ 4 ký tự, không phải từ chung).
    words: HashSet<String>,
    publisher: Option<String>,
}

impl Names {
    fn new(name: &str, install_dir_name: Option<&str>, publisher: Option<&str>) -> Self {
        let mut strong = HashSet::new();
        for s in std::iter::once(name).chain(install_dir_name) {
            let b = base(s);
            if b.len() >= 3 {
                strong.insert(b);
            }
        }
        let words = tokens(name)
            .into_iter()
            .chain(install_dir_name.map(tokens).unwrap_or_default())
            .filter(|t| t.chars().count() >= 4 && !GENERIC_WORDS.contains(&t.as_str()))
            .collect();
        let publisher = publisher.map(base).filter(|p| p.len() >= 3 && !GENERIC_WORDS.contains(&p.as_str()));
        Self { strong, words, publisher }
    }

    fn classify(&self, dir_name: &str) -> Option<Confidence> {
        let b = base(dir_name);
        if b.is_empty() {
            None
        } else if self.strong.contains(&b) {
            Some(Confidence::Strong)
        } else if self.words.contains(&b) {
            Some(Confidence::Possible)
        } else {
            None
        }
    }
}

// ---- Lớp an toàn ---------------------------------------------------------------

fn inside(path: &str, base: &str) -> bool {
    path == base || path.strip_prefix(base).is_some_and(|r| r.starts_with('\\'))
}

/// Kiểm tra độc lập cho mọi ứng viên (đường dẫn đã chuẩn hoá bằng `norm`).
#[derive(Debug, Default)]
struct Guard {
    /// Không được nằm trong (Windows, thư mục dữ liệu cá nhân).
    forbidden_under: Vec<String>,
    /// Không được là chính nó hay tổ tiên của (gốc tìm kiếm, hồ sơ người dùng…).
    protected: Vec<String>,
    user_profile: String,
    users_root: String,
    /// Thư mục cài của các app KHÁC.
    other_apps: Vec<String>,
    /// Thư mục chứa exe của tiến trình đang chạy (gồm cả PC Manager).
    running: Vec<String>,
}

impl Guard {
    fn check(&self, path: &Path) -> Option<&'static str> {
        let mut normals = 0usize;
        for c in path.components() {
            match c {
                Component::Prefix(p) if !matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) => {
                    return Some("ổ mạng / đường dẫn đặc biệt")
                }
                Component::CurDir | Component::ParentDir => return Some("đường dẫn chứa '.' hoặc '..'"),
                Component::Normal(_) => normals += 1,
                _ => {}
            }
        }
        if !path.is_absolute() || normals < 2 {
            return Some("gốc ổ đĩa hoặc đường dẫn tương đối");
        }
        let s = norm(&path.to_string_lossy());
        let name = s.rsplit('\\').next().unwrap_or_default();
        if crate::settings::is_protected(&s) {
            return Some("nằm trong danh sách bảo vệ của bạn");
        }
        if SHARED_DIRS.contains(&name) {
            return Some("thư mục dùng chung");
        }
        if self.forbidden_under.iter().any(|f| inside(&s, f)) {
            return Some("thư mục hệ thống hoặc dữ liệu cá nhân");
        }
        if self.protected.iter().any(|p| inside(p, &s)) {
            return Some("thư mục dùng chung");
        }
        let parent = path.parent().map(|p| norm(&p.to_string_lossy())).unwrap_or_default();
        if parent == self.users_root || (parent == self.user_profile && !name.starts_with('.')) {
            return Some("gốc thư mục người dùng");
        }
        if self.other_apps.iter().any(|o| inside(o, &s) || inside(&s, o)) {
            return Some("liên quan tới thư mục của ứng dụng khác");
        }
        None
    }

    fn running_inside(&self, path: &Path) -> bool {
        let s = norm(&path.to_string_lossy());
        self.running.iter().any(|r| inside(r, &s))
    }
}

/// Các thư mục gốc để tìm phần còn sót.
#[derive(Debug, Default)]
struct Roots {
    /// Tìm con cấp 1 (và cấp 2 dưới thư mục nhà phát hành).
    data: Vec<PathBuf>,
    /// Hồ sơ người dùng: chỉ thư mục chấm (`.netbeans`), luôn là "có thể".
    profile: Option<PathBuf>,
    start_menu: Vec<PathBuf>,
}

impl Roots {
    fn from_env() -> Self {
        let env = |k: &str| crate::user::env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        let mut data = Vec::new();
        let mut start_menu = Vec::new();
        if let Some(a) = env("APPDATA") {
            start_menu.push(a.join(r"Microsoft\Windows\Start Menu\Programs"));
            data.push(a);
        }
        if let Some(l) = env("LOCALAPPDATA") {
            data.push(l.join("Programs"));
            if let Some(low) = l.parent().map(|p| p.join("LocalLow")) {
                data.push(low);
            }
            data.push(l);
        }
        if let Some(pd) = env("ProgramData") {
            start_menu.push(pd.join(r"Microsoft\Windows\Start Menu\Programs"));
            data.push(pd);
        }
        for k in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Some(p) = env(k) {
                if !data.contains(&p) {
                    data.push(p);
                }
            }
        }
        Self { data, profile: env("USERPROFILE"), start_menu }
    }

    fn all(&self) -> impl Iterator<Item = &PathBuf> {
        self.data.iter().chain(&self.start_menu).chain(&self.profile)
    }
}

fn running_dirs() -> Vec<String> {
    let mut out: Vec<String> = process::enumerate()
        .unwrap_or_default()
        .iter()
        .filter_map(|p| process::image_path(p.pid))
        .filter_map(|p| p.parent().map(|d| norm(&d.to_string_lossy())))
        .collect();
    if let Some(d) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| norm(&d.to_string_lossy()))) {
        out.push(d);
    }
    out.sort();
    out.dedup();
    out
}

fn build_guard(app_id: &str, own_location: Option<&Path>, roots: &Roots) -> Guard {
    let ctx = BanCtx::from_env();
    let mut forbidden_under = vec![norm(&ctx.windows)];
    forbidden_under.extend(ctx.user_folders.iter().filter(|f| !f.is_empty()).map(|f| norm(f)));
    let mut protected: Vec<String> = roots.all().map(|r| norm(&r.to_string_lossy())).collect();
    protected.extend(forbidden_under.iter().cloned());
    for p in [&ctx.user_profile, &ctx.users_root, &ctx.program_data] {
        if !p.is_empty() {
            protected.push(norm(p));
        }
    }
    protected.extend(ctx.program_files.iter().filter(|p| !p.is_empty()).map(|p| norm(p)));
    let own = own_location.map(|l| norm(&l.to_string_lossy()));
    let other_apps = installed::list()
        .into_iter()
        .filter(|a| a.id != app_id)
        .filter_map(|a| a.install_location)
        .filter(|l| usable_location(l))
        .map(|l| norm(&l.to_string_lossy()))
        // Bỏ vị trí "rộng" (là gốc tìm kiếm hoặc tổ tiên của nó) và trùng thư mục của chính app.
        .filter(|l| !protected.iter().any(|p| inside(p, l)) && Some(l) != own.as_ref())
        .collect();
    Guard {
        forbidden_under,
        protected,
        user_profile: norm(&ctx.user_profile),
        users_root: norm(&ctx.users_root),
        other_apps,
        running: running_dirs(),
    }
}

// ---- Quét ------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Item {
    info: Leftover,
    /// Đường dẫn thật (file/thư mục); None với key registry.
    path: Option<PathBuf>,
}

struct Collector<'a> {
    guard: &'a Guard,
    items: Vec<Item>,
    seen: HashSet<String>,
    deadline: Instant,
}

impl Collector<'_> {
    fn add(&mut self, path: PathBuf, kind: LeftoverKind, confidence: Confidence, reason: String) {
        let key = norm(&path.to_string_lossy());
        // Bỏ trùng và mục nằm trong mục đã có (vd. data dir ở trong thư mục cài).
        if self.seen.iter().any(|s| inside(&key, s)) {
            return;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else { return };
        if is_link_or_cloud(&meta) || self.guard.check(&path).is_some() {
            return;
        }
        let blocked = (meta.is_dir() && self.guard.running_inside(&path)).then(|| "có chương trình đang chạy từ thư mục này".to_owned());
        let bytes = if meta.is_dir() { dir_size(&path, self.deadline) } else { Some(meta.len()) };
        self.seen.retain(|s| !inside(s, &key));
        self.items.retain(|i| i.path.as_ref().is_none_or(|p| !inside(&norm(&p.to_string_lossy()), &key)));
        self.seen.insert(key);
        let id = self.items.len() as u32;
        self.items.push(Item {
            info: Leftover { id, kind, path: path.display().to_string(), bytes, confidence, reason, blocked },
            path: Some(path),
        });
    }
}

fn child_dirs(dir: &Path) -> Vec<(PathBuf, String)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    rd.flatten()
        .filter(|e| e.metadata().is_ok_and(|m| m.is_dir() && !is_link_or_cloud(&m)))
        .map(|e| (e.path(), e.file_name().to_string_lossy().into_owned()))
        .collect()
}

/// Thư mục cài: InstallLocation; thiếu thì thư mục chứa trình gỡ (chỉ khi tên khớp app).
fn install_dir(app: &Installed, names: &Names) -> Option<(PathBuf, Confidence)> {
    if let Some(loc) = app.install_location.as_deref().filter(|l| usable_location(l)) {
        return Some((loc.to_path_buf(), Confidence::Strong));
    }
    let sys32 = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32");
    let cmd = parse_uninstall(app.uninstall.as_deref()?, &sys32)?;
    let dir = cmd.program.parent()?.to_path_buf();
    let conf = names.classify(&dir.file_name()?.to_string_lossy())?;
    usable_location(&dir).then_some((dir, conf))
}

fn uninstaller_exists(app: &Installed) -> bool {
    let sys32 = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32");
    app.uninstall.as_deref().and_then(|u| parse_uninstall(u, &sys32)).is_some_and(|c| c.program.is_file())
}

fn scan_data_dirs(c: &mut Collector<'_>, roots: &Roots, names: &Names) {
    for root in &roots.data {
        for (dir, name) in child_dirs(root) {
            if let Some(conf) = names.classify(&name) {
                c.add(dir, LeftoverKind::DataDir, conf, format!("tên thư mục khớp ứng dụng ({})", root.display()));
            } else if names.publisher.as_deref() == Some(base(&name).as_str()) {
                // <gốc>\<Nhà phát hành>\<App>: chỉ lấy thư mục con khớp app, không lấy thư mục nhà phát hành.
                for (sub, sub_name) in child_dirs(&dir) {
                    if let Some(conf) = names.classify(&sub_name) {
                        c.add(sub, LeftoverKind::DataDir, conf, format!("thư mục của app trong {name}"));
                    }
                }
            }
        }
    }
    if let Some(profile) = &roots.profile {
        for (dir, name) in child_dirs(profile) {
            if name.starts_with('.') && names.classify(&name).is_some() {
                c.add(dir, LeftoverKind::DataDir, Confidence::Possible, "thư mục cấu hình trong hồ sơ người dùng".into());
            }
        }
    }
}

/// Shortcut Start Menu trỏ vào thư mục cài, và thư mục Start Menu mang tên app.
fn scan_shortcuts(c: &mut Collector<'_>, roots: &Roots, names: &Names, install: Option<&Path>) {
    let Ok(com) = ComGuard::init() else { return };
    let install = install.map(|p| norm(&p.to_string_lossy()));
    let points_into_install = |lnk: &Path| -> bool {
        let Some(target) = shell::resolve_lnk(&com, lnk).ok().and_then(|t| t.path) else { return false };
        install.as_deref().is_some_and(|i| inside(&norm(&target.to_string_lossy()), i))
    };
    let is_lnk = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
    for root in &roots.start_menu {
        let mut stack = vec![(root.clone(), 0u8)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let Ok(m) = e.metadata() else { continue };
                if is_link_or_cloud(&m) {
                    continue;
                }
                let p = e.path();
                let name = e.file_name().to_string_lossy().into_owned();
                if m.is_dir() {
                    let only_links = std::fs::read_dir(&p).is_ok_and(|rd| {
                        rd.flatten().all(|f| {
                            let fp = f.path();
                            let ext = fp.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
                            f.metadata().is_ok_and(|fm| fm.is_file()) && matches!(ext.as_str(), "lnk" | "url" | "ini")
                        })
                    });
                    // Thư mục mà mọi .lnk bên trong đều trỏ vào thư mục cài => của app này.
                    let all_ours = || {
                        let lnks: Vec<PathBuf> = std::fs::read_dir(&p)
                            .map(|rd| rd.flatten().map(|f| f.path()).filter(|f| is_lnk(f)).collect())
                            .unwrap_or_default();
                        !lnks.is_empty() && lnks.iter().all(|l| points_into_install(l))
                    };
                    match names.classify(&name) {
                        Some(conf) if depth == 0 && only_links => {
                            c.add(p, LeftoverKind::Shortcut, conf, "thư mục Start Menu của ứng dụng".into());
                        }
                        None if depth == 0 && only_links && all_ours() => {
                            c.add(p, LeftoverKind::Shortcut, Confidence::Strong, "thư mục Start Menu chỉ chứa shortcut của app".into());
                        }
                        _ if depth < 1 => stack.push((p, depth + 1)),
                        _ => {}
                    }
                } else if is_lnk(&p) && points_into_install(&p) {
                    c.add(p, LeftoverKind::Shortcut, Confidence::Strong, "shortcut trỏ vào thư mục cài".into());
                }
            }
        }
    }
}

// ---- Service / driver / scheduled task dùng thư mục của app ---------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DependencyKind {
    Service,
    Driver,
    Task,
}

/// Thành phần hệ thống trỏ vào thư mục sắp dọn. Xoá file mà để lại chúng sẽ gây
/// lỗi lúc khởi động — nên mục liên quan bị hạ xuống "có thể" và UI cảnh báo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub kind: DependencyKind,
    pub name: String,
    pub command: String,
}

/// (loại, tên, dòng lệnh đã chuẩn hoá) của mọi service, driver và scheduled task.
fn system_commands() -> Vec<(DependencyKind, String, String)> {
    const SERVICES: &str = r"SYSTEM\CurrentControlSet\Services";
    let windir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let expand = |s: &str| crate::startup::command::expand_env(s, crate::user::env);
    let mut out = Vec::new();
    if let Ok(key) = windows_registry::LOCAL_MACHINE.open(SERVICES) {
        for name in key.keys().into_iter().flatten() {
            let Ok(k) = key.open(&name) else { continue };
            let Ok(image) = k.get_string("ImagePath") else { continue };
            // Type: 1 = kernel driver, 2 = file system driver; còn lại là service.
            let kind = if matches!(k.get_u32("Type").unwrap_or(0x10), 1 | 2) { DependencyKind::Driver } else { DependencyKind::Service };
            let display = k.get_string("DisplayName").ok().filter(|d| !d.starts_with('@')).unwrap_or_else(|| name.clone());
            out.push((kind, display, expand(&crate::startup::command::normalize_image_path(&image, &windir))));
        }
    }
    if let Ok(com) = ComGuard::init() {
        for t in crate::sys::tasks::all_tasks(&com).unwrap_or_default() {
            for (exe, args) in &t.actions {
                out.push((DependencyKind::Task, t.path.clone(), expand(&format!("{exe} {args}"))));
            }
        }
    }
    out
}

/// Gắn các thành phần hệ thống vào mục thư mục mà chúng dùng.
fn mark_dependencies(items: &mut [Item], commands: &[(DependencyKind, String, String)]) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for item in items.iter_mut().filter(|i| i.info.kind != LeftoverKind::Registry) {
        let Some(path) = &item.path else { continue };
        let dir = format!("{}\\", norm(&path.to_string_lossy()));
        let hits: Vec<_> = commands.iter().filter(|(_, _, cmd)| cmd.replace('/', "\\").to_lowercase().contains(&dir)).collect();
        if hits.is_empty() {
            continue;
        }
        item.info.confidence = Confidence::Possible;
        let names: Vec<&str> = hits.iter().map(|(_, n, _)| n.as_str()).take(3).collect();
        item.info.reason = format!("{} — đang được dùng bởi {} (service/driver/task)", item.info.reason, names.join(", "));
        for (kind, name, command) in hits {
            if !deps.iter().any(|d: &Dependency| d.name == *name && d.kind == *kind) {
                deps.push(Dependency { kind: *kind, name: name.clone(), command: command.clone() });
            }
        }
    }
    deps
}

fn scan_app(app: &Installed, key_exists: bool) -> (Vec<Item>, bool, Vec<Dependency>) {
    let roots = Roots::from_env();
    let guard = build_guard(&app.id, app.install_location.as_deref(), &roots);
    let install_name = app.install_location.as_deref().and_then(|l| l.file_name()).map(|n| n.to_string_lossy().into_owned());
    let names = Names::new(&app.name, install_name.as_deref(), app.publisher.as_deref());
    let mut c = Collector { guard: &guard, items: Vec::new(), seen: HashSet::new(), deadline: Instant::now() + Duration::from_secs(15) };

    let install = install_dir(app, &names);
    if let Some((dir, conf)) = &install {
        c.add(dir.clone(), LeftoverKind::InstallDir, *conf, "thư mục cài đặt".into());
    }
    scan_data_dirs(&mut c, &roots, &names);
    scan_shortcuts(&mut c, &roots, &names, install.as_ref().map(|(d, _)| d.as_path()));

    let mut items = c.items;
    let dependencies = mark_dependencies(&mut items, &system_commands());
    // Trình gỡ đã xoá key thì không còn gì để dọn ở registry.
    if let Some((_, _, full)) = installed::registry_location(&app.id).filter(|_| key_exists) {
        // App còn service/driver/task hoặc còn thư mục không dọn được => xoá key chỉ làm
        // app "biến mất" khỏi danh sách trong khi vẫn đang cài. Không tick sẵn.
        let risky = !dependencies.is_empty() || items.iter().any(|i| i.info.blocked.is_some());
        items.push(Item {
            info: Leftover {
                id: 0,
                kind: LeftoverKind::Registry,
                path: full,
                bytes: None,
                confidence: if risky { Confidence::Possible } else { Confidence::Strong },
                reason: if risky {
                    "mục trong danh sách ứng dụng đã cài — app vẫn còn thành phần đang dùng, chỉ xoá nếu chắc chắn".into()
                } else {
                    "mục trong danh sách ứng dụng đã cài (sao lưu .reg trước khi xoá)".into()
                },
                blocked: None,
            },
            path: None,
        });
    }
    for (i, it) in items.iter_mut().enumerate() {
        it.info.id = i as u32;
    }
    (items, uninstaller_exists(app), dependencies)
}

// ---- Quản lý phiên quét / dọn ---------------------------------------------------

struct Stored {
    scan_id: u64,
    app_id: String,
    app_name: String,
    install_location: Option<PathBuf>,
    created: Instant,
    items: Vec<Item>,
}

pub struct LeftoverManager {
    backup_dir: PathBuf,
    next_id: AtomicU64,
    stored: Mutex<Option<Stored>>,
    /// Thông tin app chụp ngay trước khi chạy trình gỡ (key registry có thể đã bị xoá).
    snapshots: Mutex<HashMap<String, Installed>>,
}

impl LeftoverManager {
    pub fn new(backup_dir: PathBuf) -> Self {
        Self { backup_dir, next_id: AtomicU64::new(1), stored: Mutex::new(None), snapshots: Mutex::new(HashMap::new()) }
    }

    /// Ghi nhớ app trước khi gỡ. Dữ liệu lấy từ registry phía backend, không từ UI.
    pub fn remember(&self, app: Installed) {
        let mut s = self.snapshots.lock_recover();
        if s.len() >= 32 {
            s.clear();
        }
        s.insert(app.id.clone(), app);
    }

    pub fn scan(&self, app_id: &str) -> AppResult<LeftoverScan> {
        let current = installed::find(app_id);
        let key_exists = current.is_some();
        let app = current
            .or_else(|| self.snapshots.lock_recover().get(app_id).cloned())
            .ok_or_else(|| AppError::NotFound("ứng dụng không còn trong danh sách".into()))?;
        let (items, uninstaller_exists, dependencies) = scan_app(&app, key_exists);
        let scan_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let out = LeftoverScan {
            scan_id,
            app_name: app.name.clone(),
            uninstaller_exists,
            items: items.iter().map(|i| i.info.clone()).collect(),
            dependencies,
        };
        let stored = Stored { scan_id, app_id: app.id, app_name: app.name, install_location: app.install_location, created: Instant::now(), items };
        *self.stored.lock_recover() = Some(stored);
        Ok(out)
    }

    /// Dọn các mục `ids` của lần quét `scan_id` (dùng một lần).
    pub fn clean(&self, store: &Store, scan_id: u64, ids: &[u32], on_progress: &dyn Fn(LeftoverProgress)) -> AppResult<LeftoverReport> {
        let stored = {
            let mut g = self.stored.lock_recover();
            match g.take() {
                Some(s) if s.scan_id == scan_id && s.created.elapsed() < SCAN_TTL => s,
                other => {
                    *g = other;
                    return Err(AppError::InvalidInput("kết quả quét đã hết hạn — hãy quét lại".into()));
                }
            }
        };
        // Dựng lại lớp an toàn: tiến trình / app khác có thể đã thay đổi từ lúc quét.
        let guard = build_guard(&stored.app_id, stored.install_location.as_deref(), &Roots::from_env());
        let mut report = LeftoverReport { recycled: 0, recycled_bytes: 0, registry_removed: false, backup: None, failed: 0, items: Vec::new() };
        let selected: Vec<&Item> = stored.items.iter().filter(|i| ids.contains(&i.info.id)).collect();
        let mut install_failed = false;
        let total = selected.len() as u32;
        let mut done = 0u32;
        let mut step = |current: &str| {
            on_progress(LeftoverProgress { done, total, current: current.to_owned() });
            done += 1;
        };

        for item in selected.iter().filter(|i| i.path.is_some()) {
            let Some(path) = item.path.as_deref() else { continue };
            step(&item.info.path);
            let result = recycle_checked(&guard, item, path);
            let ok = result.is_ok();
            store.log_action_best_effort("app.leftover.recycle", &item.info.path, ok, result.as_ref().err().map_or("", String::as_str));
            if ok {
                report.recycled += 1;
                report.recycled_bytes += item.info.bytes.unwrap_or(0);
            } else {
                report.failed += 1;
                install_failed |= item.info.kind == LeftoverKind::InstallDir;
            }
            report.items.push(LeftoverOutcome { id: item.info.id, ok, message: result.err() });
        }

        if let Some(reg) = selected.iter().find(|i| i.info.kind == LeftoverKind::Registry) {
            step(&reg.info.path);
            let result = if install_failed {
                Err("giữ lại vì thư mục cài chưa dọn được".to_owned())
            } else {
                self.remove_uninstall_key(&stored.app_id, &stored.app_name)
            };
            let ok = result.is_ok();
            let detail = match &result {
                Ok(b) => format!("backup: {}", b.display()),
                Err(e) => e.clone(),
            };
            store.log_action_best_effort("app.leftover.registry", &reg.info.path, ok, &detail);
            match result {
                Ok(b) => {
                    report.registry_removed = true;
                    report.backup = Some(b.display().to_string());
                }
                Err(_) => report.failed += 1,
            }
            report.items.push(LeftoverOutcome { id: reg.info.id, ok, message: (!ok).then_some(detail) });
        }
        Ok(report)
    }

    /// Export key ra `.reg` rồi mới xoá. Key phải vẫn là của đúng app đã quét.
    fn remove_uninstall_key(&self, app_id: &str, app_name: &str) -> Result<PathBuf, String> {
        let current = installed::find(app_id).ok_or("key không còn tồn tại")?;
        if current.name != app_name {
            return Err("key đã thay đổi từ lúc quét".into());
        }
        let (root, path, full) = installed::registry_location(app_id).ok_or("id không hợp lệ")?;
        std::fs::create_dir_all(&self.backup_dir).map_err(|e| format!("không tạo được thư mục backup: {e}"))?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let safe: String = app_id.chars().map(|ch| if ch.is_alphanumeric() || "-_.{}".contains(ch) { ch } else { '_' }).collect();
        let file = self.backup_dir.join(format!("{safe}-{stamp}.reg"));
        let reg = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join(r"System32\reg.exe");
        let status = std::process::Command::new(reg)
            .arg("export")
            .arg(&full)
            .arg(&file)
            .arg("/y")
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("không chạy được reg.exe: {e}"))?;
        if !status.success() || !file.is_file() {
            return Err("sao lưu .reg thất bại — không xoá key".into());
        }
        root.remove_tree(&path).map_err(|e| format!("không xoá được key: {e}"))?;
        Ok(file)
    }
}

fn recycle_checked(guard: &Guard, item: &Item, path: &Path) -> Result<(), String> {
    if item.info.blocked.is_some() {
        return Err("mục này không được phép dọn".into());
    }
    let meta = std::fs::symlink_metadata(path).map_err(|_| "không còn tồn tại".to_owned())?;
    if is_link_or_cloud(&meta) {
        return Err("là liên kết / file đám mây".into());
    }
    if let Some(why) = guard.check(path) {
        return Err(format!("bị chặn: {why}"));
    }
    if meta.is_dir() && guard.running_inside(path) {
        return Err("có chương trình đang chạy từ thư mục này".into());
    }
    crate::user::recycle::move_to_recycle_bin(path)?;
    if path.exists() {
        return Err("một phần bị khoá, chưa chuyển hết vào Thùng rác".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_tokens_are_dropped() {
        assert_eq!(base("NetBeans-25"), "netbeans");
        assert_eq!(base("Apache NetBeans IDE 25"), "apachenetbeanside");
        assert_eq!(base("7-Zip 24.08 (x64)"), "zip");
        assert_eq!(base("App v2"), "app");
        assert_eq!(base("2024"), "");
    }

    #[test]
    fn netbeans_names() {
        let n = Names::new("Apache NetBeans IDE 25", Some("NetBeans-25"), Some("Apache Software Foundation"));
        assert_eq!(n.classify("NetBeans"), Some(Confidence::Strong)); // = tên thư mục cài bỏ số phiên bản
        assert_eq!(n.classify("NetBeans-25"), Some(Confidence::Strong));
        assert_eq!(n.classify("Apache"), None, "từ chung");
        assert_eq!(n.classify("IDE"), None);
        assert_eq!(n.classify("Microsoft"), None);
        assert_eq!(n.publisher, Some("apachesoftwarefoundation".into()));
    }

    #[test]
    fn distinctive_word_is_only_possible() {
        let n = Names::new("Foxit PDF Reader", None, Some("Foxit Software Inc."));
        assert_eq!(n.classify("Foxit PDF Reader"), Some(Confidence::Strong));
        assert_eq!(n.classify("Foxit"), Some(Confidence::Possible));
        assert_eq!(n.classify("Reader"), Some(Confidence::Possible));
        assert_eq!(n.classify("PDF"), None, "quá ngắn");
        assert_eq!(n.publisher, Some("foxitsoftwareinc".into()));
    }

    fn guard() -> Guard {
        let n = |s: &str| norm(s);
        Guard {
            forbidden_under: vec![n(r"C:\Windows"), n(r"C:\Users\An\Documents"), n(r"C:\Users\An\Desktop")],
            protected: vec![
                n(r"C:\Program Files"),
                n(r"C:\ProgramData"),
                n(r"C:\Users\An\AppData\Roaming"),
                n(r"C:\Users\An\AppData\Local"),
                n(r"C:\Users\An\AppData\Local\Programs"),
                n(r"C:\Users\An"),
                n(r"C:\Users"),
                n(r"C:\Windows"),
                n(r"C:\Users\An\Documents"),
            ],
            user_profile: n(r"C:\Users\An"),
            users_root: n(r"C:\Users"),
            other_apps: vec![n(r"C:\Program Files\Vendor\OtherApp")],
            running: vec![n(r"C:\Program Files\Busy\bin")],
        }
    }

    #[test]
    fn guard_allows_app_folders() {
        let g = guard();
        for p in [
            r"C:\Program Files\NetBeans-25",
            r"C:\Users\An\AppData\Roaming\NetBeans",
            r"C:\Users\An\AppData\Local\NetBeans\Cache",
            r"C:\Users\An\.netbeans",
            r"D:\Apps\NetBeans",
            r"C:\Program Files\Vendor\ThisApp",
        ] {
            assert_eq!(g.check(Path::new(p)), None, "phải được phép: {p}");
        }
    }

    #[test]
    fn guard_blocks_shared_system_and_ancestors() {
        let g = guard();
        for p in [
            r"C:\Program Files",                         // gốc tìm kiếm
            r"C:\Users\An\AppData",                      // tổ tiên của gốc
            r"C:\Users\An",                              // hồ sơ
            r"C:\Users\Binh",                            // hồ sơ khác
            r"C:\Users\An\Documents\NetBeansProjects",   // dữ liệu cá nhân
            r"C:\Users\An\Downloads2",                   // con trực tiếp của hồ sơ, không phải thư mục chấm
            r"C:\Windows\Temp\x",
            r"C:\Program Files\Common Files",
            r"C:\Users\An\AppData\Local\Microsoft",
            r"C:\Users\An\.ssh",
            r"C:\Program Files\Vendor",                  // chứa app khác
            r"C:\Program Files\Vendor\OtherApp\plugins", // nằm trong app khác
            r"C:\",
            r"D:\NetBeans",
            r"\\server\share\NetBeans",
            r"C:\Program Files\X\..\..\Windows",
        ] {
            assert!(g.check(Path::new(p)).is_some(), "phải bị chặn: {p}");
        }
    }

    #[test]
    fn running_programs_block_their_folder() {
        let g = guard();
        assert!(g.running_inside(Path::new(r"C:\Program Files\Busy")));
        assert!(g.running_inside(Path::new(r"C:\Program Files\Busy\bin")));
        assert!(!g.running_inside(Path::new(r"C:\Program Files\Busy\bin\plugins")));
        assert!(!g.running_inside(Path::new(r"C:\Program Files\BusyOther")));
    }

    #[test]
    fn collector_dedups_nested_and_skips_links() {
        let root = std::env::temp_dir().join(format!("pcm-leftover-{}", std::process::id()));
        let app = root.join("MyApp");
        std::fs::create_dir_all(app.join("cache")).unwrap();
        std::fs::write(app.join("cache").join("a.bin"), b"12345").unwrap();
        let g = Guard::default();
        let mut c = Collector { guard: &g, items: Vec::new(), seen: HashSet::new(), deadline: Instant::now() + Duration::from_secs(5) };
        c.add(app.join("cache"), LeftoverKind::DataDir, Confidence::Possible, "x".into());
        c.add(app.clone(), LeftoverKind::InstallDir, Confidence::Strong, "x".into());
        c.add(app.join("cache"), LeftoverKind::DataDir, Confidence::Possible, "x".into());
        c.add(root.join("missing"), LeftoverKind::DataDir, Confidence::Strong, "x".into());
        assert_eq!(c.items.len(), 1, "mục con bị thay bằng thư mục cha, mục thiếu bị bỏ");
        assert_eq!(c.items[0].info.kind, LeftoverKind::InstallDir);
        assert_eq!(c.items[0].info.bytes, Some(5));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn services_pointing_into_folder_downgrade_it() {
        let item = |id: u32, p: &str| Item {
            info: Leftover {
                id,
                kind: LeftoverKind::DataDir,
                path: p.into(),
                bytes: None,
                confidence: Confidence::Strong,
                reason: "x".into(),
                blocked: None,
            },
            path: Some(PathBuf::from(p)),
        };
        let mut items = vec![item(0, r"C:\Program Files\VPN"), item(1, r"C:\Program Files\VPNData")];
        let cmds = vec![
            (DependencyKind::Driver, "vpnfilter".to_owned(), r"C:\Program Files\VPN\drv\filter.sys".to_owned()),
            (DependencyKind::Service, "VPN Service".to_owned(), r#""C:/Program Files/VPN/svc.exe" -k"#.to_owned()),
            (DependencyKind::Task, r"\Other".to_owned(), r"C:\Other\x.exe".to_owned()),
        ];
        let deps = mark_dependencies(&mut items, &cmds);
        assert_eq!(deps.len(), 2);
        assert_eq!(items[0].info.confidence, Confidence::Possible);
        assert!(items[0].info.reason.contains("vpnfilter"));
        // "VPNData" không bị nhầm là nằm trong "VPN".
        assert_eq!(items[1].info.confidence, Confidence::Strong);
    }

    /// Chẩn đoán trên máy thật: `cargo test print_leftovers -- --ignored --nocapture`
    /// (đặt PCM_APP=một phần tên app). Chỉ quét, KHÔNG dọn.
    #[test]
    #[ignore = "chỉ để xem dữ liệu thật"]
    fn print_leftovers() {
        let q = std::env::var("PCM_APP").unwrap_or_else(|_| "netbeans".into()).to_lowercase();
        let Some(app) = installed::list().into_iter().find(|a| a.name.to_lowercase().contains(&q)) else {
            println!("không thấy app khớp '{q}'");
            return;
        };
        let (items, unins, deps) = scan_app(&app, true);
        println!("{} (trình gỡ còn: {unins}) phụ thuộc: {deps:?}", app.name);
        for i in items {
            println!("{:?} {:?} {:?} {} — {} {:?}", i.info.kind, i.info.confidence, i.info.bytes, i.info.path, i.info.reason, i.info.blocked);
        }
    }
}
