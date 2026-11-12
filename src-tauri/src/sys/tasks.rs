//! Liệt kê scheduled task qua COM `ITaskService` (task logon, hoặc mọi task).
//! Chỉ đọc (v1): không bật/tắt/xoá task.

use windows::core::{Interface, BSTR};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::TaskScheduler::{
    IExecAction, IRegisteredTask, ITaskFolder, ITaskService, TaskScheduler, TASK_ACTION_EXEC,
    TASK_ACTION_TYPE, TASK_ENUM_HIDDEN, TASK_TRIGGER_LOGON, TASK_TRIGGER_TYPE2,
};
use windows::Win32::System::Variant::VARIANT;

use super::com::ComGuard;

#[derive(Debug, Clone)]
pub struct LogonTask {
    /// Đường dẫn đầy đủ trong Task Scheduler, vd `\Microsoft\OneDrive\...`.
    pub path: String,
    pub name: String,
    pub enabled: bool,
    pub author: String,
    /// (exe, tham số) của mỗi action kiểu Exec.
    pub actions: Vec<(String, String)>,
}

/// Duyệt đệ quy toàn bộ thư mục task. Task không đọc được (thiếu quyền) bị bỏ qua.
pub fn logon_tasks(com: &ComGuard) -> windows::core::Result<Vec<LogonTask>> {
    list(com, true)
}

/// Mọi task (mọi loại trigger) — dùng để tìm task trỏ vào thư mục của một app.
pub fn all_tasks(com: &ComGuard) -> windows::core::Result<Vec<LogonTask>> {
    list(com, false)
}

/// Có task tên `name` ở thư mục gốc của Task Scheduler (thay `schtasks /Query`: không sinh tiến trình).
/// Không đọc được (thiếu quyền, dịch vụ lỗi) => false, như `schtasks` báo lỗi.
pub fn task_exists(_com: &ComGuard, name: &str) -> bool {
    let found = || -> windows::core::Result<()> {
        // SAFETY: COM đã init (bằng chứng `_com`); CLSID/IID khớp.
        let svc: ITaskService = unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)? };
        let empty = VARIANT::default();
        // SAFETY: VARIANT rỗng = máy local, user hiện tại; BSTR hợp lệ sống tới hết lời gọi.
        unsafe {
            svc.Connect(&empty, &empty, &empty, &empty)?;
            let root = svc.GetFolder(&BSTR::from("\\"))?;
            root.GetTask(&BSTR::from(name))?;
        }
        Ok(())
    };
    found().is_ok()
}

fn list(_com: &ComGuard, logon_only: bool) -> windows::core::Result<Vec<LogonTask>> {
    // SAFETY: COM đã init (bằng chứng `_com`); CLSID/IID khớp.
    let svc: ITaskService = unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)? };
    let empty = VARIANT::default();
    // SAFETY: VARIANT rỗng = máy local, user hiện tại.
    unsafe { svc.Connect(&empty, &empty, &empty, &empty)? };
    // SAFETY: BSTR hợp lệ sống tới hết lời gọi.
    let root = unsafe { svc.GetFolder(&BSTR::from("\\"))? };
    let mut out = Vec::new();
    walk(&root, &mut out, 0, logon_only);
    Ok(out)
}

fn walk(folder: &ITaskFolder, out: &mut Vec<LogonTask>, depth: u32, logon_only: bool) {
    if depth > 16 {
        return; // phòng vòng lặp bất thường
    }
    // SAFETY: các lời gọi COM trên interface hợp lệ; lỗi được bỏ qua từng mục.
    unsafe {
        if let Ok(tasks) = folder.GetTasks(TASK_ENUM_HIDDEN.0) {
            let n = tasks.Count().unwrap_or(0);
            for i in 1..=n {
                // Collection của Task Scheduler đánh số từ 1.
                if let Ok(t) = tasks.get_Item(&VARIANT::from(i)) {
                    if let Some(task) = read_task(&t, logon_only) {
                        out.push(task);
                    }
                }
            }
        }
        if let Ok(subs) = folder.GetFolders(0) {
            let n = subs.Count().unwrap_or(0);
            for i in 1..=n {
                if let Ok(f) = subs.get_Item(&VARIANT::from(i)) {
                    walk(&f, out, depth + 1, logon_only);
                }
            }
        }
    }
}

/// `None` nếu không đọc được, hoặc `logon_only` mà task không có trigger logon.
fn read_task(t: &IRegisteredTask, logon_only: bool) -> Option<LogonTask> {
    // SAFETY: lời gọi COM trên interface hợp lệ; out-pointer trỏ tới biến stack.
    unsafe {
        let def = t.Definition().ok()?;
        let triggers = def.Triggers().ok()?;
        let mut count = 0i32;
        triggers.Count(&mut count).ok()?;
        let has_logon = (1..=count).any(|i| {
            triggers.get_Item(i).is_ok_and(|tr| {
                let mut ty = TASK_TRIGGER_TYPE2::default();
                tr.Type(&mut ty).is_ok() && ty == TASK_TRIGGER_LOGON
            })
        });
        if logon_only && !has_logon {
            return None;
        }

        let mut actions = Vec::new();
        if let Ok(coll) = def.Actions() {
            let mut n = 0i32;
            let _ = coll.Count(&mut n);
            for i in 1..=n {
                let Ok(a) = coll.get_Item(i) else { continue };
                let mut ty = TASK_ACTION_TYPE::default();
                if a.Type(&mut ty).is_err() || ty != TASK_ACTION_EXEC {
                    continue;
                }
                let Ok(exec) = a.cast::<IExecAction>() else { continue };
                let (mut path, mut args) = (BSTR::new(), BSTR::new());
                let _ = exec.Path(&mut path);
                let _ = exec.Arguments(&mut args);
                actions.push((path.to_string(), args.to_string()));
            }
        }

        let mut author = BSTR::new();
        if let Ok(info) = def.RegistrationInfo() {
            let _ = info.Author(&mut author);
        }

        Some(LogonTask {
            path: t.Path().map(|b| b.to_string()).unwrap_or_default(),
            name: t.Name().map(|b| b.to_string()).unwrap_or_default(),
            enabled: t.Enabled().is_ok_and(|v| v.as_bool()),
            author: author.to_string(),
            actions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_task_does_not_exist() {
        let com = ComGuard::init().unwrap();
        assert!(!task_exists(&com, "PC Manager khong co task nay 7f3a"));
    }

    #[test]
    fn enumerates_without_error() {
        let com = ComGuard::init().unwrap();
        let tasks = logon_tasks(&com).unwrap();
        // Không chắc máy nào cũng có task logon, chỉ kiểm tra dữ liệu hợp lệ.
        for t in &tasks {
            assert!(t.path.starts_with('\\'), "{t:?}");
        }
        assert!(all_tasks(&com).unwrap().len() >= tasks.len());
    }
}
