//! Toàn bộ code `unsafe` / windows-rs nằm trong module này và được bọc
//! thành hàm safe. Ngoài `sys`, crate bật `#![deny(unsafe_code)]`.
#![allow(unsafe_code)]

pub mod com;
pub mod handle;
pub mod memory;
mod ntdll;
pub mod process;
pub mod shell;
pub mod signature;
pub mod tasks;
pub mod token;

pub use handle::OwnedHandle;
