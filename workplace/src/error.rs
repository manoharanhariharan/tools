use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// A single field-level validation problem, surfaced to the client UI.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Debug, Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<FieldError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    existing: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid request: {0}")]
    BadRequest(&'static str),
    #[error("validation failed")]
    Validation(Vec<FieldError>),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("authentication required")]
    Unauthorized,
    #[error("forbidden: {0}")]
    Forbidden(&'static str),
    #[error("conflict: {message}")]
    Conflict {
        code: &'static str,
        message: String,
        existing: Option<serde_json::Value>,
    },
    #[error("too many requests")]
    TooManyRequests,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn internal(err: impl Into<anyhow::Error>) -> Self {
        AppError::Internal(err.into())
    }

    pub fn conflict(
        code: &'static str,
        message: impl Into<String>,
        existing: Option<serde_json::Value>,
    ) -> Self {
        AppError::Conflict {
            code,
            message: message.into(),
            existing,
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("record"),
            other => AppError::Internal(anyhow::Error::new(other)),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::BadRequest("malformed JSON body")
            .tap_log(err)
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(err: argon2::password_hash::Error) -> Self {
        AppError::Internal(anyhow::anyhow!("password hashing failed: {err}"))
    }
}

impl AppError {
    fn tap_log(self, err: impl std::fmt::Display) -> Self {
        if !matches!(self, AppError::Internal(_)) {
            tracing::debug!(error = %err, "request rejected");
        }
        self
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, fields, existing) = match self {
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "bad_request",
                msg.to_string(),
                Vec::new(),
                None,
            ),
            AppError::Validation(fields) => (
                StatusCode::BAD_REQUEST,
                "validation_failed",
                "some fields are invalid".to_string(),
                fields,
                None,
            ),
            AppError::NotFound(what) => (
                StatusCode::NOT_FOUND,
                "not_found",
                format!("{what} not found"),
                Vec::new(),
                None,
            ),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "authentication required".to_string(),
                Vec::new(),
                None,
            ),
            AppError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                "forbidden",
                msg.to_string(),
                Vec::new(),
                None,
            ),
            AppError::Conflict {
                code,
                message,
                existing,
            } => (StatusCode::CONFLICT, code, message, Vec::new(), existing),
            AppError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_requests",
                "too many attempts, try again later".to_string(),
                Vec::new(),
                None,
            ),
            AppError::Internal(err) => {
                tracing::error!(error = ?err, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_string(),
                    Vec::new(),
                    None,
                )
            }
        };

        (
            status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code,
                    message,
                    fields,
                    existing,
                },
            }),
        )
            .into_response()
    }
}

/// Convenience alias used by handlers and services.
pub type AppResult<T> = Result<T, AppError>;
