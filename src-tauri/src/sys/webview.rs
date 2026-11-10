//! WebView2: đặt mức dùng bộ nhớ của giao diện (`ICoreWebView2_19::SetMemoryUsageTargetLevel`).
//!
//! Mức `Low` bảo WebView2 nhả bộ nhớ không cần thiết (cache, trang bộ nhớ ít dùng) —
//! dùng khi cửa sổ không được tương tác. Đặt lại `Normal` khi người dùng quay lại.

use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Controller, ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
};
use windows_core_062::Interface;

/// `low = true`: nhả bộ nhớ (cửa sổ không được dùng). Runtime WebView2 cũ không có
/// ICoreWebView2_19 => trả lỗi, người gọi bỏ qua.
pub fn set_memory_low(controller: &ICoreWebView2Controller, low: bool) -> windows_core_062::Result<()> {
    // SAFETY: `controller` là controller hợp lệ do Tauri giữ, gọi trên main thread
    // (bên trong `with_webview`); CoreWebView2 trả interface có đếm tham chiếu.
    let webview = unsafe { controller.CoreWebView2()? };
    let webview: ICoreWebView2_19 = webview.cast()?;
    let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
    // SAFETY: interface hợp lệ vừa lấy ở trên; giá trị là hằng của enum.
    unsafe { webview.SetMemoryUsageTargetLevel(level) }
}
