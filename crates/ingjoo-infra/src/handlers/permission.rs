use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::Deserialize;

use ingjoo_core::db::ids::GroupId;
use ingjoo_core::db::models::{ModelAccessRow, RecordRuleRow, UpsertModelAccessRequest, UpsertRecordRuleRequest};
use ingjoo_core::db::traits::IngjooStore;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

async fn require_admin(user: &CurrentUser, store: &Arc<dyn IngjooStore>) -> Result<(), AppError> {
    if !user.is_admin() {
        let _ =
            store.create_audit_log(Some(&user.user_id), "admin_required_denied", "permissions", None, None, None).await;
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    Ok(())
}

// ==================== 查询参数 ====================

#[derive(Debug, Deserialize)]
pub struct GroupFilter {
    pub group_id: Option<String>,
}

// ==================== Model Access CRUD ====================

pub async fn list_model_accesses(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<GroupFilter>,
) -> Result<Json<Vec<ModelAccessRow>>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let group_id = filter.group_id.map(GroupId::new);
    let accesses = state.store.list_model_accesses(group_id.as_ref()).await?;
    Ok(Json(accesses))
}

pub async fn create_model_access(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertModelAccessRequest>,
) -> Result<(StatusCode, Json<ModelAccessRow>), AppError> {
    require_admin(&current_user, &state.store).await?;
    let access = ModelAccessRow {
        id: uuid::Uuid::new_v4().to_string(),
        group_id: req.group_id,
        model: req.model,
        perm_read: req.perm_read,
        perm_write: req.perm_write,
        perm_create: req.perm_create,
        perm_delete: req.perm_delete,
        perm_import: req.perm_import,
        perm_export: req.perm_export,
    };
    let access = state.store.create_model_access(&access).await?;
    state.cache.invalidate_scope("policy");
    Ok((StatusCode::CREATED, Json(access)))
}

pub async fn get_model_access(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ModelAccessRow>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let access = state.store.get_model_access(&id).await?.ok_or_else(|| AppError::NotFound("权限规则不存在".into()))?;
    Ok(Json(access))
}

pub async fn update_model_access(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpsertModelAccessRequest>,
) -> Result<Json<ModelAccessRow>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let access = ModelAccessRow {
        id: id.clone(),
        group_id: req.group_id,
        model: req.model,
        perm_read: req.perm_read,
        perm_write: req.perm_write,
        perm_create: req.perm_create,
        perm_delete: req.perm_delete,
        perm_import: req.perm_import,
        perm_export: req.perm_export,
    };
    let access = state
        .store
        .update_model_access(&id, &access)
        .await?
        .ok_or_else(|| AppError::NotFound("权限规则不存在".into()))?;
    state.cache.invalidate_scope("policy");
    Ok(Json(access))
}

pub async fn delete_model_access(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user, &state.store).await?;
    let deleted = state.store.delete_model_access(&id).await?;
    if deleted {
        state.cache.invalidate_scope("policy");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("权限规则不存在".into()))
    }
}

// ==================== Record Rule CRUD ====================

pub async fn list_record_rules(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<GroupFilter>,
) -> Result<Json<Vec<RecordRuleRow>>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let group_id = filter.group_id.map(GroupId::new);
    let rules = state.store.list_record_rules(group_id.as_ref()).await?;
    Ok(Json(rules))
}

pub async fn create_record_rule(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertRecordRuleRequest>,
) -> Result<(StatusCode, Json<RecordRuleRow>), AppError> {
    require_admin(&current_user, &state.store).await?;
    let rule = RecordRuleRow {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name,
        group_id: req.group_id,
        model: req.model,
        domain: req.domain,
        perm_read: req.perm_read,
        perm_write: req.perm_write,
        perm_create: req.perm_create,
        perm_delete: req.perm_delete,
    };
    let rule = state.store.create_record_rule(&rule).await?;
    state.cache.invalidate_scope("policy");
    Ok((StatusCode::CREATED, Json(rule)))
}

pub async fn get_record_rule(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RecordRuleRow>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let rule = state.store.get_record_rule(&id).await?.ok_or_else(|| AppError::NotFound("记录规则不存在".into()))?;
    Ok(Json(rule))
}

pub async fn update_record_rule(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpsertRecordRuleRequest>,
) -> Result<Json<RecordRuleRow>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let rule = RecordRuleRow {
        id: id.clone(),
        name: req.name,
        group_id: req.group_id,
        model: req.model,
        domain: req.domain,
        perm_read: req.perm_read,
        perm_write: req.perm_write,
        perm_create: req.perm_create,
        perm_delete: req.perm_delete,
    };
    let rule =
        state.store.update_record_rule(&id, &rule).await?.ok_or_else(|| AppError::NotFound("记录规则不存在".into()))?;
    state.cache.invalidate_scope("policy");
    Ok(Json(rule))
}

pub async fn delete_record_rule(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user, &state.store).await?;
    let deleted = state.store.delete_record_rule(&id).await?;
    if deleted {
        state.cache.invalidate_scope("policy");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("记录规则不存在".into()))
    }
}
