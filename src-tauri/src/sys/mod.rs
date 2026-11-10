//! Toàn bộ code `unsafe` / windows-rs nằm trong module này và được bọc
//! thành hàm safe. Module khác KHÔNG được dùng `unsafe`.

pub mod handle;
pub mod token;

pub use handle::OwnedHandle;
