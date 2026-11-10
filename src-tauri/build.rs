fn main() {
    // Nhúng manifest: asInvoker + Common-Controls v6 (thiếu Common-Controls thì
    // dialog/message box lỗi). App chạy quyền thường; chức năng cần admin thì
    // người dùng bấm "Chạy với quyền Administrator" (ShellExecute "runas"). Xem CLAUDE.md.
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    println!("cargo:rerun-if-changed=app.manifest");
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
