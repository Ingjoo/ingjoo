//! 全局搜索 + 索引重建 handler

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use ingjoo_core::extension::search::SearchQuery;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct GlobalSearchRequest {
    pub text: String,
    pub models: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub async fn global_search(
    Extension(_current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<GlobalSearchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let text = body.text.trim().to_string();
    if text.is_empty() {
        return Err(AppError::BadRequest("搜索关键词不能为空".into()));
    }

    let models = match body.models {
        Some(ref m) if !m.is_empty() => m.clone(),
        _ => state.registry.list().iter().map(|m| m.name.clone()).collect(),
    };

    let query = SearchQuery { text, models, limit: body.limit, offset: body.offset, filters: None };

    let results = state.search.search(query).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("搜索服务未启用") {
            AppError::ServiceUnavailable("搜索服务未启用".into())
        } else {
            AppError::Internal(anyhow::anyhow!("搜索失败: {}", msg))
        }
    })?;

    let total = results.len();

    Ok(Json(json!({
        "results": results,
        "total": total,
        "query": body.text,
    })))
}

pub async fn rebuild_index(
    Extension(_current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
) -> Result<StatusCode, AppError> {
    state.search.rebuild_index(&model_name).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("搜索服务未启用") {
            AppError::ServiceUnavailable("搜索服务未启用".into())
        } else {
            AppError::Internal(anyhow::anyhow!("重建索引失败: {}", msg))
        }
    })?;

    Ok(StatusCode::NO_CONTENT)
}
