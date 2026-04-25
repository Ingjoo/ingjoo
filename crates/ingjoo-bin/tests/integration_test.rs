use std::sync::Arc;

use axum::body::Body;
use axum::Router;
use http::StatusCode;
use ingjoo_core::db::ids::GroupId;
use ingjoo_core::db::models::Group;
use ingjoo_infra::{AppState, AuthConfig, AuthProvider, JwtAuthProvider, IngjooDb, IngjooStore, PluginManager, DatabaseManager};
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
    let state = Arc::new(AppState::new(store, auth, registry, Arc::new(pool.clone()), dialect, Arc::new(DatabaseManager::new(pool.clone(), dialect))));
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

    let registry = ModelRegistry::new();
    for model in models {
        registry.register(model);
    }
    let registry = Arc::new(registry);

    let state = Arc::new(AppState::new(store, auth, registry, Arc::new(pool.clone()), dialect, Arc::new(DatabaseManager::new(pool.clone(), dialect))));
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
        .create_access_token(user_id, "admin", &groups, None)
        .unwrap()
}

#[allow(dead_code)]
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
    assert!(!groups.as_array().unwrap().is_empty());
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
    assert!(!list.as_array().unwrap().is_empty());

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
    assert!(!list.as_array().unwrap().is_empty());

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
    assert!(!list["items"].as_array().unwrap().is_empty());

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

// ── 插件管理 ──

/// 创建带 PluginManager 的测试状态
async fn setup_app_with_plugins() -> (Router, Arc<AppState>) {
    let tmp = tempfile::Builder::new()
        .prefix("ingjoo_plugin_test_")
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
    let plugin_manager = Arc::new(PluginManager::new(registry.clone(), Arc::new(pool.clone()), dialect));
    let state = Arc::new(
        AppState::new(store, auth, registry, Arc::new(pool.clone()), dialect, Arc::new(DatabaseManager::new(pool.clone(), dialect)))
            .with_plugin_manager(plugin_manager),
    );
    let router = ingjoo_infra::router::base_router(state.clone());
    (router, state)
}

/// 创建测试插件 JSON 文件到临时目录，返回目录路径
fn create_test_plugin_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let json = r#"{
        "name": "test_plugin",
        "version": "1.0.0",
        "description": "测试插件",
        "models": [
            {
                "name": "test_item",
                "table_name": "test_items",
                "fields": [
                    {"name": "name", "field_type": "text", "required": true}
                ]
            }
        ]
    }"#;
    let path = dir.path().join("test_plugin.json");
    std::fs::write(&path, json).unwrap();
    dir
}

#[tokio::test]
async fn test_plugin_list_empty() {
    let (app, state) = setup_app_with_plugins().await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .oneshot(auth_request("GET", "/api/admin/plugins", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(list.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_plugin_load_from_dir() {
    let plugin_dir = create_test_plugin_dir();
    let (app, state) = setup_app_with_plugins().await;
    let admin = get_admin_token(&app, &state).await;

    // 加载插件
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/admin/plugins/load",
            &admin,
            Some(&format!(
                r#"{{"plugins_dir":"{}"}}"#,
                plugin_dir.path().display()
            )),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let loaded = json["loaded"].as_array().unwrap();
    assert!(loaded.iter().any(|v| v.as_str() == Some("test_plugin")));

    // 插件列表中应包含 test_plugin
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/admin/plugins", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let names: Vec<&str> = list.as_array().unwrap().iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    assert!(names.contains(&"test_plugin"));

    // 模型注册表中应包含 test_item
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/models", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let models: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let model_names: Vec<&str> = models.as_array().unwrap().iter()
        .filter_map(|m| m["name"].as_str())
        .collect();
    assert!(model_names.contains(&"test_item"));
}

#[tokio::test]
async fn test_plugin_unload() {
    let plugin_dir = create_test_plugin_dir();
    let (app, state) = setup_app_with_plugins().await;
    let admin = get_admin_token(&app, &state).await;

    // 先加载
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/admin/plugins/load",
            &admin,
            Some(&format!(
                r#"{{"plugins_dir":"{}"}}"#,
                plugin_dir.path().display()
            )),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 卸载
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/admin/plugins/test_plugin/unload",
            &admin,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 列表应为空
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/admin/plugins", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(list.as_array().unwrap().is_empty());

    // 模型应从注册表移除
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/models", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let models: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let model_names: Vec<&str> = models.as_array().unwrap().iter()
        .filter_map(|m| m["name"].as_str())
        .collect();
    assert!(!model_names.contains(&"test_item"));
}

#[tokio::test]
async fn test_plugin_requires_admin() {
    let (app, _state) = setup_app_with_plugins().await;

    // 注册普通用户（非 admin）
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"pluginuser@test.com","name":"normal","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    let resp = app
        .oneshot(auth_request("GET", "/api/admin/plugins", token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_plugin_load_invalid_dir() {
    let (app, state) = setup_app_with_plugins().await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .oneshot(auth_request(
            "POST",
            "/api/admin/plugins/load",
            &admin,
            Some(r#"{"plugins_dir":"/nonexistent/path/plugins"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ── 多租户集合隔离 ──────────────────────────────────────────

fn collection_test_model() -> ingjoo_core::module::ModelDescriptor {
    ingjoo_core::module::ModelDescriptor::new("col_item", "col_items")
        .required_field("name", ingjoo_core::FieldType::Text)
        .field("collection_id", ingjoo_core::FieldType::Text)
        .field("status", ingjoo_core::FieldType::Text)
}

fn simple_url_encode(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('[', "%5B")
        .replace(']', "%5D")
        .replace('"', "%22")
        .replace(',', "%2C")
        .replace('=', "%3D")
}

#[tokio::test]
async fn test_collection_isolation_crud_filter() {
    let (app, state) = setup_app_with_models(vec![collection_test_model()]).await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request("POST", "/api/data/col_item/ensure", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"doc_a1","collection_id":"col_a","status":"published"}"#),
        ))
        .await
        .unwrap();

    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"doc_a2","collection_id":"col_a","status":"draft"}"#),
        ))
        .await
        .unwrap();

    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"doc_b1","collection_id":"col_b","status":"published"}"#),
        ))
        .await
        .unwrap();

    let domain_filter = simple_url_encode(r#"["collection_id", "=", "col_a"]"#);
    let resp = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/api/data/col_item?domain={}", domain_filter),
            &admin,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "只应返回 col_a 的 2 条记录");
    for item in items {
        assert_eq!(item["collection_id"].as_str().unwrap(), "col_a");
    }
}

/// 验证通过列表查询 domain 过滤，跨集合记录不出现在结果中
#[tokio::test]
async fn test_collection_isolation_cross_collection_denied() {
    let (app, state) = setup_app_with_models(vec![collection_test_model()]).await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request("POST", "/api/data/col_item/ensure", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 在 col_a 和 col_b 各创建一条记录
    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"doc_a","collection_id":"col_a","status":"published"}"#),
        ))
        .await
        .unwrap();
    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"secret_doc","collection_id":"col_b","status":"published"}"#),
        ))
        .await
        .unwrap();

    // 用 domain 过滤只查 col_a — secret_doc 不应出现在结果中
    let domain_filter = simple_url_encode(r#"["collection_id", "=", "col_a"]"#);
    let resp = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/api/data/col_item?domain={}", domain_filter),
            &admin,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let items = list["items"].as_array().unwrap();
    // 只应返回 col_a 的记录，secret_doc（col_b）被排除
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"].as_str().unwrap(), "doc_a");
    assert_eq!(items[0]["collection_id"].as_str().unwrap(), "col_a");
}

#[tokio::test]
async fn test_collection_isolation_with_domain_dsl() {
    let (app, state) = setup_app_with_models(vec![collection_test_model()]).await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request("POST", "/api/data/col_item/ensure", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"item1","collection_id":"col1","status":"active"}"#),
        ))
        .await
        .unwrap();

    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"item2","collection_id":"col2","status":"active"}"#),
        ))
        .await
        .unwrap();

    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"item3","collection_id":"col1","status":"inactive"}"#),
        ))
        .await
        .unwrap();

    // 使用隐式 AND: [cond1, cond2] — collection_id=col1 AND status=active
    let domain_filter = simple_url_encode(
        r#"[["collection_id", "=", "col1"], ["status", "=", "active"]]"#,
    );
    let resp = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/api/data/col_item?domain={}", domain_filter),
            &admin,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "只应返回 col1+active 的 1 条记录");
    assert_eq!(items[0]["name"].as_str().unwrap(), "item1");
}

#[tokio::test]
async fn test_collection_isolation_security_policy_layer3() {
    use ingjoo_security::{SecurityBuilder, ModelAccess, AccessOp};

    let mut builder = SecurityBuilder::new();
    builder.policy().add_model_access(ModelAccess {
        model: "col_item".to_string(),
        role: "tenant_admin".to_string(),
        read: true, write: true, create: true, delete: true,
        import: false, export: false,
    });
    let policy = builder.build();

    let ids = vec!["tenant_001".to_string(), "tenant_002".to_string()];
    let filter = policy.collection_isolation(&ids, None);

    assert!(policy.check_access("col_item", "tenant_admin", AccessOp::Read));
    assert_eq!(filter.clause, "collection_id IN (?, ?)");
    assert_eq!(filter.params, vec!["tenant_001", "tenant_002"]);

    let empty_filter = policy.collection_isolation(&[], None);
    assert_eq!(empty_filter.clause, "1=0");
}

#[tokio::test]
async fn test_collection_isolation_generic_db_filter() {
    use ingjoo_infra::db::generic::GenericDb;
    use ingjoo_core::query::domain::SqlCondition;

    let (app, state) = setup_app_with_models(vec![collection_test_model()]).await;
    let admin = get_admin_token(&app, &state).await;

    let resp = app
        .clone()
        .oneshot(auth_request("POST", "/api/data/col_item/ensure", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"g1","collection_id":"tenant_x"}"#),
        ))
        .await
        .unwrap();

    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"g2","collection_id":"tenant_y"}"#),
        ))
        .await
        .unwrap();

    app.clone()
        .oneshot(auth_request(
            "POST", "/api/data/col_item", &admin,
            Some(r#"{"name":"g3","collection_id":"tenant_x"}"#),
        ))
        .await
        .unwrap();

    let model = collection_test_model();
    let generic = GenericDb::new(&state.pool, &state.dialect);

    let col_filter = SqlCondition {
        clause: "collection_id = ?".to_string(),
        params: vec!["tenant_x".to_string()],
    };
    let result = generic.generic_list_with_filter(&model, None, Some(&col_filter), 50, 0).await.unwrap();
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);
    for item in &result.items {
        assert_eq!(item["collection_id"].as_str().unwrap(), "tenant_x");
    }

    let empty_col_filter = SqlCondition {
        clause: "1=0".to_string(),
        params: vec![],
    };
    let empty_result = generic.generic_list_with_filter(&model, None, Some(&empty_col_filter), 50, 0).await.unwrap();
    assert_eq!(empty_result.items.len(), 0);
}

// ── 集成测试扩展 ──

/// 1. 验证健康检查端点返回连接池统计信息
#[tokio::test]
async fn test_health_has_pool_stats() {
    let app = setup_app().await;
    let resp = app
        .oneshot(make_request("GET", "/api/health", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // pool 字段应存在且为对象
    let pool = &json["pool"];
    assert!(pool.is_object(), "health 响应应包含 pool 对象");

    // 验证四个连接池统计字段存在且 >= 0
    assert!(pool["total_connections"].is_number(), "pool.total_connections 应为数字");
    assert!(pool["idle_connections"].is_number(), "pool.idle_connections 应为数字");
    assert!(pool["active_connections"].is_number(), "pool.active_connections 应为数字");
    assert!(pool["max_connections"].is_number(), "pool.max_connections 应为数字");

    assert!(pool["total_connections"].as_u64().is_some());
    assert!(pool["idle_connections"].as_u64().is_some());
    assert!(pool["active_connections"].as_u64().is_some());
    assert!(pool["max_connections"].as_u64().unwrap() > 0, "max_connections 应大于 0");
}

/// 2. 验证用户资料更新功能
#[tokio::test]
async fn test_profile_update() {
    let app = setup_app().await;

    // 注册用户
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"profile@test.com","name":"TestUser","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    // 更新用户名
    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            "/api/auth/profile",
            token,
            Some(r#"{"name":"Updated Name"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["name"].as_str().unwrap(), "Updated Name");

    // GET 验证更新持久化
    let resp = app
        .oneshot(auth_request("GET", "/api/auth/profile", token, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let profile: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(profile["name"].as_str().unwrap(), "Updated Name");
}

/// 3. 验证动态模型记录的更新操作
#[tokio::test]
async fn test_crud_update_record() {
    let (app, state) = setup_app_with_models(vec![
        ingjoo_core::module::ModelDescriptor::new("upd_item", "upd_items")
            .required_field("name", ingjoo_core::FieldType::Text)
            .field("status", ingjoo_core::FieldType::Text),
    ]).await;
    let admin = get_admin_token(&app, &state).await;

    // 确保表存在
    let resp = app
        .clone()
        .oneshot(auth_request("POST", "/api/data/upd_item/ensure", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 创建记录
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/data/upd_item",
            &admin,
            Some(r#"{"name":"item1","status":"active"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let item_id = created["id"].as_str().unwrap().to_string();

    // 更新记录
    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            &format!("/api/data/upd_item/{}", item_id),
            &admin,
            Some(r#"{"status":"inactive"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["status"].as_str().unwrap(), "inactive");

    // GET 验证更新生效
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/data/upd_item/{}", item_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let got: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(got["status"].as_str().unwrap(), "inactive");
}

/// 4. 验证动态模型记录的删除操作
#[tokio::test]
async fn test_crud_delete_record() {
    let (app, state) = setup_app_with_models(vec![
        ingjoo_core::module::ModelDescriptor::new("del_item", "del_items")
            .required_field("name", ingjoo_core::FieldType::Text),
    ]).await;
    let admin = get_admin_token(&app, &state).await;

    // 确保表存在
    let resp = app
        .clone()
        .oneshot(auth_request("POST", "/api/data/del_item/ensure", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 创建记录
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/data/del_item",
            &admin,
            Some(r#"{"name":"to_delete"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let item_id = created["id"].as_str().unwrap().to_string();

    // 删除记录
    let resp = app
        .clone()
        .oneshot(auth_request("DELETE", &format!("/api/data/del_item/{}", item_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET 应返回 404
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/data/del_item/{}", item_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// 5. 验证非管理员无法创建视图（写操作需要管理员权限）
#[tokio::test]
async fn test_view_nonadmin_forbidden() {
    let app = setup_app().await;
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"viewuser@test.com","name":"normal","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    // 非管理员创建视图应被拒绝
    let resp = app
        .oneshot(auth_request(
            "POST",
            "/api/views",
            token,
            Some(r#"{"name":"非法视图","model":"test","view_type":"form","arch":{"fields":[]}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

/// 6. 验证非管理员无法创建动作（写操作需要管理员权限）
#[tokio::test]
async fn test_action_nonadmin_forbidden() {
    let app = setup_app().await;
    let resp = app
        .clone()
        .oneshot(make_request(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"actionuser@test.com","name":"normal","password":"pass123"}"#),
        ))
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["access_token"].as_str().unwrap();

    // 非管理员创建动作应被拒绝
    let resp = app
        .oneshot(auth_request(
            "POST",
            "/api/actions",
            token,
            Some(r#"{"name":"非法动作","action_type":"act_window","res_model":"test","view_mode":["list"],"view_ids":[]}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

/// 7. 验证受保护路由的速率限制（30 token 突发，超过后返回 429）
#[tokio::test]
async fn test_rate_limit_protected_route() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    let mut got_429 = false;
    // 受保护路由有 30 token 限制，发送 40+ 请求触发限流
    for _ in 0..45 {
        let resp = app
            .clone()
            .oneshot(auth_request("GET", "/api/models", &admin, None))
            .await
            .unwrap();
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            got_429 = true;
            break;
        }
    }
    assert!(got_429, "连续请求后应至少收到一个 429 响应");
}

/// 8. 验证调度任务的完整 CRUD 流程
#[tokio::test]
async fn test_schedule_crud_full_flow() {
    let (app, state) = setup_app_with_state().await;
    let admin = get_admin_token(&app, &state).await;

    // 创建调度任务（cron 6 字段格式: sec min hour dom month dow）
    let resp = app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/schedules",
            &admin,
            Some(r#"{"name":"测试调度","cron_expr":"0 0 * * * *","job_name":"test_job","payload":{"key":"value"}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let schedule_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"].as_str().unwrap(), "测试调度");
    assert_eq!(created["cron_expr"].as_str().unwrap(), "0 0 * * * *");
    assert_eq!(created["job_name"].as_str().unwrap(), "test_job");

    // 列出调度任务，验证创建成功
    let resp = app
        .clone()
        .oneshot(auth_request("GET", "/api/schedules", &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(list.as_array().unwrap().iter().any(|s| s["id"].as_str() == Some(&schedule_id)));

    // 更新调度任务
    let resp = app
        .clone()
        .oneshot(auth_request(
            "PUT",
            &format!("/api/schedules/{}", schedule_id),
            &admin,
            Some(r#"{"name":"更新调度","cron_expr":"0 0 0 * * *"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["name"].as_str().unwrap(), "更新调度");
    assert_eq!(updated["cron_expr"].as_str().unwrap(), "0 0 0 * * *");

    // 删除调度任务
    let resp = app
        .clone()
        .oneshot(auth_request("DELETE", &format!("/api/schedules/{}", schedule_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 验证已删除（GET 返回 404）
    let resp = app
        .clone()
        .oneshot(auth_request("GET", &format!("/api/schedules/{}", schedule_id), &admin, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_cache_scope_access_put_and_get() {
    let (_, state) = setup_app_with_state().await;

    assert!(state.cache.get_scope_access("policy", "admin").is_none());

    let policy = ingjoo_security::SecurityPolicy::new();
    state.cache.put_scope_access("policy", "admin", policy.clone());

    let cached = state.cache.get_scope_access("policy", "admin");
    assert!(cached.is_some(), "put 后应有缓存");
}

#[tokio::test]
async fn test_cache_scope_access_different_keys() {
    let (_, state) = setup_app_with_state().await;

    let policy = ingjoo_security::SecurityPolicy::new();
    state.cache.put_scope_access("policy", "admin", policy.clone());
    state.cache.put_scope_access("policy", "user", policy.clone());

    assert!(state.cache.get_scope_access("policy", "admin").is_some());
    assert!(state.cache.get_scope_access("policy", "user").is_some());
    assert!(state.cache.get_scope_access("policy", "guest").is_none());
}

#[tokio::test]
async fn test_cache_scope_invalidation() {
    let (_, state) = setup_app_with_state().await;

    let policy = ingjoo_security::SecurityPolicy::new();
    state.cache.put_scope_access("policy", "admin", policy.clone());
    state.cache.put_scope_access("policy", "user", policy.clone());

    assert!(state.cache.get_scope_access("policy", "admin").is_some());
    assert!(state.cache.get_scope_access("policy", "user").is_some());

    state.cache.invalidate_scope("policy");

    assert!(state.cache.get_scope_access("policy", "admin").is_none());
    assert!(state.cache.get_scope_access("policy", "user").is_none());
}

// ── 搜索端点集成测试 ──

#[tokio::test]
async fn test_search_returns_matching_records() {
    let (app, state) = setup_app_with_models(vec![
        ingjoo_core::module::ModelDescriptor::new("search_article", "search_articles")
            .required_field("title", ingjoo_core::FieldType::Text)
            .field("body", ingjoo_core::FieldType::Text),
    ]).await;
    let admin = get_admin_token(&app, &state).await;

    // ensure 表
    let resp = app.clone().oneshot(auth_request("POST", "/api/data/search_article/ensure", &admin, None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 创建两条记录
    let resp = app.clone().oneshot(auth_request(
        "POST", "/api/data/search_article",
        &admin, Some(r#"{"title":"Rust 编程入门","body":"Rust 是一门系统级编程语言"}"#),
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = app.clone().oneshot(auth_request(
        "POST", "/api/data/search_article",
        &admin, Some(r#"{"title":"Python 数据分析","body":"Python 适合数据处理"}"#),
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 搜索 "Rust"
    let resp = app.clone().oneshot(auth_request(
        "GET", "/api/data/search_article/search?q=Rust", &admin, None,
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let results = json["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0]["title"].as_str().unwrap().contains("Rust"));

    // 搜索 "编程" — 匹配第一条的 title
    let resp = app.clone().oneshot(auth_request(
        "GET", "/api/data/search_article/search?q=%E7%BC%96%E7%A8%8B", &admin, None,
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(!json["results"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_search_no_match_returns_empty() {
    let (app, state) = setup_app_with_models(vec![
        ingjoo_core::module::ModelDescriptor::new("search_empty", "search_empty_table")
            .required_field("title", ingjoo_core::FieldType::Text),
    ]).await;
    let admin = get_admin_token(&app, &state).await;

    // ensure + 创建一条
    app.clone().oneshot(auth_request("POST", "/api/data/search_empty/ensure", &admin, None)).await.unwrap();
    app.clone().oneshot(auth_request(
        "POST", "/api/data/search_empty",
        &admin, Some(r#"{"title":"hello world"}"#),
    )).await.unwrap();

    // 搜索不存在的关键词
    let resp = app.clone().oneshot(auth_request(
        "GET", "/api/data/search_empty/search?q=nonexistent_keyword_xyz", &admin, None,
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["results"].as_array().unwrap().is_empty());
    assert_eq!(json["total"].as_i64().unwrap(), 0);
}

#[tokio::test]
async fn test_search_empty_query_returns_400() {
    let (app, state) = setup_app_with_models(vec![
        ingjoo_core::module::ModelDescriptor::new("search_bad", "search_bad_table")
            .required_field("title", ingjoo_core::FieldType::Text),
    ]).await;
    let admin = get_admin_token(&app, &state).await;

    app.clone().oneshot(auth_request("POST", "/api/data/search_bad/ensure", &admin, None)).await.unwrap();

    let resp = app.clone().oneshot(auth_request(
        "GET", "/api/data/search_bad/search?q=", &admin, None,
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
