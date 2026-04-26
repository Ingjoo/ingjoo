//! 数据库生命周期管理 API — 管理员专用

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use ingjoo_core::db::traits::IngjooStore;
use ingjoo_core::Dialect;

use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

async fn require_admin(user: &CurrentUser, store: &Arc<dyn IngjooStore>) -> Result<(), AppError> {
    if !user.is_admin() {
        let _ =
            store.create_audit_log(Some(&user.user_id), "admin_required_denied", "databases", None, None, None).await;
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    Ok(())
}

#[derive(Serialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub status: String,
    pub is_default: bool,
    pub dialect: String,
}

#[derive(Serialize)]
pub struct DatabaseStatus {
    pub name: String,
    pub connected: bool,
    pub is_default: bool,
    pub dialect: String,
    pub table_count: Option<i64>,
}

#[derive(Serialize)]
pub struct ListDatabasesResponse {
    pub multi_db_enabled: bool,
    pub databases: Vec<DatabaseInfo>,
    pub default_dialect: String,
}

#[derive(Deserialize)]
pub struct CreateDatabaseRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct CreateDatabaseResponse {
    pub name: String,
    pub dialect: String,
    pub message: String,
}

// ==================== Handler ====================

/// GET /api/databases — 列出所有数据库
pub async fn list_databases(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ListDatabasesResponse>, AppError> {
    require_admin(&current_user, &state.store).await?;

    let mgr = &state.db_manager;
    let db_names = mgr.list_databases().await;

    let mut databases = vec![DatabaseInfo {
        name: "main".to_string(),
        status: "active".to_string(),
        is_default: true,
        dialect: format!("{:?}", mgr.default_dialect()),
    }];

    for name in db_names {
        databases.push(DatabaseInfo {
            name,
            status: "active".to_string(),
            is_default: false,
            dialect: format!("{:?}", mgr.default_dialect()),
        });
    }

    Ok(Json(ListDatabasesResponse {
        multi_db_enabled: mgr.is_multi_db_enabled(),
        databases,
        default_dialect: format!("{:?}", mgr.default_dialect()),
    }))
}

/// POST /api/databases — 创建新数据库
pub async fn create_database(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateDatabaseRequest>,
) -> Result<(StatusCode, Json<CreateDatabaseResponse>), AppError> {
    require_admin(&current_user, &state.store).await?;

    let mgr = &state.db_manager;

    if !mgr.is_multi_db_enabled() {
        return Err(AppError::BadRequest("多数据库模式未启用，请设置 DATABASE_BASE_URL 环境变量".into()));
    }

    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::BadRequest("数据库名称长度必须在 1-64 之间".into()));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::BadRequest("数据库名称只能包含字母、数字和下划线".into()));
    }
    if name == "main" {
        return Err(AppError::BadRequest("不能使用保留名称 'main'".into()));
    }

    // 检查是否已存在
    {
        let existing = mgr.list_databases().await;
        if existing.contains(&name) {
            return Err(AppError::Conflict(format!("数据库 '{}' 已存在", name)));
        }
    }

    // 获取连接池（会自动创建并缓存）
    let (pool, dialect) = mgr.get_pool(&name).await.map_err(|e| {
        tracing::error!("创建数据库失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("创建数据库失败"))
    })?;

    // 验证连接
    sqlx::query("SELECT 1").execute(&*pool).await.map_err(|e| {
        tracing::error!("数据库连接验证失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("数据库连接验证失败"))
    })?;

    let _ = state
        .store
        .create_audit_log(Some(&current_user.user_id), "database_created", "databases", Some(&name), None, None)
        .await;

    Ok((
        StatusCode::CREATED,
        Json(CreateDatabaseResponse {
            name, dialect: format!("{:?}", dialect), message: "数据库创建成功".to_string()
        }),
    ))
}

/// GET /api/databases/{name}/status — 获取数据库状态
pub async fn get_database_status(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<DatabaseStatus>, AppError> {
    require_admin(&current_user, &state.store).await?;

    let mgr = &state.db_manager;

    let (pool, dialect) = mgr.get_pool(&name).await.map_err(|e| {
        tracing::error!("获取数据库连接失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("获取数据库连接失败"))
    })?;

    let connected = sqlx::query("SELECT 1").execute(&*pool).await.is_ok();

    // 尝试获取表数量
    let table_count = if connected {
        let count_sql = match dialect {
            Dialect::Sqlite => "SELECT count(*) as cnt FROM sqlite_master WHERE type='table'",
            Dialect::Postgres => "SELECT count(*) as cnt FROM information_schema.tables WHERE table_schema = 'public'",
        };
        sqlx::query(count_sql).fetch_one(&*pool).await.ok().and_then(|row| row.try_get::<i64, _>("cnt").ok())
    } else {
        None
    };

    Ok(Json(DatabaseStatus {
        is_default: name == "main" || name.is_empty(),
        name,
        connected,
        dialect: format!("{:?}", dialect),
        table_count,
    }))
}

/// DELETE /api/databases/{name} — 删除（关闭连接池）
pub async fn delete_database(
    Extension(current_user): Extension<CurrentUser>,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user, &state.store).await?;

    if name == "main" || name.is_empty() {
        return Err(AppError::BadRequest("不能删除默认数据库".into()));
    }

    let mgr = &state.db_manager;

    // 检查数据库是否存在
    let existing = mgr.list_databases().await;
    if !existing.contains(&name) {
        return Err(AppError::NotFound(format!("数据库 '{}' 不存在", name)));
    }

    mgr.remove_pool(&name).await.map_err(|e| {
        tracing::error!("删除数据库失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("删除数据库失败"))
    })?;

    let _ = state
        .store
        .create_audit_log(Some(&current_user.user_id), "database_deleted", "databases", Some(&name), None, None)
        .await;

    Ok(StatusCode::NO_CONTENT)
}

// ==================== 多数据库管理 API（公共，Basic Auth） ====================

#[cfg(feature = "multi-db")]
use crate::auth::AuthProvider;

#[cfg(feature = "multi-db")]
use regex::Regex;

/// GET /api/database/list — 公共列出数据库（受 LIST_DB 和 DBFILTER 控制）
#[cfg(feature = "multi-db")]
pub async fn public_list_databases(State(state): State<Arc<AppState>>) -> Result<Json<PublicListResponse>, AppError> {
    if !state.list_db {
        return Err(AppError::Forbidden("数据库列表功能已禁用".into()));
    }

    let mgr = &state.db_manager;
    let mut db_names = mgr.list_databases().await;

    if let Some(ref filter) = state.dbfilter {
        if let Ok(re) = Regex::new(filter) {
            db_names.retain(|name| re.is_match(name));
        }
    }

    let mut databases = vec![PublicDatabaseEntry { name: "main".to_string(), is_default: true }];

    for name in db_names {
        databases.push(PublicDatabaseEntry { name, is_default: false });
    }

    Ok(Json(PublicListResponse { databases }))
}

/// POST /api/database/create — 创建新数据库（Basic Auth）
#[cfg(feature = "multi-db")]
pub async fn public_create_database(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateDatabaseRequest>,
) -> Result<(StatusCode, Json<CreateDatabaseResponse>), AppError> {
    let mgr = &state.db_manager;

    if !mgr.is_multi_db_enabled() {
        return Err(AppError::BadRequest("多数据库模式未启用".into()));
    }

    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::BadRequest("数据库名称长度必须在 1-64 之间".into()));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::BadRequest("数据库名称只能包含字母、数字和下划线".into()));
    }
    if name == "main" {
        return Err(AppError::BadRequest("不能使用保留名称 'main'".into()));
    }

    let existing = mgr.list_databases().await;
    if existing.contains(&name) {
        return Err(AppError::Conflict(format!("数据库 '{}' 已存在", name)));
    }

    let (pool, dialect) = mgr.get_pool(&name).await.map_err(|e| {
        tracing::error!("创建数据库连接池失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("创建数据库连接池失败"))
    })?;

    let admin_hash =
        state.auth.hash_password("admin").map_err(|e| AppError::Internal(anyhow::anyhow!("密码哈希失败: {}", e)))?;

    crate::db::init_database(&pool, &dialect, &admin_hash).await.map_err(|e| {
        tracing::error!("数据库初始化失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("数据库初始化失败"))
    })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateDatabaseResponse {
            name,
            dialect: format!("{:?}", dialect),
            message: "数据库创建并初始化成功".to_string(),
        }),
    ))
}

/// DELETE /api/database/{name} — 删除数据库（Basic Auth）
#[cfg(feature = "multi-db")]
pub async fn public_delete_database(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    if name == "main" || name.is_empty() {
        return Err(AppError::BadRequest("不能删除默认数据库".into()));
    }

    let mgr = &state.db_manager;
    let existing = mgr.list_databases().await;
    if !existing.contains(&name) {
        return Err(AppError::NotFound(format!("数据库 '{}' 不存在", name)));
    }

    mgr.remove_pool(&name).await.map_err(|e| {
        tracing::error!("删除数据库失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("删除数据库失败"))
    })?;

    if let Some(base_url) = mgr.base_url() {
        if base_url.starts_with("sqlite:") {
            let file_path = format!("{}/{}.db", base_url.trim_start_matches("sqlite:").trim_end_matches('/'), name);
            let _ = std::fs::remove_file(&file_path);
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/database/{name}/info — 获取数据库信息（Basic Auth）
#[cfg(feature = "multi-db")]
pub async fn public_database_info(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<PublicDatabaseInfoResponse>, AppError> {
    let mgr = &state.db_manager;

    let (pool, dialect) = mgr.get_pool(&name).await.map_err(|e| {
        tracing::error!("获取数据库连接失败: {:?}", e);
        AppError::Internal(anyhow::anyhow!("获取数据库连接失败"))
    })?;

    let connected = sqlx::query("SELECT 1").execute(&*pool).await.is_ok();

    let table_count = if connected {
        let count_sql = match dialect {
            Dialect::Sqlite => "SELECT count(*) as cnt FROM sqlite_master WHERE type='table'",
            Dialect::Postgres => "SELECT count(*) as cnt FROM information_schema.tables WHERE table_schema = 'public'",
        };
        sqlx::query(count_sql).fetch_one(&*pool).await.ok().and_then(|row| row.try_get::<i64, _>("cnt").ok())
    } else {
        None
    };

    let size_bytes = if connected && dialect == Dialect::Sqlite {
        if let Some(base_url) = mgr.base_url() {
            let file_path = format!("{}/{}.db", base_url.trim_start_matches("sqlite:").trim_end_matches('/'), name);
            std::fs::metadata(&file_path).ok().map(|m| m.len() as i64)
        } else {
            None
        }
    } else {
        None
    };

    Ok(Json(PublicDatabaseInfoResponse {
        is_default: name == "main",
        name,
        connected,
        dialect: format!("{:?}", dialect),
        table_count,
        size_bytes,
    }))
}

/// POST /api/database/{name}/backup — 备份数据库（Basic Auth）
#[cfg(feature = "multi-db")]
pub async fn public_backup_database(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<BackupResponse>, AppError> {
    let mgr = &state.db_manager;
    let backup_dir = "./data/backups";
    std::fs::create_dir_all(backup_dir).map_err(|e| AppError::Internal(anyhow::anyhow!("创建备份目录失败: {}", e)))?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_filename = format!("{}_{}.db", name, timestamp);
    let backup_path = format!("{}/{}", backup_dir, backup_filename);

    if let Some(base_url) = mgr.base_url() {
        if base_url.starts_with("sqlite:") {
            let source_path = format!("{}/{}.db", base_url.trim_start_matches("sqlite:").trim_end_matches('/'), name);
            let (pool, _) = mgr
                .get_pool(&name)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("获取数据库连接失败: {}", e)))?;
            let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&*pool).await;

            std::fs::copy(&source_path, &backup_path)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("备份失败: {}", e)))?;
        }
    }

    Ok(Json(BackupResponse { backup_path, message: "备份成功".to_string() }))
}

/// POST /api/database/{name}/restore — 恢复数据库（Basic Auth）
#[cfg(feature = "multi-db")]
pub async fn public_restore_database(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<RestoreRequest>,
) -> Result<Json<RestoreResponse>, AppError> {
    let mgr = &state.db_manager;

    if !std::path::Path::new(&req.backup_path).exists() {
        return Err(AppError::NotFound(format!("备份文件不存在: {}", req.backup_path)));
    }

    if let Some(base_url) = mgr.base_url() {
        if base_url.starts_with("sqlite:") {
            let target_path = format!("{}/{}.db", base_url.trim_start_matches("sqlite:").trim_end_matches('/'), name);

            mgr.remove_pool(&name)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("关闭数据库连接失败: {}", e)))?;

            std::fs::copy(&req.backup_path, &target_path)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("恢复失败: {}", e)))?;

            let _ = mgr
                .get_pool(&name)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("重新连接数据库失败: {}", e)))?;
        }
    }

    Ok(Json(RestoreResponse { message: "数据库恢复成功".to_string() }))
}

/// POST /api/database/backup/upload — 上传备份文件（Basic Auth）
#[cfg(feature = "multi-db")]
pub async fn public_upload_backup(
    State(_state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Result<Json<UploadBackupResponse>, AppError> {
    let backup_dir = "./data/backups";
    std::fs::create_dir_all(backup_dir).map_err(|e| AppError::Internal(anyhow::anyhow!("创建备份目录失败: {}", e)))?;

    let filename = format!("upload_{}.db", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    let path = format!("{}/{}", backup_dir, filename);

    std::fs::write(&path, &body).map_err(|e| AppError::Internal(anyhow::anyhow!("写入备份文件失败: {}", e)))?;

    Ok(Json(UploadBackupResponse { path }))
}

/// GET /api/database/backup/list — 列出备份文件（Basic Auth）
#[cfg(feature = "multi-db")]
pub async fn public_list_backups(State(_state): State<Arc<AppState>>) -> Result<Json<ListBackupsResponse>, AppError> {
    let backup_dir = "./data/backups";
    let mut backups = Vec::new();

    if let Ok(entries) = std::fs::read_dir(backup_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "db").unwrap_or(false) {
                let metadata = entry.metadata().ok();
                let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                let created = metadata
                    .and_then(|m| m.created().ok())
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                        dt.to_rfc3339()
                    })
                    .unwrap_or_default();

                backups.push(BackupInfo { filename, size, created_at: created });
            }
        }
    }

    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(Json(ListBackupsResponse { backups }))
}

// ==================== 新增请求/响应类型（multi-db） ====================

#[derive(Serialize)]
pub struct PublicDatabaseEntry {
    pub name: String,
    pub is_default: bool,
}

#[derive(Serialize)]
pub struct PublicListResponse {
    pub databases: Vec<PublicDatabaseEntry>,
}

#[derive(Serialize)]
pub struct PublicDatabaseInfoResponse {
    pub name: String,
    pub connected: bool,
    pub is_default: bool,
    pub dialect: String,
    pub table_count: Option<i64>,
    pub size_bytes: Option<i64>,
}

#[derive(Serialize)]
pub struct BackupResponse {
    pub backup_path: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct RestoreRequest {
    pub backup_path: String,
}

#[derive(Serialize)]
pub struct RestoreResponse {
    pub message: String,
}

#[derive(Serialize)]
pub struct UploadBackupResponse {
    pub path: String,
}

#[derive(Serialize)]
pub struct ListBackupsResponse {
    pub backups: Vec<BackupInfo>,
}

#[derive(Serialize)]
pub struct BackupInfo {
    pub filename: String,
    pub size: u64,
    pub created_at: String,
}
