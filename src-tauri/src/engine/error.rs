use serde::Serialize;
use std::fmt;

/// 統一錯誤格式：前端以 `code` 判斷錯誤類型，`message` 直接顯示給使用者。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("INVALID_INPUT", message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", message)
    }
    pub fn insufficient(message: impl Into<String>) -> Self {
        Self::new("INSUFFICIENT_STOCK", message)
    }
    pub fn in_use(message: impl Into<String>) -> Self {
        Self::new("IN_USE", message)
    }
    pub fn corrupt(message: impl Into<String>) -> Self {
        Self::new("FILE_CORRUPT", message)
    }
    pub fn locked(message: impl Into<String>) -> Self {
        Self::new("FILE_LOCKED", message)
    }
    pub fn io(context: &str, err: &std::io::Error) -> Self {
        if is_lock_error(err) {
            return Self::locked(format!(
                "{context}：檔案正被其他程式使用（例如 Excel 或防毒軟體），請關閉後再試一次"
            ));
        }
        Self::new("IO_ERROR", format!("{context}：{err}"))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

/// Windows 的 ERROR_SHARING_VIOLATION (32) / ERROR_LOCK_VIOLATION (33)，
/// 以及權限被拒（檔案被鎖定時常見）都視為「檔案被佔用」。
pub fn is_lock_error(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(32) | Some(33)) || err.kind() == std::io::ErrorKind::PermissionDenied
}
