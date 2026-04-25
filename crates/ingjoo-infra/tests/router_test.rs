use std::sync::Arc;

use axum::body::Body;
use axum::Router;
use http::StatusCode;
use ingjoo_infra::{AppState, AuthConfig, JwtAuthProvider, IngjooDb, IngjooStore, DatabaseManager};
use ingjoo_core::ModelRegistry;
use tower::ServiceExt;

// ── 测试辅助函数 ──

async fn read_body(resp: axum::http::Response<Body>) -> serde_json::Value {
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    serde_json::from_slice(&body).unwrap_or_default()
}

/// 注册用户并登录，返回 access_token
async fn register_and_login(app: &Router) -> String {
    let _ = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"authtest@example.com","name":"authuser","password":"pass123"}"#),
        ))
        .await
        .unwrap();

    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"authtest@example.com","password":"pass123"}"#),
        ))
        .await
        .unwrap();

    let json = read_body(resp).await;
    json["access_token"].as_str().unwrap().to_string()
}

/// 构造带 Authorization 头的请求
fn make_auth_request(method: &str, uri: &str, token: &str) -> http::Request<Body> {
    http::Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap()
}

async fn setup_app() -> Router {
    ingjoo_core::pool::install_drivers();

    let db_url = match std::env::var("DATABASE_URL") {
        Ok(url) if url.starts_with("postgres") => {
            let test_db = format!("ingjoo_test_{}", uuid::Uuid::new_v4().to_string().replace('-', "_"));
            let base_url = url.rfind('/').map(|i| &url[..i]).unwrap_or(&url);
            let base_db_url = format!("{}/postgres", base_url);
            let (base_pool, _) = ingjoo_core::pool::connect_pool(&base_db_url).await.unwrap();
            sqlx::query(&format!("CREATE DATABASE \"{}\"", test_db))
                .execute(&base_pool)
                .await
                .expect("创建测试数据库失败");
            drop(base_pool);
            format!("{}/{}", base_url, test_db)
        }
        _ => {
            let tmp = tempfile::Builder::new()
                .prefix("router_test_")
                .suffix(".db")
                .tempfile()
                .unwrap();
            let db_path = tmp.path().to_str().unwrap().to_string();
            std::mem::forget(tmp);
            format!("sqlite://{}?mode=rwc", db_path)
        }
    };

    let (pool, dialect) = ingjoo_core::pool::connect_pool(&db_url).await.unwrap();
    IngjooDb::run_migrations(&pool, &dialect).await.unwrap();
    let store: Arc<dyn IngjooStore> = Arc::new(IngjooDb::with_dialect(pool.clone(), dialect));
    let auth = JwtAuthProvider::new(&AuthConfig::new("test-secret"));
    let registry = Arc::new(ModelRegistry::new());
    let state = Arc::new(AppState::new(store, auth, registry, Arc::new(pool.clone()), dialect, Arc::new(DatabaseManager::new(pool.clone(), dialect))));
    ingjoo_infra::router::base_router(state)
}

fn make_request(method: &str, uri: &str, body: Option<&str>) -> http::Request<Body> {
    let mut builder = http::Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder
        .body(Body::from(body.unwrap_or("").to_string()))
        .unwrap()
}

#[tokio::test]
async fn test_public_register_no_auth_needed() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"pub@example.com","name":"pubuser","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_public_login_no_auth_needed() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"anon@example.com","password":"pass"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "login is public but user not found");
}

#[tokio::test]
async fn test_public_refresh_no_auth_needed() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/refresh",
            Some(r#"{"refresh_token":"any-token"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "refresh is public endpoint but token invalid");
}

#[tokio::test]
async fn test_protected_profile_requires_auth() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/auth/profile", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_settings_requires_auth() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/settings", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_profile_invalid_jwt_returns_401() {
    let app = setup_app().await;
    let mut req = http::Request::builder()
        .method("GET")
        .uri("/api/auth/profile")
        .header("authorization", "Bearer totally.invalid.jwt")
        .body(Body::empty())
        .unwrap();
    let _ = &mut req;
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_nonexistent_route_returns_404() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/nonexistent", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Part A: 错误 JSON 格式验证 ──

#[tokio::test]
async fn test_unauthorized_returns_json_error() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/auth/profile", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json = read_body(resp).await;
    assert!(json["error"].is_string(), "response should have 'error' field");
    assert_eq!(json["code"].as_str().unwrap(), "UNAUTHORIZED");
    assert_eq!(json["status"].as_u64().unwrap(), 401);
}

#[tokio::test]
async fn test_invalid_token_returns_json_error() {
    let app = setup_app().await;
    let req = http::Request::builder()
        .method("GET")
        .uri("/api/auth/profile")
        .header("authorization", "Bearer invalid.jwt.token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json = read_body(resp).await;
    assert!(json["error"].is_string(), "response should have 'error' field");
    assert_eq!(json["code"].as_str().unwrap(), "UNAUTHORIZED");
    assert_eq!(json["status"].as_u64().unwrap(), 401);
}

#[tokio::test]
async fn test_login_wrong_password_returns_json_error() {
    let app = setup_app().await;
    let _ = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"errortest@example.com","name":"erruser","password":"correct123"}"#),
        ))
        .await
        .unwrap();

    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"errortest@example.com","password":"wrongpass"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json = read_body(resp).await;
    assert!(json["error"].is_string());
    assert_eq!(json["code"].as_str().unwrap(), "UNAUTHORIZED");
    assert_eq!(json["status"].as_u64().unwrap(), 401);
}

#[tokio::test]
async fn test_duplicate_register_returns_json_error() {
    let app = setup_app().await;
    let _ = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"duperr@example.com","name":"dup1","password":"pass123"}"#),
        ))
        .await
        .unwrap();

    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"duperr@example.com","name":"dup2","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = read_body(resp).await;
    assert!(json["error"].is_string());
    assert_eq!(json["code"].as_str().unwrap(), "BAD_REQUEST");
    assert_eq!(json["status"].as_u64().unwrap(), 400);
}

// ── Part B: Search 端点测试 ──

#[tokio::test]
async fn test_search_without_auth_returns_401() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/data/article/search?q=test", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json = read_body(resp).await;
    assert!(json["error"].is_string());
    assert_eq!(json["status"].as_u64().unwrap(), 401);
}

#[tokio::test]
async fn test_search_unregistered_model_returns_json_error() {
    let app = setup_app().await;
    let token = register_and_login(&app).await;
    let resp = app
        .oneshot(make_auth_request("GET", "/api/data/nonexistent_model/search?q=test", &token))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = read_body(resp).await;
    assert!(json["error"].is_string());
    assert_eq!(json["code"].as_str().unwrap(), "NOT_FOUND");
    assert_eq!(json["status"].as_u64().unwrap(), 404);
}

// ── Part C: Attachment 端点测试 ──

#[tokio::test]
async fn test_attachment_list_without_auth_returns_401() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/data/article/rec123/attachments", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json = read_body(resp).await;
    assert!(json["error"].is_string());
    assert_eq!(json["status"].as_u64().unwrap(), 401);
}

#[tokio::test]
async fn test_attachment_download_nonexistent_returns_404() {
    let app = setup_app().await;
    let token = register_and_login(&app).await;
    let resp = app
        .oneshot(make_auth_request("GET", "/api/data/article/rec123/attachments/nonexistent-id", &token))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = read_body(resp).await;
    assert!(json["error"].is_string());
    assert_eq!(json["code"].as_str().unwrap(), "NOT_FOUND");
    assert_eq!(json["status"].as_u64().unwrap(), 404);
}
