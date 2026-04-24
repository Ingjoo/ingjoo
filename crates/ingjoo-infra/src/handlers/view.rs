use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::Deserialize;
use sqlx::Row;

use ingjoo_core::ViewDescriptor;
use ingjoo_core::ViewType;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

fn require_admin(user: &CurrentUser) -> Result<(), AppError> {
    if user.is_admin() {
        Ok(())
    } else {
        Err(AppError::Forbidden("需要管理员权限".into()))
    }
}

pub const VIEW_COLUMNS: &str = "id, name, model, type, priority, arch, inherit_id, active, group_ids";

/// 视图行数据（供 action.rs 复用）
pub struct ViewRow {
    pub id: String,
    pub name: String,
    pub model: String,
    pub view_type_str: String,
    pub priority: i32,
    pub arch_str: String,
    pub inherit_id: Option<String>,
    pub active: bool,
    pub group_ids_str: String,
}

impl ViewRow {
    pub fn from_row(row: &sqlx::any::AnyRow) -> Self {
        let active_int: i32 = row.try_get("active").unwrap_or(1);
        Self {
            id: row.get("id"),
            name: row.get("name"),
            model: row.get("model"),
            view_type_str: row.try_get("type").ok().unwrap_or_else(|| "form".into()),
            priority: row.try_get("priority").ok().unwrap_or(16),
            arch_str: row.try_get("arch").ok().unwrap_or_else(|| "{}".into()),
            inherit_id: row.try_get("inherit_id").ok().flatten(),
            active: active_int != 0,
            group_ids_str: row.try_get("group_ids").ok().unwrap_or_default(),
        }
    }

    pub fn to_descriptor(&self) -> ViewDescriptor {
        let view_type = parse_view_type(&self.view_type_str);
        let arch: serde_json::Value =
            serde_json::from_str(&self.arch_str).unwrap_or(serde_json::json!({}));
        let group_ids: Vec<String> =
            serde_json::from_str(&self.group_ids_str).unwrap_or_default();

        ViewDescriptor {
            id: self.id.clone(),
            name: self.name.clone(),
            model: self.model.clone(),
            view_type,
            priority: self.priority,
            arch,
            inherit_id: self.inherit_id.clone(),
            active: self.active,
            group_ids,
        }
    }
}

fn parse_view_type(s: &str) -> ViewType {
    match s {
        "form" => ViewType::Form,
        "list" => ViewType::List,
        "kanban" => ViewType::Kanban,
        "search" => ViewType::Search,
        "graph" => ViewType::Graph,
        "calendar" => ViewType::Calendar,
        _ => ViewType::Form,
    }
}

#[derive(Deserialize)]
pub struct ViewQueryParams {
    pub model: Option<String>,
    pub r#type: Option<String>,
}

/// GET /api/views — 查询视图（支持 ?model=X&type=list 过滤）
pub async fn list_views(
    Extension(_current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ViewQueryParams>,
) -> Result<Json<Vec<ViewDescriptor>>, AppError> {
    let (sql_str, has_model, has_type) = match (&params.model, &params.r#type) {
        (Some(_), Some(_)) => (
            format!("SELECT {} FROM ir_view WHERE active = 1 AND model = ? AND type = ? ORDER BY priority", VIEW_COLUMNS),
            true, true,
        ),
        (Some(_), None) => (
            format!("SELECT {} FROM ir_view WHERE active = 1 AND model = ? ORDER BY type, priority", VIEW_COLUMNS),
            true, false,
        ),
        (None, Some(_)) => (
            format!("SELECT {} FROM ir_view WHERE active = 1 AND type = ? ORDER BY model, priority", VIEW_COLUMNS),
            false, true,
        ),
        (None, None) => (
            format!("SELECT {} FROM ir_view WHERE active = 1 ORDER BY model, type, priority", VIEW_COLUMNS),
            false, false,
        ),
    };

    let sql = state.dialect.prepare(&sql_str);
    let mut q = sqlx::query(&sql);
    if has_model {
        q = q.bind(params.model.as_deref().unwrap());
    }
    if has_type {
        q = q.bind(params.r#type.as_deref().unwrap());
    }

    let rows = q
        .fetch_all(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询视图失败: {}", e)))?;

    let descriptors: Vec<ViewDescriptor> = rows.iter().map(|r| ViewRow::from_row(r).to_descriptor()).collect();
    Ok(Json(descriptors))
}

/// GET /api/views/{id} — 获取单个视图
pub async fn get_view(
    Extension(_current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ViewDescriptor>, AppError> {
    let sql = state.dialect.prepare(&format!(
        "SELECT {} FROM ir_view WHERE id = ?",
        VIEW_COLUMNS
    ));
    let row = sqlx::query(&sql)
        .bind(&id)
        .fetch_optional(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询视图失败: {}", e)))?;

    match row {
        Some(r) => Ok(Json(ViewRow::from_row(&r).to_descriptor())),
        None => Err(AppError::NotFound(format!("视图 '{}' 不存在", id))),
    }
}

/// POST /api/views — 创建视图（管理员）
pub async fn create_view(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateViewRequest>,
) -> Result<(StatusCode, Json<ViewDescriptor>), AppError> {
    require_admin(&current_user)?;

    let id = uuid::Uuid::new_v4().to_string();
    let priority = req.priority.unwrap_or(16);
    let arch_str = serde_json::to_string(&req.arch).unwrap_or_else(|_| "{}".into());
    let group_ids = req.group_ids.unwrap_or_default();
    let group_ids_str = serde_json::to_string(&group_ids).unwrap_or_else(|_| "[]".into());

    let sql = state.dialect.prepare(
        "INSERT INTO ir_view (id, name, model, type, priority, arch, inherit_id, group_ids, active) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1)",
    );
    sqlx::query(&sql)
        .bind(&id)
        .bind(&req.name)
        .bind(&req.model)
        .bind(req.view_type.as_str())
        .bind(priority)
        .bind(&arch_str)
        .bind(&req.inherit_id)
        .bind(&group_ids_str)
        .execute(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("创建视图失败: {}", e)))?;

    let desc = ViewDescriptor {
        id,
        name: req.name,
        model: req.model,
        view_type: req.view_type,
        priority,
        arch: req.arch,
        inherit_id: req.inherit_id,
        active: true,
        group_ids,
    };

    Ok((StatusCode::CREATED, Json(desc)))
}

/// PUT /api/views/{id} — 更新视图（管理员）
pub async fn update_view(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateViewRequest>,
) -> Result<Json<ViewDescriptor>, AppError> {
    require_admin(&current_user)?;

    // 查询现有记录
    let sql = state.dialect.prepare(&format!(
        "SELECT {} FROM ir_view WHERE id = ?",
        VIEW_COLUMNS
    ));
    let row = sqlx::query(&sql)
        .bind(&id)
        .fetch_optional(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询视图失败: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("视图 '{}' 不存在", id)))?;

    let existing = ViewRow::from_row(&row);

    // 合并字段
    let name = req.name.unwrap_or(existing.name);
    let view_type_str = req
        .view_type
        .map(|t| t.as_str().to_string())
        .unwrap_or(existing.view_type_str);
    let priority = req.priority.unwrap_or(existing.priority);
    let arch = req
        .arch
        .unwrap_or_else(|| serde_json::from_str(&existing.arch_str).unwrap_or(serde_json::json!({})));
    let arch_str = serde_json::to_string(&arch).unwrap_or_else(|_| "{}".into());
    let group_ids = req
        .group_ids
        .unwrap_or_else(|| serde_json::from_str(&existing.group_ids_str).unwrap_or_default());
    let group_ids_str = serde_json::to_string(&group_ids).unwrap_or_else(|_| "[]".into());

    let sql = state.dialect.prepare(
        "UPDATE ir_view SET name = ?, type = ?, priority = ?, arch = ?, group_ids = ?, updated_at = datetime('now') WHERE id = ?",
    );
    sqlx::query(&sql)
        .bind(&name)
        .bind(&view_type_str)
        .bind(priority)
        .bind(&arch_str)
        .bind(&group_ids_str)
        .bind(&id)
        .execute(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("更新视图失败: {}", e)))?;

    Ok(Json(ViewDescriptor {
        id,
        name,
        model: existing.model,
        view_type: parse_view_type(&view_type_str),
        priority,
        arch,
        inherit_id: existing.inherit_id,
        active: true,
        group_ids,
    }))
}

/// DELETE /api/views/{id} — 删除视图（管理员）
pub async fn delete_view(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user)?;

    let sql = state.dialect.prepare("DELETE FROM ir_view WHERE id = ?");
    let result = sqlx::query(&sql)
        .bind(&id)
        .execute(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("删除视图失败: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("视图 '{}' 不存在", id)));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct CreateViewRequest {
    pub name: String,
    pub model: String,
    pub view_type: ViewType,
    pub priority: Option<i32>,
    pub arch: serde_json::Value,
    pub inherit_id: Option<String>,
    pub group_ids: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct UpdateViewRequest {
    pub name: Option<String>,
    pub view_type: Option<ViewType>,
    pub priority: Option<i32>,
    pub arch: Option<serde_json::Value>,
    pub group_ids: Option<Vec<String>>,
}
