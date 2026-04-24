//! 菜单 CRUD 处理器 — 树构建、分组可见性过滤

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::Deserialize;
use sqlx::Row;

use ingjoo_core::MenuDescriptor;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

// ==================== 权限检查 ====================

fn require_admin(user: &CurrentUser) -> Result<(), AppError> {
    if user.is_admin() {
        Ok(())
    } else {
        Err(AppError::Forbidden("需要管理员权限".into()))
    }
}

// ==================== 行提取 ====================

/// 从 AnyRow 提取菜单数据
fn row_to_menu(row: &sqlx::any::AnyRow) -> MenuDescriptor {
    let id: String = row.get("id");
    let name: String = row.get("name");
    let parent_id: Option<String> = row.try_get("parent_id").ok().flatten();
    let sequence: i32 = row.try_get("sequence").unwrap_or(10);
    let action_id: Option<String> = row.try_get("action_id").ok().flatten();
    let web_icon: Option<String> = row.try_get("web_icon").ok().flatten();
    let group_ids_str: String = row.try_get("group_ids").ok().unwrap_or_default();
    let group_ids: Vec<String> = serde_json::from_str(&group_ids_str).unwrap_or_default();

    MenuDescriptor {
        id,
        name,
        parent_id,
        sequence,
        action_id,
        web_icon,
        active: true,
        group_ids,
        children: vec![],
    }
}

// ==================== 公开 API ====================

/// GET /api/menus — 返回当前用户可见的菜单树
pub async fn list_menus(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sql = state.dialect.prepare(
        "SELECT id, name, parent_id, sequence, action_id, web_icon, group_ids FROM ir_menu WHERE active = 1 ORDER BY sequence",
    );
    let rows = sqlx::query(&sql)
        .fetch_all(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询菜单失败: {}", e)))?;

    // 构建描述符列表
    let all_menus: Vec<MenuDescriptor> = rows.iter().map(row_to_menu).collect();

    // 按用户分组过滤可见性（空 group_ids = 所有人可见）
    let visible: Vec<MenuDescriptor> = all_menus
        .into_iter()
        .filter(|m| m.group_ids.is_empty() || m.group_ids.iter().any(|g| current_user.groups.contains(g)))
        .collect();

    // 构建树结构
    let tree = build_menu_tree(&visible, None);

    Ok(Json(serde_json::json!({ "menus": tree })))
}

/// POST /api/menus — 创建菜单（管理员）
pub async fn create_menu(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateMenuRequest>,
) -> Result<(StatusCode, Json<MenuDescriptor>), AppError> {
    require_admin(&current_user)?;

    let id = uuid::Uuid::new_v4().to_string();
    let sequence = req.sequence.unwrap_or(10);
    let group_ids = req.group_ids.unwrap_or_default();
    let group_ids_json = serde_json::to_string(&group_ids).unwrap_or_else(|_| "[]".into());

    let sql = state.dialect.prepare(
        "INSERT INTO ir_menu (id, name, parent_id, sequence, action_id, web_icon, group_ids, active) VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
    );
    sqlx::query(&sql)
        .bind(&id)
        .bind(&req.name)
        .bind(&req.parent_id)
        .bind(sequence)
        .bind(&req.action_id)
        .bind(&req.web_icon)
        .bind(&group_ids_json)
        .execute(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("创建菜单失败: {}", e)))?;

    let desc = MenuDescriptor {
        id,
        name: req.name,
        parent_id: req.parent_id,
        sequence,
        action_id: req.action_id,
        web_icon: req.web_icon,
        active: true,
        group_ids,
        children: vec![],
    };

    Ok((StatusCode::CREATED, Json(desc)))
}

/// PUT /api/menus/{id} — 更新菜单（管理员）
pub async fn update_menu(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMenuRequest>,
) -> Result<Json<MenuDescriptor>, AppError> {
    require_admin(&current_user)?;

    // 查询现有记录
    let sql = state.dialect.prepare(
        "SELECT id, name, parent_id, sequence, action_id, web_icon, group_ids FROM ir_menu WHERE id = ?",
    );
    let row = sqlx::query(&sql)
        .bind(&id)
        .fetch_optional(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("查询菜单失败: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("菜单 '{}' 不存在", id)))?;

    let existing = row_to_menu(&row);

    // 合并字段
    let name = req.name.unwrap_or(existing.name);
    let parent_id = match req.parent_id {
        Some(v) => v,               // 显式传了值（含 None）就用新值
        None => existing.parent_id, // 没传就保留原值
    };
    let sequence = req.sequence.unwrap_or(existing.sequence);
    let action_id = match req.action_id {
        Some(v) => v,
        None => existing.action_id,
    };
    let web_icon = match req.web_icon {
        Some(v) => v,
        None => existing.web_icon,
    };
    let group_ids = req.group_ids.unwrap_or(existing.group_ids);
    let group_ids_json = serde_json::to_string(&group_ids).unwrap_or_else(|_| "[]".into());

    let sql = state.dialect.prepare(
        "UPDATE ir_menu SET name = ?, parent_id = ?, sequence = ?, action_id = ?, web_icon = ?, group_ids = ?, updated_at = datetime('now') WHERE id = ?",
    );
    sqlx::query(&sql)
        .bind(&name)
        .bind(&parent_id)
        .bind(sequence)
        .bind(&action_id)
        .bind(&web_icon)
        .bind(&group_ids_json)
        .bind(&id)
        .execute(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("更新菜单失败: {}", e)))?;

    Ok(Json(MenuDescriptor {
        id,
        name,
        parent_id,
        sequence,
        action_id,
        web_icon,
        active: true,
        group_ids,
        children: vec![],
    }))
}

/// DELETE /api/menus/{id} — 删除菜单（管理员）
pub async fn delete_menu(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user)?;

    let sql = state.dialect.prepare("DELETE FROM ir_menu WHERE id = ?");
    let result = sqlx::query(&sql)
        .bind(&id)
        .execute(&*state.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("删除菜单失败: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("菜单 '{}' 不存在", id)));
    }
    Ok(StatusCode::NO_CONTENT)
}

// ==================== 请求类型 ====================

#[derive(Deserialize)]
pub struct CreateMenuRequest {
    pub name: String,
    pub parent_id: Option<String>,
    pub sequence: Option<i32>,
    pub action_id: Option<String>,
    pub web_icon: Option<String>,
    pub group_ids: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct UpdateMenuRequest {
    pub name: Option<String>,
    pub parent_id: Option<Option<String>>,
    pub sequence: Option<i32>,
    pub action_id: Option<Option<String>>,
    pub web_icon: Option<Option<String>>,
    pub group_ids: Option<Vec<String>>,
}

// ==================== 树构建 ====================

/// 递归构建菜单树（按 parent_id 关系）
fn build_menu_tree(menus: &[MenuDescriptor], parent_id: Option<&str>) -> Vec<MenuDescriptor> {
    menus
        .iter()
        .filter(|m| m.parent_id.as_deref() == parent_id)
        .map(|m| {
            let mut desc = m.clone();
            desc.children = build_menu_tree(menus, Some(&m.id));
            desc
        })
        .collect()
}
