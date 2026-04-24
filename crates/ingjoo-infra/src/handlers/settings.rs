use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::Deserialize;

use ingjoo_core::db::traits::IngjooStore;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Deserialize)]
pub struct SetSettingRequest {
    pub value: String,
}

async fn require_admin(user: &CurrentUser, store: &Arc<dyn IngjooStore>) -> Result<(), AppError> {
    if user.role != "admin" && user.role != "owner" {
        let _ = store.create_audit_log(
            Some(&user.user_id),
            "admin_required_denied",
            "settings",
            None,
            None,
            None,
        ).await;
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    Ok(())
}

pub async fn list_settings(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let settings = state.store.get_all_settings().await?;
    Ok(Json(settings))
}

pub async fn set_setting(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(req): Json<SetSettingRequest>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user, &state.store).await?;
    state.store.set_setting(&key, &req.value).await?;
    Ok(StatusCode::NO_CONTENT)
}
