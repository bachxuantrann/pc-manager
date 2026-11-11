//! Toàn bộ code `unsafe` / windows-rs nằm trong module này và được bọc
//! thành hàm safe. Ngoài `sys`, crate bật `#![deny(unsafe_code)]`.
#![allow(unsafe_code)]

pub mod com;
pub mod disk;
pub mod firmware;
pub mod handle;
pub mod instance;
pub mod launch;
pub mod memory;
mod ntdll;
pub mod process;
pub mod recycle;
pub mod service;
pub mod session;
pub mod shell;
pub mod signature;
pub mod storage;
pub mod tasks;
pub mod webview;
pub mod token;

pub use handle::OwnedHandle;
