use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use ingjoo_core::db::error::StoreError;

pub enum AppError {
    Internal(anyhow::Error),
    NotFound(String),
    Unauthorized(String),
    Forbidden(String),
    BadRequest(String),
    Conflict(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Internal(e) => {
                tracing::error!("Internal error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
            }
            AppError::NotFound(id) => {
                (StatusCode::NOT_FOUND, format!("Not found: {}", id)).into_response()
            }
            AppError::Unauthorized(msg) => {
                (StatusCode::UNAUTHORIZED, msg).into_response()
            }
            AppError::Forbidden(msg) => {
                (StatusCode::FORBIDDEN, msg).into_response()
            }
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, msg).into_response()
            }
            AppError::Conflict(msg) => {
                (StatusCode::CONFLICT, msg).into_response()
            }
        }
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
            StoreError::ForeignKeyViolation(msg) => {
                AppError::BadRequest(format!("外键约束冲突: {}", msg))
            }
            StoreError::Database(msg) => AppError::Internal(anyhow::anyhow!(msg)),
            StoreError::Config(msg) => AppError::Internal(anyhow::anyhow!(msg)),
            StoreError::BadRequest(msg) => AppError::BadRequest(msg),
            StoreError::Io(e) => AppError::Internal(e.into()),
        }
    }
}
