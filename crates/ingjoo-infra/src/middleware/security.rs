use std::sync::Arc;

use axum::extract::Request;
use axum::extract::State;
use axum::middleware::Next;
use axum::response::Response;

use crate::auth::AuthProvider;
use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = request.headers().get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok());

    let current_user = match auth_header {
        Some(header) => {
            if let Some(token) = header.strip_prefix("Bearer ") {
                match state.auth.verify_access_token(token) {
                    Ok(claims) => CurrentUser {
                        user_id: claims.sub,
                        role: claims.role,
                        groups: claims.groups,
                        database: claims.database,
                    },
                    Err(_) => return Err(AppError::Unauthorized("未认证".into())),
                }
            } else {
                return Err(AppError::Unauthorized("未认证".into()));
            }
        }
        None => return Err(AppError::Unauthorized("未认证".into())),
    };

    request.extensions_mut().insert(current_user);

    Ok(next.run(request).await)
}
