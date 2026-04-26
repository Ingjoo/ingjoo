use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Serialize)]
pub struct ModuleInfo {
    pub name: String,
    pub state: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
}

/// 列出所有模块（已安装 + 可安装）
pub async fn list_modules(State(state): State<Arc<AppState>>) -> Result<Json<Vec<ModuleInfo>>, AppError> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT name, state, version, description, author, website FROM ir_module ORDER BY name",
    )
    .fetch_all(&*state.pool)
    .await
    .map_err(|e| {
        tracing::error!("查询模块列表失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("查询模块列表失败"))
    })?;

    let modules = rows
        .into_iter()
        .map(|(name, state, version, description, author, website)| ModuleInfo {
            name,
            state,
            version,
            description,
            author,
            website,
        })
        .collect();

    Ok(Json(modules))
}

/// 安装模块（从请求体获取模块名，注册到 ir_module）
#[derive(Deserialize)]
pub struct InstallRequest {
    pub name: String,
}

pub async fn install_module(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InstallRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let name = body.name;
    let existing: Option<(String,)> = sqlx::query_as("SELECT name FROM ir_module WHERE name = ?")
        .bind(&name)
        .fetch_optional(&*state.pool)
        .await
        .map_err(|e| {
            tracing::error!("查询模块失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("查询模块失败"))
        })?;

    if existing.is_some() {
        return Err(AppError::Conflict(format!("模块 '{}' 已安装", name)));
    }

    let (ignore_prefix, now_fn, on_conflict) = match &state.dialect {
        ingjoo_core::Dialect::Sqlite => ("INSERT OR IGNORE INTO", "datetime('now')", ""),
        _ => ("INSERT INTO", "CURRENT_TIMESTAMP", " ON CONFLICT (name) DO NOTHING"),
    };
    let sql =
        format!("{ignore_prefix} ir_module (name, state, installed_at) VALUES (?, 'installed', {now_fn}){on_conflict}");
    sqlx::query(&state.dialect.prepare(&sql)).bind(&name).execute(&*state.pool).await.map_err(|e| {
        tracing::error!("注册模块失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("注册模块失败"))
    })?;

    Ok(Json(serde_json::json!({"installed": name})))
}

/// 卸载模块
pub async fn uninstall_module(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    let result =
        sqlx::query("DELETE FROM ir_module WHERE name = ?").bind(&name).execute(&*state.pool).await.map_err(|e| {
            tracing::error!("卸载模块失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("卸载模块失败"))
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("模块 '{}' 不存在", name)));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// 升级模块（暂未实现）
pub async fn upgrade_module(Path(_name): Path<String>) -> Result<Json<serde_json::Value>, AppError> {
    Err(AppError::Internal(anyhow::anyhow!("模块升级功能尚未实现")))
}
