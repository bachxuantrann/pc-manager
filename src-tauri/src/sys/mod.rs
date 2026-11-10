//! Toàn bộ code `unsafe` / windows-rs nằm trong module này và được bọc
//! thành hàm safe. Ngoài `sys`, crate bật `#![deny(unsafe_code)]`.
#![allow(unsafe_code)]

pub mod handle;
pub mod memory;
pub mod token;

pub use handle::OwnedHandle;
