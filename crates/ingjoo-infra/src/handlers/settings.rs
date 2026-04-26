use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use ingjoo_core::db::traits::IngjooStore;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Serialize)]
pub struct SettingsDefinition {
    pub key: String,
    pub r#type: String,
    pub default_value: String,
    pub group_name: String,
    pub label: String,
    pub description: String,
}

#[derive(Deserialize)]
pub struct SetSettingRequest {
    pub value: String,
}

async fn require_admin(user: &CurrentUser, store: &Arc<dyn IngjooStore>) -> Result<(), AppError> {
    if user.role != "admin" && user.role != "owner" {
        let _ =
            store.create_audit_log(Some(&user.user_id), "admin_required_denied", "settings", None, None, None).await;
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

/// 获取所有设置定义（含分组、类型、默认值）
pub async fn list_settings_definitions(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<SettingsDefinition>>, AppError> {
    require_admin(&current_user, &state.store).await?;

    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT key, type, default_value, group_name, label, description FROM ir_settings_definition ORDER BY group_name, key"
    )
    .fetch_all(&*state.pool)
    .await
    .map_err(|e| {
        tracing::error!("查询设置定义失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("查询设置定义失败"))
    })?;

    let defs = rows
        .into_iter()
        .map(|(key, r#type, default_value, group_name, label, description)| SettingsDefinition {
            key,
            r#type,
            default_value,
            group_name,
            label,
            description,
        })
        .collect();

    Ok(Json(defs))
}

#[derive(Deserialize)]
pub struct ModuleSettingsQuery {
    pub module: Option<String>,
}

/// 获取模块配置项（公开端点，用于 Footer 合规信息等）
pub async fn list_public_module_settings(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModuleSettingsQuery>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let settings = state
        .store
        .list_module_settings("system", None, query.module.as_deref())
        .await
        .unwrap_or_default();
    let items: Vec<serde_json::Value> = settings
        .into_iter()
        .map(|s| serde_json::json!({ "key": s.key, "value": s.value }))
        .collect();
    Ok(Json(items))
}
