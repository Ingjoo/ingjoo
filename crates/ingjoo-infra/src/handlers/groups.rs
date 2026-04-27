use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;

use ingjoo_core::db::error::StoreError;
use ingjoo_core::db::ids::GroupId;
use ingjoo_core::db::models::{Group, UpsertGroupRequest};
use ingjoo_core::db::traits::IngjooStore;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

// ==================== 权限检查辅助 ====================

async fn require_admin(user: &CurrentUser, store: &Arc<dyn IngjooStore>) -> Result<(), AppError> {
    if !user.is_admin() {
        let _ = store.create_audit_log(Some(&user.user_id), "admin_required_denied", "groups", None, None, None).await;
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    Ok(())
}

// ==================== Groups CRUD ====================

pub async fn list_groups(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Group>>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let groups = state.store.list_groups().await?;
    Ok(Json(groups))
}

pub async fn create_group(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertGroupRequest>,
) -> Result<(StatusCode, Json<Group>), AppError> {
    require_admin(&current_user, &state.store).await?;
    let group = Group {
        id: GroupId::new(uuid::Uuid::new_v4().to_string()),
        name: req.name,
        display_name: req.display_name,
        comment: req.comment,
        created_at: String::new(),
        updated_at: String::new(),
    };
    let group = state.store.create_group(&group).await.map_err(|e| match e {
        StoreError::UniqueViolation { .. } => AppError::Conflict("组名已存在".into()),
        other => AppError::from(other),
    })?;
    if !req.implied_group_ids.is_empty() {
        state.store.set_implied_groups(&group.id, &req.implied_group_ids).await?;
    }
    Ok((StatusCode::CREATED, Json(group)))
}

pub async fn get_group(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<GroupId>,
) -> Result<Json<Group>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let group = state.store.get_group(&id).await?.ok_or_else(|| AppError::NotFound("组不存在".into()))?;
    Ok(Json(group))
}

pub async fn update_group(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<GroupId>,
    Json(req): Json<UpsertGroupRequest>,
) -> Result<Json<Group>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let group = state
        .store
        .update_group(&id, req.display_name.as_deref(), req.comment.as_deref())
        .await?
        .ok_or_else(|| AppError::NotFound("组不存在".into()))?;
    state.store.set_implied_groups(&id, &req.implied_group_ids).await?;
    Ok(Json(group))
}

pub async fn delete_group(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<GroupId>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user, &state.store).await?;
    let deleted = state.store.delete_group(&id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("组不存在".into()))
    }
}

// ==================== 组继承关系 ====================

pub async fn get_implied_groups(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<GroupId>,
) -> Result<Json<Vec<ingjoo_core::db::models::GroupImplied>>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let implied = state.store.get_implied_groups(&id).await?;
    Ok(Json(implied))
}

// ==================== 用户-组关系 ====================

pub async fn get_user_groups(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> Result<Json<Vec<Group>>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let groups = state.store.get_user_groups(&ingjoo_core::db::ids::UserId::new(user_id)).await?;
    Ok(Json(groups))
}

pub async fn set_user_groups(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    Json(group_ids): Json<Vec<GroupId>>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user, &state.store).await?;
    state.store.set_user_groups(&ingjoo_core::db::ids::UserId::new(user_id), &group_ids).await?;
    Ok(StatusCode::NO_CONTENT)
}
