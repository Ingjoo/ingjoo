use std::sync::Arc;

use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;

use ingjoo_core::db::models::CreateAttachment;
use ingjoo_core::UserId;
use ingjoo_security::{AccessOp, ModelAccess, RecordRule, SecurityPolicy};

use crate::db::generic::{GenericDb, GenericRecordStore};
use crate::extractors::CurrentUser;
use crate::middleware::database_selector::ResolvedDatabase;
use crate::middleware::error::AppError;
use crate::AppState;

fn resolve_model(state: &AppState, model_name: &str) -> Result<ingjoo_core::module::ModelDescriptor, AppError> {
    state.registry.get(model_name).ok_or_else(|| AppError::NotFound(format!("模型 '{}' 未注册", model_name)))
}

async fn load_security_policy(state: &AppState, groups: &[String]) -> Result<SecurityPolicy, AppError> {
    if groups.is_empty() {
        return Ok(SecurityPolicy::new());
    }

    let mut sorted_groups = groups.to_vec();
    sorted_groups.sort();
    let cache_key = sorted_groups.join(":");

    if let Some(cached) = state.cache.get_scope_access("policy", &cache_key) {
        return Ok(cached);
    }

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
        let domain = ingjoo_core::query::domain::Domain::from_json(&row.domain).map_err(|e| {
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

fn check_read_access(policy: &SecurityPolicy, model: &str, groups: &[String]) -> Result<(), AppError> {
    if !policy.check_access_groups(model, groups, AccessOp::Read) {
        return Err(AppError::Forbidden(format!("无权对 '{}' 执行 read 操作", model)));
    }
    Ok(())
}

fn check_write_access(policy: &SecurityPolicy, model: &str, groups: &[String]) -> Result<(), AppError> {
    if !policy.check_access_groups(model, groups, AccessOp::Write) {
        return Err(AppError::Forbidden(format!("无权对 '{}' 执行 write 操作", model)));
    }
    Ok(())
}

pub async fn upload_attachment(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path((model_name, record_id)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let _model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_write_access(&policy, &model_name, &current_user.groups)?;

    let generic = GenericDb::new(&resolved_db.pool, &resolved_db.dialect);
    let existing = generic.generic_read(&_model, &record_id).await?;
    if existing.is_none() {
        return Err(AppError::NotFound(format!("记录 {}#{} 不存在", model_name, record_id)));
    }

    let mut result: Option<serde_json::Value> = None;
    while let Some(field) =
        multipart.next_field().await.map_err(|e| AppError::BadRequest(format!("Multipart 解析错误: {}", e)))?
    {
        let filename = field.file_name().unwrap_or("unknown").to_string();
        let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
        let data = field.bytes().await.map_err(|e| AppError::BadRequest(format!("读取文件数据失败: {}", e)))?;
        let size = data.len() as i64;

        let file_id = uuid::Uuid::new_v4().to_string();
        let storage_path = format!("{}/{}/{}_{}", model_name, record_id, file_id, filename);

        state.file_storage.save(&storage_path, &data).await.map_err(|e| {
            tracing::error!("文件保存失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("文件保存失败"))
        })?;

        let user_id = UserId::new(&current_user.user_id);
        let create = CreateAttachment {
            filename,
            mime_type: content_type,
            size,
            entity_type: Some(model_name.clone()),
            entity_id: Some(record_id.clone()),
        };
        let attachment =
            state.store.create_attachment(&file_id, &user_id, &create, &storage_path).await.map_err(|e| {
                tracing::error!("创建附件记录失败: {:?}", e);
                AppError::Internal(anyhow::anyhow!("创建附件记录失败"))
            })?;

        result = Some(serde_json::to_value(&attachment).unwrap_or_default());
    }

    match result {
        Some(v) => Ok((StatusCode::CREATED, Json(v))),
        None => Err(AppError::BadRequest("未提供文件".into())),
    }
}

pub async fn list_attachments(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path((model_name, record_id)): Path<(String, String)>,
) -> Result<Json<ingjoo_core::PaginatedResult<ingjoo_core::db::models::Attachment>>, AppError> {
    let _model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_read_access(&policy, &model_name, &current_user.groups)?;

    let result = state.store.list_attachments(Some(&model_name), Some(&record_id), 100, 0).await.map_err(|e| {
        tracing::error!("查询附件列表失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("查询附件列表失败"))
    })?;

    Ok(Json(result))
}

pub async fn download_attachment(
    Extension(_current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path((_model_name, _record_id, attachment_id)): Path<(String, String, String)>,
) -> Result<axum::response::Response, AppError> {
    let attachment = state
        .store
        .get_attachment(&attachment_id)
        .await
        .map_err(|e| {
            tracing::error!("查询附件失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("查询附件失败"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("附件 {} 不存在", attachment_id)))?;

    let data = state.file_storage.load(&attachment.storage_path).await.map_err(|e| {
        tracing::error!("读取文件失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("读取文件失败"))
    })?;

    let body = axum::body::Body::from(data);
    let response = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", &attachment.mime_type)
        .header("Content-Disposition", format!("attachment; filename=\"{}\"", attachment.filename))
        .body(body)
        .map_err(|e| {
            tracing::error!("构建响应失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("构建响应失败"))
        })?;

    Ok(response)
}

pub async fn delete_attachment(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path((model_name, _record_id, attachment_id)): Path<(String, String, String)>,
) -> Result<StatusCode, AppError> {
    let _model = resolve_model(&state, &model_name)?;
    let policy = load_security_policy(&state, &current_user.groups).await?;
    check_write_access(&policy, &model_name, &current_user.groups)?;

    let attachment = state
        .store
        .get_attachment(&attachment_id)
        .await
        .map_err(|e| {
            tracing::error!("查询附件失败: {:?}", e);
            AppError::Internal(anyhow::anyhow!("查询附件失败"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("附件 {} 不存在", attachment_id)))?;

    state.file_storage.delete(&attachment.storage_path).await.map_err(|e| {
        tracing::error!("删除文件失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("删除文件失败"))
    })?;

    state.store.delete_attachment(&attachment_id).await.map_err(|e| {
        tracing::error!("删除附件记录失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("删除附件记录失败"))
    })?;

    Ok(StatusCode::NO_CONTENT)
}
