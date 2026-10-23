use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "BAD_REQUEST", message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", message)
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "FORBIDDEN", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message)
    }

    pub fn too_many(message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", message)
    }

    pub fn internal() -> Self {
        tracing::error!("internal application error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Something went wrong. Please try again.",
        )
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &err {
            let message = db.message().to_string();
            if message.contains("UNIQUE constraint failed: users.username")
                || message.contains("idx_users_username")
            {
                return AppError::conflict("DUPLICATE_USERNAME", "This username is already taken.");
            }
            if message.contains("UNIQUE constraint failed: users.email")
                || message.contains("idx_users_email")
            {
                return AppError::conflict("DUPLICATE_EMAIL", "This email is already registered.");
            }
        }
        tracing::error!(error = %err, "database error");
        AppError::internal()
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(err: argon2::password_hash::Error) -> Self {
        tracing::error!(error = %err, "password hash error");
        AppError::internal()
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: &'a str,
    message: &'a str,
    request_id: &'a str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let request_id = crate::middleware::current_request_id();
        let body = ErrorBody {
            error: ErrorDetail {
                code: self.code,
                message: &self.message,
                request_id: &request_id,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
