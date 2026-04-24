use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use ingjoo_core::module::plugin::PluginInfo;

use crate::middleware::error::AppError;
use crate::AppState;

pub async fn list_plugins(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PluginInfo>>, AppError> {
    let manager = state.plugin_manager.as_ref()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("插件管理器未初始化")))?;
    Ok(Json(manager.list_plugins()))
}

pub async fn load_all_plugins(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let dir = body["plugins_dir"].as_str()
        .ok_or_else(|| AppError::BadRequest("缺少 plugins_dir 字段".into()))?;
    let manager = state.plugin_manager.as_ref()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("插件管理器未初始化")))?;
    let loaded = manager.load_from_dir(std::path::Path::new(dir))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("加载插件失败: {}", e)))?;
    Ok(Json(serde_json::json!({"loaded": loaded})))
}

pub async fn unload_plugin(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let manager = state.plugin_manager.as_ref()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("插件管理器未初始化")))?;
    manager.unload_plugin(&name)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("卸载插件失败: {}", e)))?;
    Ok(Json(serde_json::json!({"unloaded": name})))
}

pub async fn reload_plugin(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let manager = state.plugin_manager.as_ref()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("插件管理器未初始化")))?;
    manager.reload_plugin(&name)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("重载插件失败: {}", e)))?;
    Ok(Json(serde_json::json!({"reloaded": name})))
}
