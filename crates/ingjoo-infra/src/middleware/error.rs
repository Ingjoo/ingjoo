use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use ingjoo_core::db::error::StoreError;
use serde_json::json;

/// HTTP 层错误类型，自动映射为状态码 + JSON 响应
pub enum AppError {
    Internal(anyhow::Error),
    NotFound(String),
    Unauthorized(String),
    Forbidden(String),
    BadRequest(String),
    Conflict(String),
    TooManyRequests(String),
    ServiceUnavailable(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, msg) = match &self {
            AppError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "服务器内部错误".to_string())
            }
            AppError::NotFound(id) => (StatusCode::NOT_FOUND, "NOT_FOUND", format!("未找到: {}", id)),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone()),
            AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", msg.clone()),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE", msg.clone()),
        };
        if let AppError::Internal(e) = &self {
            tracing::error!("Internal error: {:?}", e);
        }
        (
            status,
            Json(json!({
                "error": msg,
                "code": code,
                "status": status.as_u16(),
            })),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err)
    }
}

impl From<StoreError> for AppError {
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::NotFound(msg) => AppError::NotFound(msg),
            StoreError::UniqueViolation { table, column } => {
                AppError::Conflict(format!("唯一约束冲突: {}.{}", table, column))
            }
            StoreError::ForeignKeyViolation(msg) => AppError::BadRequest(format!("外键约束冲突: {}", msg)),
            StoreError::Database(msg) => AppError::Internal(anyhow::anyhow!(msg)),
            StoreError::Config(msg) => AppError::Internal(anyhow::anyhow!(msg)),
            StoreError::BadRequest(msg) => AppError::BadRequest(msg),
            StoreError::Io(e) => AppError::Internal(e.into()),
        }
    }
}
