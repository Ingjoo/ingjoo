use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::Deserialize;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Deserialize)]
pub struct SetSettingRequest {
    pub value: String,
}

pub async fn list_settings(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    if current_user.role != "admin" && current_user.role != "owner" {
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    let settings = state.store.get_all_settings().await?;
    Ok(Json(settings))
}

pub async fn set_setting(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(req): Json<SetSettingRequest>,
) -> Result<StatusCode, AppError> {
    if current_user.role != "admin" && current_user.role != "owner" {
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    state.store.set_setting(&key, &req.value).await?;
    Ok(StatusCode::NO_CONTENT)
}
