use std::sync::Arc;

use axum::extract::Request;
use axum::extract::State;
use axum::middleware::Next;
use axum::response::Response;

use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;

use crate::db::database_manager::DatabaseManager;
use crate::extractors::CurrentUser;
use crate::middleware::error::AppError;
use crate::AppState;

const DB_HEADER: &str = "X-Ingjoo-Database";
const DB_QUERY_PARAM: &str = "db";

#[derive(Debug, Clone)]
pub struct ResolvedDatabase {
    pub db_name: String,
    pub pool: Arc<Pool>,
    pub dialect: Dialect,
}

pub async fn database_selector_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let db_name = extract_database_name(&request)
        .or_else(|| request.extensions().get::<CurrentUser>().and_then(|u| u.database.clone()));

    let resolved = resolve_database(&state.db_manager, db_name).await?;

    request.extensions_mut().insert(resolved);

    Ok(next.run(request).await)
}

fn extract_database_name(request: &Request) -> Option<String> {
    if let Some(header) = request.headers().get(DB_HEADER) {
        if let Ok(name) = header.to_str() {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    let uri = request.uri();
    let query = uri.query().unwrap_or("");
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix(DB_QUERY_PARAM) {
            let value = value.trim_start_matches('=').trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    None
}

async fn resolve_database(
    manager: &Arc<DatabaseManager>,
    db_name: Option<String>,
) -> Result<ResolvedDatabase, AppError> {
    let name = db_name.unwrap_or_else(|| "main".to_string());

    validate_db_name(&name)?;

    let (pool, dialect) = manager
        .get_pool(&name)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("获取数据库 '{}' 连接池失败: {}", name, e)))?;

    Ok(ResolvedDatabase { db_name: name, pool, dialect })
}

fn validate_db_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::BadRequest("数据库名不能为空".into()));
    }
    if name.len() > 64 {
        return Err(AppError::BadRequest("数据库名长度不能超过 64 个字符".into()));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(AppError::BadRequest("数据库名只能包含字母、数字、下划线和连字符".into()));
    }
    Ok(())
}

pub fn get_resolved_database(extensions: &axum::http::Extensions) -> Option<&ResolvedDatabase> {
    extensions.get::<ResolvedDatabase>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_db_name_valid() {
        assert!(validate_db_name("tenant_acme").is_ok());
        assert!(validate_db_name("my-db-123").is_ok());
        assert!(validate_db_name("main").is_ok());
    }

    #[test]
    fn test_validate_db_name_rejects_empty() {
        assert!(validate_db_name("").is_err());
    }

    #[test]
    fn test_validate_db_name_rejects_too_long() {
        let long_name = "a".repeat(65);
        assert!(validate_db_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_db_name_rejects_special_chars() {
        assert!(validate_db_name("db;drop").is_err());
        assert!(validate_db_name("db name").is_err());
        assert!(validate_db_name("db.name").is_err());
    }

    #[test]
    fn test_extract_database_name_from_header() {
        let req =
            axum::http::Request::builder().header(DB_HEADER, "tenant_acme").body(axum::body::Body::empty()).unwrap();
        assert_eq!(extract_database_name(&req), Some("tenant_acme".to_string()));
    }

    #[test]
    fn test_extract_database_name_from_query() {
        let req = axum::http::Request::builder()
            .uri("/api/data?db=tenant_acme&limit=10")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(extract_database_name(&req), Some("tenant_acme".to_string()));
    }

    #[test]
    fn test_extract_database_name_none() {
        let req = axum::http::Request::builder().uri("/api/data").body(axum::body::Body::empty()).unwrap();
        assert_eq!(extract_database_name(&req), None);
    }

    #[test]
    fn test_header_takes_priority_over_query() {
        let req = axum::http::Request::builder()
            .uri("/api/data?db=query_db")
            .header(DB_HEADER, "header_db")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(extract_database_name(&req), Some("header_db".to_string()));
    }
}
