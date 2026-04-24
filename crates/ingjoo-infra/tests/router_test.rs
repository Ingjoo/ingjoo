use std::sync::Arc;

use axum::body::Body;
use axum::Router;
use http::StatusCode;
use ingjoo_infra::{AppState, AuthConfig, JwtAuthProvider, IngjooDb, IngjooStore};
use ingjoo_core::ModelRegistry;
use tower::ServiceExt;

async fn setup_app() -> Router {
    let tmp = tempfile::Builder::new()
        .prefix("router_test_")
        .suffix(".db")
        .tempfile()
        .unwrap();
    let db_path = tmp.path().to_str().unwrap().to_string();
    std::mem::forget(tmp);

    let db_url = format!("sqlite://{}?mode=rwc", db_path);
    ingjoo_core::pool::install_drivers();
    let (pool, dialect) = ingjoo_core::pool::connect_pool(&db_url).await.unwrap();
    IngjooDb::run_migrations(&pool, &dialect).await.unwrap();
    let store: Arc<dyn IngjooStore> = Arc::new(IngjooDb::with_dialect(pool.clone(), dialect));
    let auth = JwtAuthProvider::new(&AuthConfig::new("test-secret"));
    let registry = Arc::new(ModelRegistry::new());
    let state = Arc::new(AppState::new(store, auth, registry, Arc::new(pool), dialect));
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
