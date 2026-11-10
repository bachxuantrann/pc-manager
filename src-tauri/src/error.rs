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

    #[error("{0}")]
    Other(String),
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
}
