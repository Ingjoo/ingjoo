use std::sync::Arc;

use axum::extract::State;
use axum::Extension;
use axum::Json;
use serde_json::json;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

pub async fn get_stats(
    Extension(_current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let users_page = state.store.list_users(1, 0).await?;
    let total_users = users_page.total;

    let active_users_24h: i64 = 0;

    Ok(Json(json!({
        "total_users": total_users,
        "active_users_24h": active_users_24h,
        "total_records": 0i64,
    })))
}
