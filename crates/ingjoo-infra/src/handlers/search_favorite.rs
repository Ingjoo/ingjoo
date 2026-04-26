//! 搜索收藏 handler — 用户保存和召回搜索过滤条件

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use ingjoo_core::Dialect;
use serde::{Deserialize, Serialize};

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Serialize, Deserialize)]
pub struct SearchFavorite {
    pub id: i64,
    pub user_id: String,
    pub name: String,
    pub model: String,
    pub domain: Option<serde_json::Value>,
    pub context: Option<serde_json::Value>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateSearchFavorite {
    pub name: String,
    pub model: String,
    pub domain: Option<serde_json::Value>,
    pub context: Option<serde_json::Value>,
    pub is_default: Option<bool>,
}

#[derive(Deserialize)]
pub struct ListFavoritesQuery {
    pub model: Option<String>,
}

pub async fn list_favorites(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListFavoritesQuery>,
) -> Result<Json<Vec<SearchFavorite>>, AppError> {
    let user_id = &current_user.user_id;

    let ts_cast = match &state.dialect {
        Dialect::Sqlite => "",
        _ => "::text",
    };
    let sql = if query.model.is_some() {
        state.dialect.prepare(&format!(
            "SELECT id, user_id, name, model, domain, context, is_default, created_at{ts_cast}, updated_at{ts_cast} \
             FROM ir_search_favorite WHERE user_id = ? AND model = ? ORDER BY created_at DESC",
        ))
    } else {
        state.dialect.prepare(&format!(
            "SELECT id, user_id, name, model, domain, context, is_default, created_at{ts_cast}, updated_at{ts_cast} \
             FROM ir_search_favorite WHERE user_id = ? ORDER BY created_at DESC",
        ))
    };

    let rows = if let Some(model) = &query.model {
        sqlx::query_as::<_, (i64, String, String, String, Option<String>, Option<String>, bool, String, String)>(&sql)
            .bind(user_id)
            .bind(model)
            .fetch_all(&*state.pool)
            .await
    } else {
        sqlx::query_as::<_, (i64, String, String, String, Option<String>, Option<String>, bool, String, String)>(&sql)
            .bind(user_id)
            .fetch_all(&*state.pool)
            .await
    }
    .map_err(|e| {
        tracing::error!("查询搜索收藏失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("查询搜索收藏失败"))
    })?;

    let favorites = rows
        .into_iter()
        .map(|(id, user_id, name, model, domain, context, is_default, created_at, updated_at)| SearchFavorite {
            id,
            user_id,
            name,
            model,
            domain: domain.and_then(|d| serde_json::from_str(&d).ok()),
            context: context.and_then(|c| serde_json::from_str(&c).ok()),
            is_default,
            created_at,
            updated_at,
        })
        .collect();

    Ok(Json(favorites))
}

pub async fn create_favorite(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSearchFavorite>,
) -> Result<Json<SearchFavorite>, AppError> {
    let user_id = &current_user.user_id;

    let domain_str = req.domain.as_ref().and_then(|d| serde_json::to_string(d).ok());
    let context_str = req.context.as_ref().and_then(|c| serde_json::to_string(c).ok());
    let is_default = req.is_default.unwrap_or(false);
    let is_default_literal = if is_default { "TRUE" } else { "FALSE" };

    let now_fn = match &state.dialect {
        Dialect::Sqlite => "datetime('now')",
        _ => "NOW()",
    };

    let sql = state.dialect.prepare(&format!(
        "INSERT INTO ir_search_favorite (user_id, name, model, domain, context, is_default, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, {is_default_literal}, {now_fn}, {now_fn})"
    ));

    let result = sqlx::query(&sql)
        .bind(user_id)
        .bind(&req.name)
        .bind(&req.model)
        .bind(&domain_str)
        .bind(&context_str)
        .execute(&*state.pool)
        .await
        .map_err(|e| {
            tracing::error!("创建搜索收藏失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("创建搜索收藏失败"))
        })?;

    let id = result.last_insert_id().unwrap_or(0);

    Ok(Json(SearchFavorite {
        id,
        user_id: user_id.clone(),
        name: req.name,
        model: req.model,
        domain: req.domain,
        context: req.context,
        is_default,
        created_at: String::new(),
        updated_at: String::new(),
    }))
}

pub async fn delete_favorite(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = &current_user.user_id;

    let check_sql = state.dialect.prepare("SELECT user_id FROM ir_search_favorite WHERE id = ?");
    let row: Option<(String,)> =
        sqlx::query_as(&check_sql).bind(id).fetch_optional(&*state.pool).await.map_err(|e| {
            tracing::error!("查询搜索收藏失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("查询搜索收藏失败"))
        })?;

    match row {
        None => return Err(AppError::NotFound(format!("搜索收藏 #{}", id))),
        Some((ref owner_id,)) if owner_id != user_id => {
            return Err(AppError::Forbidden("只能删除自己的搜索收藏".into()));
        }
        _ => {}
    }

    let delete_sql = state.dialect.prepare("DELETE FROM ir_search_favorite WHERE id = ?");
    sqlx::query(&delete_sql).bind(id).execute(&*state.pool).await.map_err(|e| {
        tracing::error!("删除搜索收藏失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("删除搜索收藏失败"))
    })?;

    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn set_default_favorite(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = &current_user.user_id;

    let check_sql = state
        .dialect
        .prepare("SELECT user_id, model FROM ir_search_favorite WHERE id = ?");
    let row: Option<(String, String)> =
        sqlx::query_as(&check_sql).bind(id).fetch_optional(&*state.pool).await.map_err(|e| {
            tracing::error!("查询搜索收藏失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("查询搜索收藏失败"))
        })?;

    let model = match row {
        None => return Err(AppError::NotFound(format!("搜索收藏 #{}", id))),
        Some((ref owner_id, ref model)) => {
            if owner_id != user_id {
                return Err(AppError::Forbidden("只能操作自己的搜索收藏".into()));
            }
            model.clone()
        }
    };

    let now_fn = match &state.dialect {
        Dialect::Sqlite => "datetime('now')",
        _ => "NOW()",
    };

    let unset_sql = state.dialect.prepare(&format!(
        "UPDATE ir_search_favorite SET is_default = FALSE, updated_at = {now_fn} \
         WHERE user_id = ? AND model = ? AND is_default = TRUE"
    ));
    sqlx::query(&unset_sql)
        .bind(user_id)
        .bind(&model)
        .execute(&*state.pool)
        .await
        .map_err(|e| {
            tracing::error!("取消默认搜索收藏失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("取消默认搜索收藏失败"))
        })?;

    let set_sql = state.dialect.prepare(&format!(
        "UPDATE ir_search_favorite SET is_default = TRUE, updated_at = {now_fn} WHERE id = ?"
    ));
    sqlx::query(&set_sql).bind(id).execute(&*state.pool).await.map_err(|e| {
        tracing::error!("设置默认搜索收藏失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("设置默认搜索收藏失败"))
    })?;

    Ok(Json(serde_json::json!({"ok": true})))
}
