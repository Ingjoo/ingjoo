use std::sync::Arc;

use axum::body::Body;
use axum::Router;
use http::StatusCode;
use ingjoo_core::db::ids::GroupId;
use ingjoo_core::db::models::Group;
use ingjoo_infra::{AppState, AuthConfig, AuthProvider, JwtAuthProvider, IngjooDb, IngjooStore};
use ingjoo_core::ModelRegistry;
use tower::ServiceExt;

async fn create_state() -> (Router, Arc<AppState>) {
    let tmp = tempfile::Builder::new()
        .prefix("ingjoo_test_")
        .suffix(".db")
        .tempfile()
        .unwrap();
    let db_path = tmp.path().to_str().unwrap().to_string();
    std::mem::forget(tmp);

    let db_url = format!("sqlite://{}?mode=rwc", db_path);
    ingjoo_core::pool::install_drivers();
    let (pool, dialect) = ingjoo_core::pool::connect_pool(&db_url)
        .await
        .unwrap();
    IngjooDb::run_migrations(&pool, &dialect).await.unwrap();
    let store: Arc<dyn IngjooStore> = Arc::new(IngjooDb::with_dialect(pool.clone(), dialect));
    let auth = JwtAuthProvider::new(&AuthConfig::new("test-secret"));
    let registry = Arc::new(ModelRegistry::new());
    let state = Arc::new(AppState::new(store, auth, registry, Arc::new(pool), dialect));
    let router = ingjoo_infra::router::base_router(state.clone());
    (router, state)
}

async fn setup_app() -> Router {
    let (router, _) = create_state().await;
    router
}

async fn setup_app_with_state() -> (Router, Arc<AppState>) {
    create_state().await
}

async fn setup_app_with_models(models: Vec<ingjoo_core::module::ModelDescriptor>) -> (Router, Arc<AppState>) {
    let tmp = tempfile::Builder::new()
        .prefix("ingjoo_test_")
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

    let mut registry = ModelRegistry::new();
    for model in models {
        registry.register(model);
    }
    let registry = Arc::new(registry);

    let state = Arc::new(AppState::new(store, auth, registry, Arc::new(pool), dialect));
    let router = ingjoo_infra::router::base_router(state.clone());
    (router, state)
}

async fn get_admin_token(app: &Router, state: &Arc<AppState>) -> String {
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"admin@test.com","name":"admin","password":"adminpass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let user_id = json["user"]["id"].as_str().unwrap();

    let admin_group = match state.store.get_group_by_name("admin").await.unwrap() {
        Some(g) => g,
        None => {
            let g = Group {
                id: GroupId::new(uuid::Uuid::new_v4().to_string()),
                name: "admin".to_string(),
                display_name: Some("管理员".to_string()),
                comment: None,
                created_at: String::new(),
                updated_at: String::new(),
            };
            state.store.create_group(&g).await.unwrap();
            g
        }
    };

    state
        .store
        .add_user_to_group(
            &ingjoo_core::db::ids::UserId::new(user_id.to_string()),
            &admin_group.id,
        )
        .await
        .unwrap();

    let groups = state
        .store
        .resolve_all_groups(&ingjoo_core::db::ids::UserId::new(user_id.to_string()))
        .await
        .unwrap();

    state
        .auth
        .create_access_token(user_id, "admin", &groups)
        .unwrap()
}

async fn get_user_token(app: &Router) -> String {
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"user@test.com","name":"normaluser","password":"userpass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["access_token"].as_str().unwrap().to_string()
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

fn auth_request(method: &str, uri: &str, token: &str, body: Option<&str>) -> http::Request<Body> {
    let mut builder = http::Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {}", token));
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder
        .body(Body::from(body.unwrap_or("").to_string()))
        .unwrap()
}

#[tokio::test]
async fn test_register_login_profile_flow() {
    let app = setup_app().await;

    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"test@example.com","name":"testuser","password":"password123"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(resp.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let access_token = json["access_token"].as_str().unwrap();
    let refresh_token = json["refresh_token"].as_str().unwrap();
    assert!(!access_token.is_empty());
    assert!(!refresh_token.is_empty());
    assert_eq!(json["user"]["email"].as_str().unwrap(), "test@example.com");
    assert_eq!(json["user"]["name"].as_str().unwrap(), "testuser");

    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/auth/profile", access_token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096)
        .await
        .unwrap();
    let profile: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(profile["email"].as_str().unwrap(), "test@example.com");

    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            "/api/auth/profile",
            access_token,
            Some(r#"{"name":"updated_name","bio":"hello world"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["name"].as_str().unwrap(), "updated_name");
    assert_eq!(updated["bio"].as_str().unwrap(), "hello world");
}

#[tokio::test]
async fn test_login_returns_tokens() {
    let app = setup_app().await;

    app.clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"login@test.com","name":"logintest","password":"pass123"}"#),
        ))
        .await
        .unwrap();

    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"login@test.com","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["access_token"].is_string());
    assert!(json["refresh_token"].is_string());
}

#[tokio::test]
async fn test_login_wrong_password() {
    let app = setup_app().await;

    app.clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"wrong@test.com","name":"wrongtest","password":"pass123"}"#),
        ))
        .await
        .unwrap();

    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"wrong@test.com","password":"wrongpass"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_duplicate_registration() {
    let app = setup_app().await;

    app.clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"dup@test.com","name":"dup1","password":"pass123"}"#),
        ))
        .await
        .unwrap();

    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"dup@test.com","name":"dup2","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_unauthenticated_access_denied() {
    let app = setup_app().await;

    let resp = app
        .oneshot(make_request("GET", "/api/auth/profile", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_refresh_token_flow() {
    let app = setup_app().await;

    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"refresh@test.com","name":"refreshtest","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let refresh_token = json["refresh_token"].as_str().unwrap();

    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/refresh",
            Some(&format!(r#"{{"refresh_token":"{}"}}"#, refresh_token)),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["access_token"].is_string());
    assert!(json["refresh_token"].is_string());
}

#[tokio::test]
async fn test_settings_requires_admin() {
    let app = setup_app().await;

    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"user@test.com","name":"normaluser","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    let resp = app
        .oneshot(auth_request("GET", "/api/settings", token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── 健康检查 ──

#[tokio::test]
async fn test_health_check_ok() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/health", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"].as_str().unwrap(), "ok");
    assert!(json["database"].as_bool().unwrap());
    assert!(json["uptime_seconds"].is_number());
    assert!(json["registered_models"].is_number());
    assert!(json["dialect"].is_string());
}

// ── 速率限制 ──

#[tokio::test]
async fn test_rate_limit_returns_429_after_burst() {
    let app = setup_app().await;

    for i in 0..10 {
        let resp = app
            .clone()
            .oneshot(make_request(
                "POST",
                "/api/auth/login",
                Some(&format!(
                    r#"{{"email":"ratelimit{}@test.com","password":"wrong"}}"#,
                    i
                )),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "request {} should pass", i);
    }

    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"ratelimit@test.com","password":"wrong"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

// ── CRUD 动态模型 ──

#[tokio::test]
async fn test_crud_ensure_table() {
    let (app, state) = setup_app_with_models(vec![
        ingjoo_core::module::ModelDescriptor::new("test_item", "test_items")
            .required_field("name", ingjoo_core::FieldType::Text),
    ]).await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request("POST", "/api/data/test_item/ensure", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_crud_model_not_found() {
    let (app, state) = setup_app_with_models(vec![
        ingjoo_core::module::ModelDescriptor::new("article", "articles")
            .required_field("title", ingjoo_core::FieldType::Text),
    ]).await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/data/nonexistent", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── 管理员：组管理 ──

#[tokio::test]
async fn test_groups_crud_admin() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/groups",
            &admin,
            Some(r#"{"name":"editors","display_name":"编辑组","implied_group_ids":[]}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let group: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let gid = group["id"].as_str().unwrap();
    assert_eq!(group["name"].as_str().unwrap(), "editors");

    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/groups", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            &format!("/api/groups/{}", gid),
            &admin,
            Some(r#"{"name":"editors","display_name":"高级编辑组","implied_group_ids":[]}"#),
        ))
        .await
        .unwrap();
    assert!(resp.status() == StatusCode::OK || resp.status() == StatusCode::NO_CONTENT);

    let resp = app
        .clone()
        .oneshot(auth_request(
            "DELETE",
            &format!("/api/groups/{}", gid),
            &admin,
            None,
        ))
        .await
        .unwrap();
    assert!(resp.status() == StatusCode::OK || resp.status() == StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_groups_nonadmin_forbidden() {
    let app = setup_app().await;

    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"normal@groups.com","name":"normal","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    let resp = app
        .oneshot(auth_request("GET", "/api/groups", token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── 管理员：设置管理 ──

#[tokio::test]
async fn test_settings_crud_admin() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            "/api/settings/test.key",
            &admin,
            Some(r#"{"value":"test_value"}"#),
        ))
        .await
        .unwrap();
    assert!(resp.status() == StatusCode::OK || resp.status() == StatusCode::NO_CONTENT);

    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/settings", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let found = json
        .as_object()
        .unwrap()
        .keys()
        .any(|k| k == "test.key");
    assert!(found, "settings list should contain test.key");
}

// ── 管理员：权限规则 ──

#[tokio::test]
async fn test_access_rules_requires_admin() {
    let app = setup_app().await;
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"normal@access.com","name":"normal","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    let resp = app
        .oneshot(auth_request("GET", "/api/access", token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_health_has_database_field() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/health", None))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["database"].is_boolean());
    assert!(json["database"].as_bool().unwrap());
}

#[tokio::test]
async fn test_crud_model_not_registered() {
    let app = setup_app().await;
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"crud@test.com","name":"cruduser","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    let resp = app
        .oneshot(auth_request("GET", "/api/data/nonexistent_model", token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_groups_requires_auth() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/groups", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_register_missing_email() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"name":"noemail","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"nobody@test.com","password":"anything"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_token_rejected() {
    let app = setup_app().await;
    let resp = app
        .oneshot(auth_request("GET", "/api/auth/profile", "invalid.jwt.token", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_refresh_with_invalid_token() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request(
            "POST",
            "/api/auth/refresh",
            Some(r#"{"refresh_token":"completely-invalid-token"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_models_list_authenticated() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .oneshot(auth_request("GET", "/api/models", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.is_array());
}

// ── 菜单管理 ──

#[tokio::test]
async fn test_menu_crud_admin() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    // 创建菜单
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/menus",
            &admin,
            Some(r#"{"name":"测试菜单","sequence":10}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let menu: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let menu_id = menu["id"].as_str().unwrap();
    assert_eq!(menu["name"].as_str().unwrap(), "测试菜单");

    // 列出菜单
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/menus", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["menus"].is_array());

    // 更新菜单
    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            &format!("/api/menus/{}", menu_id),
            &admin,
            Some(r#"{"name":"更新菜单"}"#),
        ))
        .await
        .unwrap();
    assert!(resp.status() == StatusCode::OK);

    // 删除菜单
    let resp = app
        .clone()
        .oneshot(auth_request("DELETE", &format!("/api/menus/{}", menu_id), &admin, None))
        .await
        .unwrap();
    assert!(resp.status() == StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_menu_list_authenticated() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/menus", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["menus"].is_array());
}

#[tokio::test]
async fn test_menu_nonadmin_forbidden() {
    let app = setup_app().await;
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"menuuser@test.com","name":"normal","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    let resp = app
        .oneshot(auth_request("POST", "/api/menus", token, Some(r#"{"name":"x"}"#)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── 视图管理 ──

#[tokio::test]
async fn test_view_crud_admin() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    // 创建视图
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/views",
            &admin,
            Some(r#"{"name":"产品表单","model":"product","view_type":"form","arch":{"fields":["name","price"]}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let view: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let view_id = view["id"].as_str().unwrap();
    assert_eq!(view["name"].as_str().unwrap(), "产品表单");
    assert_eq!(view["type"].as_str().unwrap(), "form");

    // 列出视图
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/views", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 获取单个视图
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/views/{}", view_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let fetched: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(fetched["id"].as_str().unwrap(), view_id);

    // 删除视图
    let resp = app
        .clone()
        .oneshot(auth_request("DELETE", &format!("/api/views/{}", view_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// ── 动作管理 ──

#[tokio::test]
async fn test_action_crud_admin() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    // 创建动作
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/actions",
            &admin,
            Some(r#"{"name":"产品管理","action_type":"act_window","res_model":"product","view_mode":["list","form"],"view_ids":[]}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let action: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let action_id = action["id"].as_str().unwrap();
    assert_eq!(action["name"].as_str().unwrap(), "产品管理");

    // 列出动作
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/actions", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 获取单个动作（含关联视图）
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/actions/{}", action_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let fetched: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(fetched["action"].is_object());

    // 删除动作
    let resp = app
        .clone()
        .oneshot(auth_request("DELETE", &format!("/api/actions/{}", action_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// ── 调度任务管理 ──

#[tokio::test]
async fn test_schedule_list_admin() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/schedules", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(list.is_array());
}

#[tokio::test]
async fn test_schedule_nonadmin_forbidden() {
    let app = setup_app().await;
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"scheduser@test.com","name":"normal","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    let resp = app
        .oneshot(auth_request("GET", "/api/schedules", token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── 记录规则管理 ──

#[tokio::test]
async fn test_record_rules_requires_auth() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/rules", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── 模型权限管理 ──

#[tokio::test]
async fn test_model_access_nonadmin_forbidden() {
    let app = setup_app().await;
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"mauser@test.com","name":"normal","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    let resp = app
        .oneshot(auth_request("GET", "/api/access", token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_record_rules_nonadmin_forbidden() {
    let app = setup_app().await;
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"rruser@test.com","name":"normal","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    let resp = app
        .oneshot(auth_request("GET", "/api/rules", token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── 用户-组关系 ──

#[tokio::test]
async fn test_user_groups_admin() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    // 注册普通用户
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"uguser@test.com","name":"ugtest","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let user_id = json["user"]["id"].as_str().unwrap();

    // 查询用户组（初始为空）
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/users/{}/groups", user_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let groups: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(groups.is_array());

    // 创建一个组
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/groups",
            &admin,
            Some(r#"{"name":"operators","display_name":"操作员","implied_group_ids":[]}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let group: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let group_id = group["id"].as_str().unwrap();

    // 设置用户组
    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            &format!("/api/users/{}/groups", user_id),
            &admin,
            Some(&format!(r#"["{}"]"#, group_id)),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 验证组已设置
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/users/{}/groups", user_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let groups: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(groups.as_array().unwrap().len() >= 1);
}

// ── 模型权限 CRUD (admin) ──

#[tokio::test]
async fn test_model_access_crud_admin() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    // 创建测试组
    let group = Group {
        id: GroupId::new(uuid::Uuid::new_v4().to_string()),
        name: "mac_test".into(),
        display_name: Some("MA测试组".into()),
        comment: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    state.store.create_group(&group).await.unwrap();
    let group_id_str = &group.id.0;

    // CREATE
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/access",
            &admin,
            Some(&format!(
                r#"{{"group_id":"{}","model":"test_model","perm_read":true,"perm_write":false,"perm_create":false,"perm_delete":false}}"#,
                group_id_str
            )),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let access_id = json["id"].as_str().unwrap().to_string();
    assert_eq!(json["model"], "test_model");
    assert_eq!(json["perm_read"], true);

    // LIST
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/access", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(list.as_array().unwrap().len() >= 1);

    // GET
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/access/{}", access_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let got: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(got["model"], "test_model");

    // UPDATE
    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            &format!("/api/access/{}", access_id),
            &admin,
            Some(&format!(
                r#"{{"group_id":"{}","model":"test_model","perm_read":true,"perm_write":true,"perm_create":false,"perm_delete":false}}"#,
                group_id_str
            )),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["perm_write"], true);

    // DELETE
    let resp = app
        .clone()
        .oneshot(auth_request("DELETE", &format!("/api/access/{}", access_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 确认已删除
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/access/{}", access_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── 记录规则 CRUD (admin) ──

#[tokio::test]
async fn test_record_rules_crud_admin() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    // 创建测试组
    let group = Group {
        id: GroupId::new(uuid::Uuid::new_v4().to_string()),
        name: "rr_test".into(),
        display_name: Some("RR测试组".into()),
        comment: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    state.store.create_group(&group).await.unwrap();
    let group_id_str = &group.id.0;

    // CREATE
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/rules",
            &admin,
            Some(&format!(
                r#"{{"name":"test_rule","group_id":"{}","model":"test_model","domain":"[]","perm_read":true,"perm_write":false,"perm_create":false,"perm_delete":false}}"#,
                group_id_str
            )),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let rule_id = json["id"].as_str().unwrap().to_string();
    assert_eq!(json["name"], "test_rule");
    assert_eq!(json["domain"], "[]");

    // LIST
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/rules", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(list.as_array().unwrap().len() >= 1);

    // GET
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/rules/{}", rule_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let got: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(got["name"], "test_rule");

    // UPDATE
    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            &format!("/api/rules/{}", rule_id),
            &admin,
            Some(&format!(
                r#"{{"name":"updated_rule","group_id":"{}","model":"test_model","domain":"[\"&\",[\"name\",\"=\",\"foo\"]]","perm_read":true,"perm_write":true,"perm_create":false,"perm_delete":false}}"#,
                group_id_str
            )),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["name"], "updated_rule");
    assert_eq!(updated["perm_write"], true);

    // DELETE
    let resp = app
        .clone()
        .oneshot(auth_request("DELETE", &format!("/api/rules/{}", rule_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 确认已删除
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/rules/{}", rule_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Generic CRUD (动态模型数据) ──

fn test_model_descriptor() -> ingjoo_core::module::ModelDescriptor {
    use ingjoo_core::module::*;
    let mut m = ModelDescriptor::new("test_item", "test_item");
    m.fields.push(FieldDescriptor {
        name: "name".into(),
        field_type: FieldType::Text,
        required: true,
        default_value: None,
        unique: false,
        relation: None,
    });
    m.fields.push(FieldDescriptor {
        name: "value".into(),
        field_type: FieldType::Integer,
        required: false,
        default_value: None,
        unique: false,
        relation: None,
    });
    m
}

#[tokio::test]
async fn test_generic_crud_full_flow() {
    let (app, state) = setup_app_with_models(vec![test_model_descriptor()]).await;
    let admin = get_admin_token(&app, &state).await;

    // 先确保表存在
    let resp = app
        .clone()
        .oneshot(auth_request("POST", "/api/data/test_item/ensure", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // CREATE
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/data/test_item",
            &admin,
            Some(r#"{"name":"item1","value":42}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let item_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "item1");
    assert_eq!(created["value"], 42);

    // LIST
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/data/test_item", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(list["items"].as_array().unwrap().len() >= 1);

    // READ
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/data/test_item/{}", item_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let got: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(got["name"], "item1");

    // UPDATE
    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            &format!("/api/data/test_item/{}", item_id),
            &admin,
            Some(r#"{"name":"item1_updated","value":99}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["name"], "item1_updated");
    assert_eq!(updated["value"], 99);

    // DELETE
    let resp = app
        .clone()
        .oneshot(auth_request("DELETE", &format!("/api/data/test_item/{}", item_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 确认已删除
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/data/test_item/{}", item_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
