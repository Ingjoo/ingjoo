use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Deserialize)]
pub struct ListQuery {
    pub unread: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list_notifications(
    State(state): State<Arc<AppState>>,
    user: CurrentUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let unread_only = q.unread.unwrap_or(false);
    let limit = q.limit.unwrap_or(20).min(100);
    let offset = q.offset.unwrap_or(0);
    let items = state
        .notification
        .list_notifications(&user.user_id, unread_only, limit, offset)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(serde_json::json!({ "items": items })))
}

pub async fn get_unread_count(
    State(state): State<Arc<AppState>>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, AppError> {
    let count = state.notification.get_unread_count(&user.user_id).await.map_err(AppError::Internal)?;
    Ok(Json(serde_json::json!({ "count": count })))
}

pub async fn mark_read(
    State(state): State<Arc<AppState>>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let ok = state.notification.mark_read(&id, &user.user_id).await.map_err(AppError::Internal)?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "success": ok }))))
}

pub async fn mark_all_read(
    State(state): State<Arc<AppState>>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, AppError> {
    let count = state.notification.mark_all_read(&user.user_id).await.map_err(AppError::Internal)?;
    Ok(Json(serde_json::json!({ "updated": count })))
}
