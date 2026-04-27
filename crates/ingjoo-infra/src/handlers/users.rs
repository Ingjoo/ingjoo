use std::sync::Arc;

use axum::extract::Query;
use axum::extract::State;
use axum::Extension;
use axum::Json;
use ingjoo_core::extension::audit::AuditQuery;
use ingjoo_core::UserPublic;
use serde::Deserialize;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Deserialize)]
pub struct SearchUsersParams {
    pub q: String,
    pub limit: Option<i64>,
}

pub async fn search_users(
    Extension(_current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchUsersParams>,
) -> Result<Json<Vec<UserPublic>>, AppError> {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let results = state.store.search_users(&params.q, limit).await?;
    Ok(Json(results))
}

#[derive(Deserialize)]
pub struct AuditLogParams {
    pub action: Option<String>,
    pub entity_type: Option<String>,
    pub user_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn get_audit_log(
    Extension(_current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditLogParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let offset = params.offset.unwrap_or(0).max(0);

    let query = AuditQuery {
        user_id: params.user_id,
        action: params.action,
        resource: params.entity_type,
        resource_id: None,
        ip: None,
        limit: Some(limit),
        offset: Some(offset),
    };

    let items = state.audit.list_audit_logs(query).await?;
    let total = items.len() as i64;

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "limit": limit,
        "offset": offset,
    })))
}
