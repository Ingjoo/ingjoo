use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub enum AppError {
    Internal(anyhow::Error),
    NotFound(String),
    Unauthorized(String),
    Forbidden(String),
    BadRequest(String),
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
        }
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        AppError::Internal(err.into())
    }
}
