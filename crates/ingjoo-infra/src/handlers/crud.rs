use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::Deserialize;

use ingjoo_core::db::traits::IngjooStore;
use ingjoo_core::query::domain::{Domain, SqlCondition};
use ingjoo_security::{AccessOp, ModelAccess, RecordRule, SecurityPolicy};

use crate::db::generic::{GenericDb, GenericRecordStore};
use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub domain: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn resolve_model(state: &AppState, model_name: &str) -> Result<ingjoo_core::module::ModelDescriptor, AppError> {
    state.registry.get(model_name)
        .ok_or_else(|| AppError::NotFound(format!("模型 '{}' 未注册", model_name)))
}

fn check_access(policy: &SecurityPolicy, model: &str, groups: &[String], op: AccessOp) -> Result<(), AppError> {
    let op_label = format!("{:?}", op).to_lowercase();
    if !policy.check_access_groups(model, groups, op) {
        return Err(AppError::Forbidden(format!("无权对 '{}' 执行 {} 操作", model, op_label)));
    }
    Ok(())
}

async fn check_access_with_audit(
    policy: &SecurityPolicy,
    model: &str,
    groups: &[String],
    op: AccessOp,
    store: &Arc<dyn IngjooStore>,
    user_id: &str,
) -> Result<(), AppError> {
    let op_label = format!("{:?}", op).to_lowercase();
    if !policy.check_access_groups(model, groups, op) {
        let _ = store.create_audit_log(
            Some(user_id),
            "access_denied",
            model,
            None,
            Some(serde_json::json!({ "op": op_label, "groups": groups })),
            None,
        ).await;
        return Err(AppError::Forbidden(format!("无权对 '{}' 执行 {} 操作", model, op_label)));
    }
    Ok(())
}

/// 从数据库加载当前用户的权限策略（带缓存）
async fn load_security_policy(state: &AppState, groups: &[String]) -> Result<SecurityPolicy, AppError> {
    if groups.is_empty() {
        return Ok(SecurityPolicy::new());
    }

    // 按排序后的 groups 构建缓存键，确保相同组合命中同一缓存
    let mut sorted_groups = groups.to_vec();
    sorted_groups.sort();
    let cache_key = sorted_groups.join(":");

    if let Some(cached) = state.cache.get_scope_access("policy", &cache_key) {
        return Ok(cached);
    }

    // 并行加载 model_access + record_rules
    let (access_rows, rule_rows) = tokio::try_join!(
        state.store.get_model_accesses_for_groups(groups),
        state.store.get_record_rules_for_groups(groups),
    ).map_err(|e| AppError::Internal(anyhow::anyhow!("加载权限失败: {}", e)))?;

    let mut policy = SecurityPolicy::new();

    for row in access_rows {
        policy.add_model_access(ModelAccess {
            model: row.model,
            role: row.group_id.to_string(),
            read: row.perm_read,
            write: row.perm_write,
            create: row.perm_create,
            delete: row.perm_delete,
            import: row.perm_import,
            export: row.perm_export,
        });
    }

    for row in rule_rows {
        let domain = Domain::from_json(&row.domain)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Record rule domain 解析失败: {}", e)))?;
        policy.add_record_rule(RecordRule {
            model: row.model,
            role: row.group_id.to_string(),
            domain,
            perm_read: row.perm_read,
            perm_write: row.perm_write,
            perm_create: row.perm_create,
            perm_delete: row.perm_delete,
        });
    }

    state.cache.put_scope_access("policy", &cache_key, policy.clone());

    Ok(policy)
}

fn get_record_filter(
    policy: &SecurityPolicy,
    model: &str,
    groups: &[String],
    user_id: &str,
    op: AccessOp,
    dialect: &ingjoo_core::Dialect,
) -> Option<SqlCondition> {
    let filter = policy.record_filter_groups_with_dialect(model, groups, user_id, &op, Some(dialect));
    if filter.clause.is_empty() {
        None
    } else {
        Some(filter)
    }
}

pub async fn crud_list(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Json<ingjoo_core::PaginatedResult<serde_json::Value>>, AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(&policy, &model_name, &current_user.groups, AccessOp::Read, &state.store, &current_user.user_id).await?;

    let domain = params.domain.as_deref()
        .map(Domain::from_json)
        .transpose()
        .map_err(|e| AppError::BadRequest(format!("Domain 解析错误: {}", e)))?;

    let record_filter = get_record_filter(
        &policy, &model_name, &current_user.groups, &current_user.user_id, AccessOp::Read, &state.dialect,
    );

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let generic = GenericDb::new(&state.pool, &state.dialect);
    let result = generic.generic_list_with_filter(&model, domain.as_ref(), record_filter.as_ref(), limit, offset).await?;
    Ok(Json(result))
}

pub async fn crud_read(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path((model_name, id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(&policy, &model_name, &current_user.groups, AccessOp::Read, &state.store, &current_user.user_id).await?;

    let record_filter = get_record_filter(
        &policy, &model_name, &current_user.groups, &current_user.user_id, AccessOp::Read, &state.dialect,
    );

    let generic = GenericDb::new(&state.pool, &state.dialect);
    let record = generic.generic_read_with_filter(&model, &id, record_filter.as_ref()).await?;
    match record {
        Some(r) => Ok(Json(r)),
        None => {
            if record_filter.is_some() {
                let _ = state.store.create_audit_log(
                    Some(&current_user.user_id),
                    "record_filter_denied",
                    &model_name,
                    Some(&id),
                    None,
                    None,
                ).await;
            }
            Err(AppError::NotFound("记录不存在".into()))
        }
    }
}

pub async fn crud_create(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(&policy, &model_name, &current_user.groups, AccessOp::Create, &state.store, &current_user.user_id).await?;

    let generic = GenericDb::new(&state.pool, &state.dialect);
    let record = generic.generic_create(&model, &data, Some(&current_user.user_id)).await?;
    Ok((StatusCode::CREATED, Json(record)))
}

pub async fn crud_update(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path((model_name, id)): Path<(String, String)>,
    Json(data): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(&policy, &model_name, &current_user.groups, AccessOp::Write, &state.store, &current_user.user_id).await?;

    let record_filter = get_record_filter(
        &policy, &model_name, &current_user.groups, &current_user.user_id, AccessOp::Write, &state.dialect,
    );

    let generic = GenericDb::new(&state.pool, &state.dialect);
    let existing = generic.generic_read_with_filter(&model, &id, record_filter.as_ref()).await?;
    match existing {
        Some(_) => {},
        None => {
            if record_filter.is_some() {
                let _ = state.store.create_audit_log(
                    Some(&current_user.user_id),
                    "record_filter_denied",
                    &model_name,
                    Some(&id),
                    None,
                    None,
                ).await;
            }
            return Err(AppError::NotFound("记录不存在或无权修改".into()));
        }
    }

    let record = generic.generic_update(&model, &id, &data, Some(&current_user.user_id)).await?
        .ok_or_else(|| AppError::NotFound("记录不存在".into()))?;
    Ok(Json(record))
}

pub async fn crud_delete(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path((model_name, id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(&policy, &model_name, &current_user.groups, AccessOp::Delete, &state.store, &current_user.user_id).await?;

    let record_filter = get_record_filter(
        &policy, &model_name, &current_user.groups, &current_user.user_id, AccessOp::Delete, &state.dialect,
    );

    let generic = GenericDb::new(&state.pool, &state.dialect);
    let existing = generic.generic_read_with_filter(&model, &id, record_filter.as_ref()).await?;
    match existing {
        Some(_) => {},
        None => {
            if record_filter.is_some() {
                let _ = state.store.create_audit_log(
                    Some(&current_user.user_id),
                    "record_filter_denied",
                    &model_name,
                    Some(&id),
                    None,
                    None,
                ).await;
            }
            return Err(AppError::NotFound("记录不存在或无权删除".into()));
        }
    }

    let deleted = generic.generic_delete(&model, &id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("记录不存在".into()))
    }
}

pub async fn crud_ensure_table(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
) -> Result<StatusCode, AppError> {
    let model = resolve_model(&state, &model_name)?;
    if !current_user.is_admin() {
        let _ = state.store.create_audit_log(
            Some(&current_user.user_id),
            "admin_required_denied",
            &model_name,
            None,
            None,
            None,
        ).await;
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    let generic = GenericDb::new(&state.pool, &state.dialect);
    generic.ensure_table(&model).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn crud_models(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ingjoo_core::module::ModelDescriptor>>, AppError> {
    if !current_user.is_admin() {
        let _ = state.store.create_audit_log(
            Some(&current_user.user_id),
            "admin_required_denied",
            "models",
            None,
            None,
            None,
        ).await;
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    Ok(Json(state.registry.list()))
}
