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
        let _ = store.create_audit_log(
            Some(&user.user_id),
            "admin_required_denied",
            "databases",
            None,
            None,
            None,
        ).await;
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
        return Err(AppError::BadRequest(
            "多数据库模式未启用，请设置 DATABASE_BASE_URL 环境变量".into(),
        ));
    }

    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::BadRequest("数据库名称长度必须在 1-64 之间".into()));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::BadRequest(
            "数据库名称只能包含字母、数字和下划线".into(),
        ));
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
    let (pool, dialect) = mgr.get_pool(&name).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("创建数据库失败: {}", e)))?;

    // 验证连接
    sqlx::query("SELECT 1")
        .execute(&*pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("数据库连接验证失败: {}", e)))?;

    let _ = state.store.create_audit_log(
        Some(&current_user.user_id),
        "database_created",
        "databases",
        Some(&name),
        None,
        None,
    ).await;

    Ok((
        StatusCode::CREATED,
        Json(CreateDatabaseResponse {
            name,
            dialect: format!("{:?}", dialect),
            message: "数据库创建成功".to_string(),
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

    let (pool, dialect) = mgr.get_pool(&name).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("获取数据库连接失败: {}", e)))?;

    let connected = sqlx::query("SELECT 1")
        .execute(&*pool)
        .await
        .is_ok();

    // 尝试获取表数量
    let table_count = if connected {
        let count_sql = match dialect {
            Dialect::Sqlite => "SELECT count(*) as cnt FROM sqlite_master WHERE type='table'",
            Dialect::Postgres => "SELECT count(*) as cnt FROM information_schema.tables WHERE table_schema = 'public'",
        };
        sqlx::query(count_sql)
            .fetch_one(&*pool)
            .await
            .ok()
            .and_then(|row| row.try_get::<i64, _>("cnt").ok())
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

    mgr.remove_pool(&name).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("删除数据库失败: {}", e)))?;

    let _ = state.store.create_audit_log(
        Some(&current_user.user_id),
        "database_deleted",
        "databases",
        Some(&name),
        None,
        None,
    ).await;

    Ok(StatusCode::NO_CONTENT)
}
