use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::Deserialize;
use sqlx::Row;

use ingjoo_core::ActionDescriptor;
use ingjoo_core::ActionType;
use ingjoo_core::ViewType;

use super::view::{ViewRow, VIEW_COLUMNS};
use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

const ACTION_COLUMNS: &str =
    "id, name, type, res_model, view_mode, view_ids, domain, context, page_limit, target, search_view_id, url, help, group_ids";

fn require_admin(user: &CurrentUser) -> Result<(), AppError> {
    if user.is_admin() {
        Ok(())
    } else {
        Err(AppError::Forbidden("需要管理员权限".into()))
    }
}

// ==================== 行提取 ====================

struct ActionRow {
    id: String,
    name: String,
    action_type_str: String,
    res_model: Option<String>,
    view_mode_str: Option<String>,
    view_ids_str: Option<String>,
    domain_str: Option<String>,
    context_str: Option<String>,
    limit: Option<i32>,
    target: Option<String>,
    search_view_id: Option<String>,
    url: Option<String>,
    help: Option<String>,
    group_ids_str: String,
}

impl ActionRow {
    fn from_row(row: &sqlx::any::AnyRow) -> Self {
        Self {
            id: row.get("id"),
            name: row.get("name"),
            action_type_str: row.try_get("type").ok().unwrap_or_else(|| "act_window".into()),
            res_model: row.try_get("res_model").ok().flatten(),
            view_mode_str: row.try_get("view_mode").ok().flatten(),
            view_ids_str: row.try_get("view_ids").ok().flatten(),
            domain_str: row.try_get("domain").ok().flatten(),
            context_str: row.try_get("context").ok().flatten(),
            limit: row.try_get("page_limit").ok().flatten(),
            target: row.try_get("target").ok().flatten(),
            search_view_id: row.try_get("search_view_id").ok().flatten(),
            url: row.try_get("url").ok().flatten(),
            help: row.try_get("help").ok().flatten(),
            group_ids_str: row.try_get("group_ids").ok().unwrap_or_default(),
        }
    }

    fn to_descriptor(&self) -> ActionDescriptor {
        let action_type = match self.action_type_str.as_str() {
            "act_window" => ActionType::ActWindow,
            "act_url" => ActionType::ActUrl,
            "act_server" => ActionType::ActServer,
            _ => ActionType::ActWindow,
        };
        let view_mode: Vec<ViewType> = self
            .view_mode_str
            .as_deref()
            .map(|s| {
                s.split(',')
                    .filter_map(|t| match t.trim() {
                        "form" => Some(ViewType::Form),
                        "list" => Some(ViewType::List),
                        "kanban" => Some(ViewType::Kanban),
                        "search" => Some(ViewType::Search),
                        "graph" => Some(ViewType::Graph),
                        "calendar" => Some(ViewType::Calendar),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let view_ids: Vec<String> = self
            .view_ids_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let domain: Option<serde_json::Value> =
            self.domain_str.as_deref().and_then(|s| serde_json::from_str(s).ok());
        let context: Option<serde_json::Value> =
            self.context_str.as_deref().and_then(|s| serde_json::from_str(s).ok());
        let group_ids: Vec<String> =
            serde_json::from_str(&self.group_ids_str).unwrap_or_default();

        ActionDescriptor {
            id: self.id.clone(),
            name: self.name.clone(),
            action_type,
            res_model: self.res_model.clone(),
            view_mode,
            view_ids,
            domain,
            context,
            limit: self.limit,
            target: self.target.clone().or_else(|| Some("current".into())),
            search_view_id: self.search_view_id.clone(),
            url: self.url.clone(),
            help: self.help.clone(),
            group_ids,
        }
    }
}

// ==================== 公开 API ====================

/// GET /api/actions/{id} — 获取 action + 关联 views + model schema（一次请求）
pub async fn get_action(
    Extension(_current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sql = state.dialect.prepare(&format!(
        "SELECT {} FROM ir_action WHERE id = ?",
        ACTION_COLUMNS
    ));
    let row = sqlx::query(&sql)
        .bind(&id)
        .fetch_optional(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询动作失败: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("动作 '{}' 不存在", id)))?;

    let action = ActionRow::from_row(&row).to_descriptor();

    // 查询关联视图
    let views_json = if !action.view_ids.is_empty() {
        fetch_views_by_ids(&state, &action.view_ids).await?
    } else if let Some(ref res_model) = action.res_model {
        fetch_views_by_model(&state, res_model).await?
    } else {
        serde_json::json!({})
    };

    // 从注册表获取 model schema
    let model_json = if let Some(ref res_model) = action.res_model {
        state
            .registry
            .get(res_model)
            .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };

    Ok(Json(serde_json::json!({
        "action": action,
        "views": views_json,
        "model": model_json,
    })))
}

/// GET /api/actions — 列出所有动作（管理员）
pub async fn list_actions(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ActionDescriptor>>, AppError> {
    require_admin(&current_user)?;

    let sql = state.dialect.prepare(&format!(
        "SELECT {} FROM ir_action ORDER BY name",
        ACTION_COLUMNS
    ));
    let rows = sqlx::query(&sql)
        .fetch_all(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询动作失败: {}", e)))?;

    Ok(Json(rows.iter().map(|r| ActionRow::from_row(r).to_descriptor()).collect()))
}

/// POST /api/actions — 创建动作（管理员）
pub async fn create_action(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateActionRequest>,
) -> Result<(StatusCode, Json<ActionDescriptor>), AppError> {
    require_admin(&current_user)?;

    let id = uuid::Uuid::new_v4().to_string();
    let view_mode_str: String = req.view_mode.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(",");
    let view_ids_str = serde_json::to_string(&req.view_ids).unwrap_or_else(|_| "[]".into());
    let domain_str = req.domain.as_ref().and_then(|d| serde_json::to_string(d).ok());
    let context_str = req.context.as_ref().and_then(|c| serde_json::to_string(c).ok());
    let group_ids = req.group_ids.unwrap_or_default();
    let group_ids_str = serde_json::to_string(&group_ids).unwrap_or_else(|_| "[]".into());
    let target = req.target.as_deref().unwrap_or("current");

    let sql = state.dialect.prepare(
        "INSERT INTO ir_action (id, name, type, res_model, view_mode, view_ids, domain, context, page_limit, target, url, help, group_ids) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    );
    sqlx::query(&sql)
        .bind(&id)
        .bind(&req.name)
        .bind(req.action_type.as_str())
        .bind(&req.res_model)
        .bind(&view_mode_str)
        .bind(&view_ids_str)
        .bind(&domain_str)
        .bind(&context_str)
        .bind(req.limit)
        .bind(target)
        .bind(&req.url)
        .bind(&req.help)
        .bind(&group_ids_str)
        .execute(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("创建动作失败: {}", e)))?;

    Ok((StatusCode::CREATED, Json(ActionDescriptor {
        id,
        name: req.name,
        action_type: req.action_type,
        res_model: req.res_model,
        view_mode: req.view_mode,
        view_ids: req.view_ids,
        domain: req.domain,
        context: req.context,
        limit: req.limit,
        target: Some(target.to_string()),
        search_view_id: None,
        url: req.url,
        help: req.help,
        group_ids,
    })))
}

/// PUT /api/actions/{id} — 更新动作（管理员）
pub async fn update_action(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateActionRequest>,
) -> Result<Json<ActionDescriptor>, AppError> {
    require_admin(&current_user)?;

    let sql = state.dialect.prepare(&format!(
        "SELECT {} FROM ir_action WHERE id = ?",
        ACTION_COLUMNS
    ));
    let row = sqlx::query(&sql)
        .bind(&id)
        .fetch_optional(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询动作失败: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("动作 '{}' 不存在", id)))?;

    let existing = ActionRow::from_row(&row);

    let name = req.name.unwrap_or(existing.name);
    let res_model = req.res_model.or(existing.res_model);
    let view_mode = req.view_mode.unwrap_or_else(|| {
        existing
            .view_mode_str
            .as_deref()
            .map(|s| {
                s.split(',')
                    .filter_map(|t| match t.trim() {
                        "form" => Some(ViewType::Form),
                        "list" => Some(ViewType::List),
                        "kanban" => Some(ViewType::Kanban),
                        "search" => Some(ViewType::Search),
                        "graph" => Some(ViewType::Graph),
                        "calendar" => Some(ViewType::Calendar),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    });
    let view_ids = req.view_ids.unwrap_or_else(|| {
        existing
            .view_ids_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    });
    let domain = req.domain.or_else(|| {
        existing
            .domain_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
    });
    let context = req.context.or_else(|| {
        existing
            .context_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
    });
    let limit = req.limit.or(existing.limit);
    let target = req.target.or(existing.target).unwrap_or_else(|| "current".into());
    let help = req.help.or(existing.help);
    let group_ids = req.group_ids.unwrap_or_else(|| {
        serde_json::from_str(&existing.group_ids_str).unwrap_or_default()
    });

    let view_mode_str: String = view_mode.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(",");
    let view_ids_str = serde_json::to_string(&view_ids).unwrap_or_else(|_| "[]".into());
    let domain_str = domain.as_ref().and_then(|d| serde_json::to_string(d).ok());
    let context_str = context.as_ref().and_then(|c| serde_json::to_string(c).ok());
    let group_ids_str = serde_json::to_string(&group_ids).unwrap_or_else(|_| "[]".into());

    let sql = state.dialect.prepare(
        "UPDATE ir_action SET name = ?, res_model = ?, view_mode = ?, view_ids = ?, domain = ?, context = ?, page_limit = ?, target = ?, help = ?, group_ids = ?, updated_at = datetime('now') WHERE id = ?",
    );
    sqlx::query(&sql)
        .bind(&name)
        .bind(&res_model)
        .bind(&view_mode_str)
        .bind(&view_ids_str)
        .bind(&domain_str)
        .bind(&context_str)
        .bind(limit)
        .bind(&target)
        .bind(&help)
        .bind(&group_ids_str)
        .bind(&id)
        .execute(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("更新动作失败: {}", e)))?;

    Ok(Json(ActionDescriptor {
        id,
        name,
        action_type: match existing.action_type_str.as_str() {
            "act_window" => ActionType::ActWindow,
            "act_url" => ActionType::ActUrl,
            "act_server" => ActionType::ActServer,
            _ => ActionType::ActWindow,
        },
        res_model,
        view_mode,
        view_ids,
        domain,
        context,
        limit,
        target: Some(target),
        search_view_id: existing.search_view_id,
        url: existing.url,
        help,
        group_ids,
    }))
}

/// DELETE /api/actions/{id} — 删除动作（管理员）
pub async fn delete_action(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user)?;

    let sql = state.dialect.prepare("DELETE FROM ir_action WHERE id = ?");
    let result = sqlx::query(&sql)
        .bind(&id)
        .execute(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("删除动作失败: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("动作 '{}' 不存在", id)));
    }
    Ok(StatusCode::NO_CONTENT)
}

// ==================== 视图查询辅助 ====================

async fn fetch_views_by_ids(
    state: &AppState,
    view_ids: &[String],
) -> Result<serde_json::Value, AppError> {
    let placeholders = state.dialect.placeholders(view_ids.len(), 1);
    let ph_str = placeholders.join(",");
    let sql_str = format!(
        "SELECT {} FROM ir_view WHERE id IN ({}) AND active = 1",
        VIEW_COLUMNS, ph_str
    );
    let sql = state.dialect.prepare(&sql_str);

    let mut q = sqlx::query(&sql);
    for vid in view_ids {
        q = q.bind(vid.clone());
    }
    let rows = q
        .fetch_all(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询视图失败: {}", e)))?;

    let mut map = serde_json::Map::new();
    for row in &rows {
        let desc = ViewRow::from_row(row).to_descriptor();
        let key = desc.view_type.as_str().to_string();
        if !map.contains_key(&key) {
            map.insert(
                key,
                serde_json::to_value(&desc).unwrap_or(serde_json::Value::Null),
            );
        }
    }
    Ok(serde_json::Value::Object(map))
}

async fn fetch_views_by_model(
    state: &AppState,
    res_model: &str,
) -> Result<serde_json::Value, AppError> {
    let sql = state.dialect.prepare(&format!(
        "SELECT {} FROM ir_view WHERE model = ? AND active = 1 ORDER BY priority",
        VIEW_COLUMNS
    ));
    let rows = sqlx::query(&sql)
        .bind(res_model)
        .fetch_all(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询视图失败: {}", e)))?;

    let mut map = serde_json::Map::new();
    for row in &rows {
        let desc = ViewRow::from_row(row).to_descriptor();
        let key = desc.view_type.as_str().to_string();
        if !map.contains_key(&key) {
            map.insert(
                key,
                serde_json::to_value(&desc).unwrap_or(serde_json::Value::Null),
            );
        }
    }
    Ok(serde_json::Value::Object(map))
}

// ==================== 请求类型 ====================

#[derive(Deserialize)]
pub struct CreateActionRequest {
    pub name: String,
    pub action_type: ActionType,
    pub res_model: Option<String>,
    pub view_mode: Vec<ViewType>,
    pub view_ids: Vec<String>,
    pub domain: Option<serde_json::Value>,
    pub context: Option<serde_json::Value>,
    pub limit: Option<i32>,
    pub target: Option<String>,
    pub url: Option<String>,
    pub help: Option<String>,
    pub group_ids: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct UpdateActionRequest {
    pub name: Option<String>,
    pub res_model: Option<String>,
    pub view_mode: Option<Vec<ViewType>>,
    pub view_ids: Option<Vec<String>>,
    pub domain: Option<serde_json::Value>,
    pub context: Option<serde_json::Value>,
    pub limit: Option<i32>,
    pub target: Option<String>,
    pub help: Option<String>,
    pub group_ids: Option<Vec<String>>,
}
