use std::sync::Arc;

use axum::extract::Request;
use axum::extract::State;
use axum::middleware::Next;
use axum::response::Response;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

/// 管理员权限守卫中间件，要求 CurrentUser.is_admin()
pub async fn require_admin(
    State(_state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let current_user = request
        .extensions()
        .get::<CurrentUser>()
        .ok_or(AppError::Unauthorized("未认证".into()))?;

    if !current_user.is_admin() {
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }

    Ok(next.run(request).await)
}
