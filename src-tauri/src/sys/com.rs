//! Khởi tạo COM theo RAII cho thread hiện tại.

use std::marker::PhantomData;

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

/// Gọi `CoUninitialize` khi drop — chỉ khi chính ta đã init thành công.
///
/// `PhantomData<*const ()>` làm struct không `Send`/`Sync`: COM init gắn với
/// thread, drop phải xảy ra trên cùng thread đã init.
pub struct ComGuard {
    owns: bool,
    _not_send: PhantomData<*const ()>,
}

impl ComGuard {
    /// Init MTA. Nếu thread đã init ở chế độ khác (RPC_E_CHANGED_MODE) thì vẫn
    /// dùng được COM, chỉ là không được uninit.
    pub fn init() -> windows::core::Result<Self> {
        // SAFETY: CoInitializeEx không nhận con trỏ nào ngoài reserved = None.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            return Ok(Self { owns: false, _not_send: PhantomData });
        }
        hr.ok()?;
        // S_OK hoặc S_FALSE đều phải cân bằng bằng một CoUninitialize.
        Ok(Self { owns: true, _not_send: PhantomData })
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.owns {
            // SAFETY: cân bằng đúng một lần lời gọi CoInitializeEx thành công
            // trên cùng thread (ComGuard không Send nên không thể drop ở thread khác).
            unsafe { CoUninitialize() };
        }
    }
}
