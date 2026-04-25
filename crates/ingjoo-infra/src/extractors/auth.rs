use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::auth::AuthProvider;
use crate::AppState;

/// 从 Bearer token 解析出的当前用户信息
#[derive(Clone, Debug)]
pub struct CurrentUser {
    pub user_id: String,
    pub role: String,
    pub groups: Vec<String>,
    pub database: Option<String>,
}

impl CurrentUser {
    /// 判断用户是否属于 admin 组
    pub fn is_admin(&self) -> bool {
        self.groups.iter().any(|g| g == "admin")
    }

    /// 要求管理员权限，否则返回 Forbidden
    pub fn require_admin(&self) -> Result<(), crate::middleware::error::AppError> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(crate::middleware::error::AppError::Forbidden("需要管理员权限".into()))
        }
    }
}

/// 认证提取失败原因
pub enum AuthRejection {
    MissingToken,
    InvalidToken,
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        let (status, code, msg) = match self {
            AuthRejection::MissingToken => {
                (StatusCode::UNAUTHORIZED, "MISSING_TOKEN", "缺少认证头")
            }
            AuthRejection::InvalidToken => {
                (StatusCode::UNAUTHORIZED, "INVALID_TOKEN", "无效的认证令牌")
            }
        };
        (status, Json(json!({
            "error": msg,
            "code": code,
            "status": status.as_u16(),
        })))
            .into_response()
    }
}

impl FromRequestParts<Arc<AppState>> for CurrentUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::to_owned)
            .or_else(|| {
                parts
                    .headers
                    .get(axum::http::header::COOKIE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|cookie_str| {
                        cookie_str
                            .split(';')
                            .map(str::trim)
                            .find(|c| c.starts_with("access_token="))
                        .and_then(|c| c.strip_prefix("access_token=").map(str::to_owned))
                    })
            })
            .ok_or(AuthRejection::MissingToken)?;

        let claims = state
            .auth
            .verify_access_token(&token)
            .map_err(|_| AuthRejection::InvalidToken)?;

        Ok(CurrentUser {
            user_id: claims.sub,
            role: claims.role,
            groups: claims.groups,
            database: claims.database,
        })
    }
}
