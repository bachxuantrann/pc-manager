//! Lỗi chung của app. Trả về UI dạng `{ code, message }`.

use serde::{ser::SerializeStruct, Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Lỗi Win32/NT, giữ lại HRESULT để debug.
    #[error("Windows API lỗi: {0}")]
    Win32(#[from] windows::core::Error),

    #[error("Tham số không hợp lệ: {0}")]
    InvalidInput(String),

    #[error("Không được phép: {0}")]
    Forbidden(String),

    #[error("Không tìm thấy: {0}")]
    NotFound(String),

    #[error("Đã huỷ")]
    Cancelled,

    #[error("Đang bận: {0}")]
    Busy(String),

    #[error("Lỗi cơ sở dữ liệu: {0}")]
    Db(#[from] rusqlite::Error),

    /// Lỗi nội bộ (tác vụ panic). Chi tiết đã nằm trong crash.log.
    #[error("Lỗi nội bộ khi {0}. Chi tiết đã được ghi vào crash.log — hãy thử lại.")]
    Internal(String),

    #[error("{0}")]
    Other(String),
}

/// Tác vụ nền (spawn_blocking) kết thúc bất thường — thường là panic, đã được panic hook ghi lại.
pub fn task_failed(what: &str, e: impl std::fmt::Display) -> AppError {
    eprintln!("[{what}] tác vụ lỗi: {e}");
    AppError::Internal(what.to_owned())
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Win32(_) => "win32",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::Forbidden(_) => "forbidden",
            AppError::NotFound(_) => "not_found",
            AppError::Cancelled => "cancelled",
            AppError::Busy(_) => "busy",
            AppError::Db(_) => "db",
            AppError::Internal(_) => "internal",
            AppError::Other(_) => "other",
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Other(format!("{e:#}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_code_and_message() {
        let e = AppError::Forbidden("csrss.exe".into());
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["code"], "forbidden");
        assert_eq!(v["message"], "Không được phép: csrss.exe");
    }

    /// Đúng đường đi của lỗi thật (trang Ứng dụng): panic trong pool rayon, chạy bên trong
    /// spawn_blocking của command. Với panic = "unwind", command phải trả lỗi "internal"
    /// cho giao diện và app chạy tiếp — không sập cả tiến trình.
    #[test]
    fn panic_in_background_task_becomes_error_and_app_survives() {
        let result: AppResult<u32> = tauri::async_runtime::block_on(async {
            tauri::async_runtime::spawn_blocking(|| -> AppResult<u32> {
                crate::bg::pool()?.install(|| {
                    use rayon::prelude::*;
                    (0..64u32).into_par_iter().map(|i| if i == 37 { panic!("lỗi giả lập ở phần tử {i}") } else { i }).sum::<u32>();
                    Ok(1)
                })
            })
            .await
            .map_err(|e| task_failed("đọc danh sách ứng dụng", e))?
        });
        let err = result.unwrap_err();
        assert_eq!(err.code(), "internal");
        assert!(err.to_string().contains("đọc danh sách ứng dụng") && err.to_string().contains("crash.log"));

        // Pool nền và các tác vụ sau vẫn chạy bình thường.
        let ok: AppResult<u32> = tauri::async_runtime::block_on(async {
            tauri::async_runtime::spawn_blocking(|| crate::bg::pool().map(|p| p.install(|| 41 + 1)))
                .await
                .map_err(|e| task_failed("x", e))?
        });
        assert_eq!(ok.unwrap(), 42);
    }
}
