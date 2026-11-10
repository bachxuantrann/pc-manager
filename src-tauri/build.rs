fn main() {
    // Nhúng manifest: requireAdministrator + Common-Controls v6.
    // Chỉ áp dụng cho bản release; ở dev, `tauri dev` chạy từ terminal admin
    // (nếu nhúng requireAdministrator vào bản dev, cargo không spawn được exe
    // từ terminal thường => lỗi 740). Xem CLAUDE.md.
    let mut windows = tauri_build::WindowsAttributes::new();
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        windows = windows.app_manifest(include_str!("app.manifest"));
    } else {
        windows = windows.app_manifest(include_str!("app.dev.manifest"));
    }
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=app.dev.manifest");
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
