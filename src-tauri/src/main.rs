// Ẩn cửa sổ console ở bản release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pc_manager_lib::run()
}
