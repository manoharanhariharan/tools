use serde::{Deserialize, Serialize};

/// Stable machine-readable error codes. The UI switches on these values, so
/// they must never be renamed without a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Io,
    Json,
    Config,
    Crypto,
    EncryptionKey,
    Storage,
    Platform,
    Validation,
    NotFound,
    Conflict,
    Unsupported,
    PermissionDenied,
    Internal,
}

impl ErrorCode {
    /// Whether retrying the same operation can plausibly succeed.
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::Io | Self::Storage | Self::Internal)
    }
}

/// Errors crossing crate boundaries. Every crate returns `AppError`; the Tauri
/// layer converts it into the serialisable [`IpcError`] at the boundary.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("cryptography error: {0}")]
    Crypto(String),

    #[error("encryption key does not match this database: {0}")]
    EncryptionKey(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("platform error: {0}")]
    Platform(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("unsupported operation: {0}")]
    Unsupported(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Io(_) => ErrorCode::Io,
            Self::Json(_) => ErrorCode::Json,
            Self::Config(_) => ErrorCode::Config,
            Self::Crypto(_) => ErrorCode::Crypto,
            Self::EncryptionKey(_) => ErrorCode::EncryptionKey,
            Self::Storage(_) => ErrorCode::Storage,
            Self::Platform(_) => ErrorCode::Platform,
            Self::Validation(_) => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Conflict(_) => ErrorCode::Conflict,
            Self::Unsupported(_) => ErrorCode::Unsupported,
            Self::PermissionDenied(_) => ErrorCode::PermissionDenied,
            Self::Internal(_) => ErrorCode::Internal,
        }
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    pub fn crypto(message: impl Into<String>) -> Self {
        Self::Crypto(message.into())
    }

    pub fn encryption_key(message: impl Into<String>) -> Self {
        Self::EncryptionKey(message.into())
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage(message.into())
    }

    pub fn platform(message: impl Into<String>) -> Self {
        Self::Platform(message.into())
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::PermissionDenied(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

/// Serialisable error envelope sent to the Svelte UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl IpcError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            retryable: code.is_retryable(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code: {:?})", self.message, self.code)
    }
}

impl std::error::Error for IpcError {}

impl From<AppError> for IpcError {
    fn from(error: AppError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

impl From<std::io::Error> for IpcError {
    fn from(error: std::io::Error) -> Self {
        Self::new(ErrorCode::Io, error.to_string())
    }
}

impl From<serde_json::Error> for IpcError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(ErrorCode::Json, error.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_map_consistently() {
        assert_eq!(
            AppError::from(std::io::Error::other("boom")).code(),
            ErrorCode::Io
        );
        assert_eq!(AppError::validation("x").code(), ErrorCode::Validation);
        assert!(ErrorCode::Io.is_retryable());
        assert!(!ErrorCode::Validation.is_retryable());
    }

    #[test]
    fn ipc_error_serialises_with_camel_case_fields() {
        let error: IpcError = AppError::NotFound("server".into()).into();
        let json = serde_json::to_value(&error).expect("serialise");
        assert_eq!(json["code"], "not_found");
        assert_eq!(json["retryable"], false);
        assert_eq!(json["message"], "not found: server");
    }

    #[test]
    fn ipc_error_round_trips() {
        let error = IpcError::new(ErrorCode::Crypto, "bad key");
        let encoded = serde_json::to_string(&error).expect("encode");
        let decoded: IpcError = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, error);
    }
}
