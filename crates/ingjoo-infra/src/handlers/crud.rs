use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::Deserialize;

use ingjoo_core::db::traits::IngjooStore;
use ingjoo_core::extension::search::SearchQuery;
use ingjoo_core::module::FieldType;
use ingjoo_core::query::domain::{Domain, DomainOp, DomainValue, SqlCondition};
use ingjoo_security::{AccessOp, ModelAccess, RecordRule, SecurityPolicy};

use crate::db::generic::{GenericDb, GenericRecordStore};
use crate::extractors::CurrentUser;
use crate::middleware::database_selector::ResolvedDatabase;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub domain: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn resolve_model(state: &AppState, model_name: &str) -> Result<ingjoo_core::module::ModelDescriptor, AppError> {
    state.registry.get(model_name).ok_or_else(|| AppError::NotFound(format!("模型 '{}' 未注册", model_name)))
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
        let _ = store
            .create_audit_log(
                Some(user_id),
                "access_denied",
                model,
                None,
                Some(serde_json::json!({ "op": op_label, "groups": groups })),
                None,
            )
            .await;
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
    )
    .map_err(|e| {
        tracing::error!("加载权限失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("加载权限失败"))
    })?;

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
        let domain = Domain::from_json(&row.domain).map_err(|e| {
            tracing::error!("Record rule domain 解析失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("Record rule domain 解析失败"))
        })?;
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
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Json<ingjoo_core::PaginatedResult<serde_json::Value>>, AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(
        &policy,
        &model_name,
        &current_user.groups,
        AccessOp::Read,
        &state.store,
        &current_user.user_id,
    )
    .await?;

    let domain = params
        .domain
        .as_deref()
        .map(Domain::from_json)
        .transpose()
        .map_err(|e| AppError::BadRequest(format!("Domain 解析错误: {}", e)))?;

    let record_filter = get_record_filter(
        &policy,
        &model_name,
        &current_user.groups,
        &current_user.user_id,
        AccessOp::Read,
        &resolved_db.dialect,
    );

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let generic = GenericDb::new(&resolved_db.pool, &resolved_db.dialect);
    let result =
        generic.generic_list_with_filter(&model, domain.as_ref(), record_filter.as_ref(), limit, offset).await?;
    Ok(Json(result))
}

pub async fn crud_read(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path((model_name, id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(
        &policy,
        &model_name,
        &current_user.groups,
        AccessOp::Read,
        &state.store,
        &current_user.user_id,
    )
    .await?;

    let record_filter = get_record_filter(
        &policy,
        &model_name,
        &current_user.groups,
        &current_user.user_id,
        AccessOp::Read,
        &resolved_db.dialect,
    );

    let generic = GenericDb::new(&resolved_db.pool, &resolved_db.dialect);
    let record = generic.generic_read_with_filter(&model, &id, record_filter.as_ref()).await?;
    match record {
        Some(r) => Ok(Json(r)),
        None => {
            if record_filter.is_some() {
                let _ = state
                    .store
                    .create_audit_log(
                        Some(&current_user.user_id),
                        "record_filter_denied",
                        &model_name,
                        Some(&id),
                        None,
                        None,
                    )
                    .await;
            }
            Err(AppError::NotFound("记录不存在".into()))
        }
    }
}

pub async fn crud_create(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(
        &policy,
        &model_name,
        &current_user.groups,
        AccessOp::Create,
        &state.store,
        &current_user.user_id,
    )
    .await?;

    let generic = GenericDb::new(&resolved_db.pool, &resolved_db.dialect);
    let record = generic.generic_create(&model, &data, Some(&current_user.user_id)).await?;

    let _ = state
        .audit
        .create_audit_log(
            Some(&current_user.user_id),
            "create",
            &model_name,
            record.get("id").and_then(|v| v.as_str()),
            Some(data.clone()),
            None,
        )
        .await;

    // 后台索引（失败不影响主流程）
    if let Some(id) = record.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()) {
        let search = state.search.clone();
        let model_name_idx = model_name.clone();
        let data_idx = record.clone();
        tokio::spawn(async move {
            let _ = search.index_record(&model_name_idx, &id, &data_idx).await;
        });
    }

    Ok((StatusCode::CREATED, Json(record)))
}

pub async fn crud_update(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path((model_name, id)): Path<(String, String)>,
    Json(data): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(
        &policy,
        &model_name,
        &current_user.groups,
        AccessOp::Write,
        &state.store,
        &current_user.user_id,
    )
    .await?;

    let record_filter = get_record_filter(
        &policy,
        &model_name,
        &current_user.groups,
        &current_user.user_id,
        AccessOp::Write,
        &resolved_db.dialect,
    );

    let generic = GenericDb::new(&resolved_db.pool, &resolved_db.dialect);
    let existing = generic.generic_read_with_filter(&model, &id, record_filter.as_ref()).await?;
    match existing {
        Some(_) => {}
        None => {
            if record_filter.is_some() {
                let _ = state
                    .store
                    .create_audit_log(
                        Some(&current_user.user_id),
                        "record_filter_denied",
                        &model_name,
                        Some(&id),
                        None,
                        None,
                    )
                    .await;
            }
            return Err(AppError::NotFound("记录不存在或无权修改".into()));
        }
    }

    let record = generic
        .generic_update(&model, &id, &data, Some(&current_user.user_id))
        .await?
        .ok_or_else(|| AppError::NotFound("记录不存在".into()))?;

    let _ = state
        .audit
        .create_audit_log(Some(&current_user.user_id), "update", &model_name, Some(&id), Some(data), None)
        .await;

    // 后台重新索引（失败不影响主流程）
    if let Some(record_id) = record.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()) {
        let search = state.search.clone();
        let model_name_idx = model_name.clone();
        let data_idx = record.clone();
        tokio::spawn(async move {
            let _ = search.index_record(&model_name_idx, &record_id, &data_idx).await;
        });
    }

    Ok(Json(record))
}

pub async fn crud_delete(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path((model_name, id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(
        &policy,
        &model_name,
        &current_user.groups,
        AccessOp::Delete,
        &state.store,
        &current_user.user_id,
    )
    .await?;

    let record_filter = get_record_filter(
        &policy,
        &model_name,
        &current_user.groups,
        &current_user.user_id,
        AccessOp::Delete,
        &resolved_db.dialect,
    );

    let generic = GenericDb::new(&resolved_db.pool, &resolved_db.dialect);
    let existing = generic.generic_read_with_filter(&model, &id, record_filter.as_ref()).await?;
    match existing {
        Some(_) => {}
        None => {
            if record_filter.is_some() {
                let _ = state
                    .store
                    .create_audit_log(
                        Some(&current_user.user_id),
                        "record_filter_denied",
                        &model_name,
                        Some(&id),
                        None,
                        None,
                    )
                    .await;
            }
            return Err(AppError::NotFound("记录不存在或无权删除".into()));
        }
    }

    let deleted = generic.generic_delete(&model, &id).await?;
    if deleted {
        let _ = state
            .audit
            .create_audit_log(Some(&current_user.user_id), "delete", &model_name, Some(&id), None, None)
            .await;

        // 后台移除索引（失败不影响主流程）
        let search = state.search.clone();
        let model_name_idx = model_name.clone();
        let id_idx = id.clone();
        tokio::spawn(async move {
            let _ = search.remove_record(&model_name_idx, &id_idx).await;
        });

        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("记录不存在".into()))
    }
}

pub async fn crud_ensure_table(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
) -> Result<StatusCode, AppError> {
    let model = resolve_model(&state, &model_name)?;
    if !current_user.is_admin() {
        let _ = state
            .store
            .create_audit_log(Some(&current_user.user_id), "admin_required_denied", &model_name, None, None, None)
            .await;
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    let generic = GenericDb::new(&resolved_db.pool, &resolved_db.dialect);
    generic.ensure_table(&model).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn crud_models(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ingjoo_core::module::ModelDescriptor>>, AppError> {
    if !current_user.is_admin() {
        let _ = state
            .store
            .create_audit_log(Some(&current_user.user_id), "admin_required_denied", "models", None, None, None)
            .await;
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    Ok(Json(state.registry.list()))
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn crud_search(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
    Query(params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_access_with_audit(
        &policy,
        &model_name,
        &current_user.groups,
        AccessOp::Read,
        &state.store,
        &current_user.user_id,
    )
    .await?;

    let query_text = match params.q {
        Some(ref q) if !q.trim().is_empty() => q.trim().to_string(),
        _ => return Err(AppError::BadRequest("搜索关键词不能为空".into())),
    };

    let limit = params.limit.unwrap_or(20);
    let offset = params.offset.unwrap_or(0);

    // 先尝试搜索引擎
    let search_query = SearchQuery {
        text: query_text.clone(),
        models: vec![model_name.clone()],
        limit: Some(limit as usize),
        offset: Some(offset as usize),
        filters: None,
    };

    if let Ok(results) = state.search.search(search_query).await {
        return Ok(Json(serde_json::json!({
            "results": results.into_iter().map(|r| r.data).collect::<Vec<_>>(),
            "total": 0,
            "query": query_text,
        })));
    }

    // 搜索引擎不可用，回退到 ILIKE
    let record_filter = get_record_filter(
        &policy,
        &model_name,
        &current_user.groups,
        &current_user.user_id,
        AccessOp::Read,
        &resolved_db.dialect,
    );

    let search_conditions: Vec<Domain> = model
        .fields
        .iter()
        .filter(|f| matches!(f.field_type, FieldType::Text))
        .map(|f| Domain::Leaf {
            field: f.name.clone(),
            op: DomainOp::ILike,
            value: DomainValue::String(format!("%{}%", query_text)),
        })
        .collect();

    if search_conditions.is_empty() {
        return Ok(Json(serde_json::json!({
            "results": [],
            "total": 0,
            "query": query_text,
        })));
    }

    let search_domain = Domain::any(search_conditions);

    let generic = GenericDb::new(&resolved_db.pool, &resolved_db.dialect);
    let result =
        generic.generic_list_with_filter(&model, Some(&search_domain), record_filter.as_ref(), limit, offset).await?;

    Ok(Json(serde_json::json!({
        "results": result.items,
        "total": result.total,
        "query": query_text,
    })))
}
