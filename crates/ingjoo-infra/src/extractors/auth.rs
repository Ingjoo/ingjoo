use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::auth::AuthProvider;
use crate::AppState;

#[derive(Clone, Debug)]
pub struct CurrentUser {
    pub user_id: String,
    pub role: String,
    pub groups: Vec<String>,
}

impl CurrentUser {
    pub fn is_admin(&self) -> bool {
        self.groups.iter().any(|g| g == "admin")
    }

    pub fn require_admin(&self) -> Result<(), crate::middleware::error::AppError> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(crate::middleware::error::AppError::Forbidden("需要管理员权限".into()))
        }
    }
}

pub enum AuthRejection {
    MissingToken,
    InvalidToken,
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AuthRejection::MissingToken => {
                (StatusCode::UNAUTHORIZED, "Missing authorization header")
            }
            AuthRejection::InvalidToken => {
                (StatusCode::UNAUTHORIZED, "Invalid token")
            }
        };
        (status, msg).into_response()
    }
}

impl FromRequestParts<Arc<AppState>> for CurrentUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthRejection::MissingToken)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthRejection::InvalidToken)?;

        let claims = state
            .auth
            .verify_access_token(token)
            .map_err(|_| AuthRejection::InvalidToken)?;

        Ok(CurrentUser {
            user_id: claims.sub,
            role: claims.role,
            groups: claims.groups,
        })
    }
}
