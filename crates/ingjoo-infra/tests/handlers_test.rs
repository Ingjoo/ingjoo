#[cfg(feature = "mock")]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::Router;
    use http::StatusCode;
    use ingjoo_core::ModelRegistry;
    use ingjoo_infra::{AppState, AuthConfig, DatabaseManager, IngjooStore, JwtAuthProvider, MockIngjooDb};
    use tower::ServiceExt;

    async fn setup_app() -> Router {
        let store: Arc<dyn IngjooStore> = Arc::new(MockIngjooDb::new());
        let auth = JwtAuthProvider::new(&AuthConfig::new("test-secret"));
        let registry = Arc::new(ModelRegistry::new());
        ingjoo_core::pool::install_drivers();
        let (pool, dialect) = ingjoo_core::pool::connect_pool("sqlite::memory:").await.unwrap();
        let state = Arc::new(AppState::new(
            store,
            auth,
            registry,
            Arc::new(pool.clone()),
            dialect,
            Arc::new(DatabaseManager::new(pool.clone(), dialect)),
        ));
        ingjoo_infra::router::base_router(state)
    }

    fn make_request(method: &str, uri: &str, body: Option<&str>) -> http::Request<Body> {
        let mut builder = http::Request::builder().method(method).uri(uri);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        builder.body(Body::from(body.unwrap_or("").to_string())).unwrap()
    }

    fn auth_request(method: &str, uri: &str, token: &str, body: Option<&str>) -> http::Request<Body> {
        let mut builder =
            http::Request::builder().method(method).uri(uri).header("authorization", format!("Bearer {}", token));
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        builder.body(Body::from(body.unwrap_or("").to_string())).unwrap()
    }

    async fn register_user(app: &Router, email: &str, name: &str, password: &str) -> serde_json::Value {
        let resp = app
            .clone()
            .oneshot(make_request(
                "POST",
                "/api/auth/register",
                Some(&format!(r#"{{"email":"{}","name":"{}","password":"{}"}}"#, email, name, password)),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    // ==================== Register ====================

    #[tokio::test]
    async fn test_register_creates_user_with_correct_fields() {
        let app = setup_app().await;
        let json = register_user(&app, "new@example.com", "newuser", "pass123").await;
        assert_eq!(json["user"]["email"].as_str().unwrap(), "new@example.com");
        assert_eq!(json["user"]["name"].as_str().unwrap(), "newuser");
        assert_eq!(json["user"]["role"].as_str().unwrap(), "user");
    }

    #[tokio::test]
    async fn test_register_returns_access_and_refresh_tokens() {
        let app = setup_app().await;
        let json = register_user(&app, "token@example.com", "tokenuser", "pass123").await;
        assert!(json["access_token"].is_string());
        assert!(!json["access_token"].as_str().unwrap().is_empty());
        assert!(json["refresh_token"].is_string());
        assert!(!json["refresh_token"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_register_duplicate_email_returns_400() {
        let app = setup_app().await;
        register_user(&app, "dup@example.com", "user1", "pass123").await;
        let resp = app
            .oneshot(make_request(
                "POST",
                "/api/auth/register",
                Some(r#"{"email":"dup@example.com","name":"user2","password":"pass123"}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // ==================== Login ====================

    #[tokio::test]
    async fn test_login_correct_password() {
        let app = setup_app().await;
        register_user(&app, "login@example.com", "loginuser", "mypass123").await;

        let resp = app
            .oneshot(make_request(
                "POST",
                "/api/auth/login",
                Some(r#"{"email":"login@example.com","password":"mypass123"}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["access_token"].is_string());
        assert!(json["refresh_token"].is_string());
    }

    #[tokio::test]
    async fn test_login_wrong_password_returns_401() {
        let app = setup_app().await;
        register_user(&app, "wrong@example.com", "wronguser", "correct-pass").await;

        let resp = app
            .oneshot(make_request(
                "POST",
                "/api/auth/login",
                Some(r#"{"email":"wrong@example.com","password":"wrong-pass"}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_login_nonexistent_email_returns_401() {
        let app = setup_app().await;
        let resp = app
            .oneshot(make_request(
                "POST",
                "/api/auth/login",
                Some(r#"{"email":"nobody@example.com","password":"pass"}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ==================== Refresh ====================

    #[tokio::test]
    async fn test_refresh_returns_new_tokens() {
        let app = setup_app().await;
        let reg = register_user(&app, "refresh@example.com", "refreshuser", "pass123").await;
        let refresh_token = reg["refresh_token"].as_str().unwrap();

        let resp = app
            .clone()
            .oneshot(make_request(
                "POST",
                "/api/auth/refresh",
                Some(&format!(r#"{{"refresh_token":"{}"}}"#, refresh_token)),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["access_token"].is_string());
        assert!(json["refresh_token"].is_string());
        assert_ne!(json["refresh_token"].as_str().unwrap(), refresh_token, "new refresh token should differ from old");
    }

    #[tokio::test]
    async fn test_refresh_old_token_invalid_after_use() {
        let app = setup_app().await;
        let reg = register_user(&app, "rotate@example.com", "rotateuser", "pass123").await;
        let refresh_token = reg["refresh_token"].as_str().unwrap();

        let resp = app
            .clone()
            .oneshot(make_request(
                "POST",
                "/api/auth/refresh",
                Some(&format!(r#"{{"refresh_token":"{}"}}"#, refresh_token)),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(make_request(
                "POST",
                "/api/auth/refresh",
                Some(&format!(r#"{{"refresh_token":"{}"}}"#, refresh_token)),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "old refresh token should be deleted");
    }

    #[tokio::test]
    async fn test_refresh_invalid_token_returns_401() {
        let app = setup_app().await;
        let resp = app
            .oneshot(make_request("POST", "/api/auth/refresh", Some(r#"{"refresh_token":"nonexistent-token"}"#)))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ==================== Profile ====================

    #[tokio::test]
    async fn test_get_profile_with_valid_token() {
        let app = setup_app().await;
        let reg = register_user(&app, "profile@example.com", "profileuser", "pass123").await;
        let access_token = reg["access_token"].as_str().unwrap();

        let resp = app.oneshot(auth_request("GET", "/api/auth/profile", access_token, None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let profile: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(profile["email"].as_str().unwrap(), "profile@example.com");
    }

    #[tokio::test]
    async fn test_get_profile_without_token_returns_401() {
        let app = setup_app().await;
        let resp = app.oneshot(make_request("GET", "/api/auth/profile", None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_get_profile_with_invalid_token_returns_401() {
        let app = setup_app().await;
        let resp = app.oneshot(auth_request("GET", "/api/auth/profile", "invalid.jwt.token", None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ==================== Update Profile ====================

    #[tokio::test]
    async fn test_update_profile_name_and_bio() {
        let app = setup_app().await;
        let reg = register_user(&app, "update@example.com", "updateuser", "pass123").await;
        let access_token = reg["access_token"].as_str().unwrap();

        let resp = app
            .clone()
            .oneshot(auth_request(
                "PUT",
                "/api/auth/profile",
                access_token,
                Some(r#"{"name":"new_name","bio":"new bio"}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(updated["name"].as_str().unwrap(), "new_name");
        assert_eq!(updated["bio"].as_str().unwrap(), "new bio");
    }

    #[tokio::test]
    async fn test_update_profile_without_token_returns_401() {
        let app = setup_app().await;
        let resp = app.oneshot(make_request("PUT", "/api/auth/profile", Some(r#"{"name":"hacker"}"#))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ==================== Full Flow ====================

    #[tokio::test]
    async fn test_full_register_login_refresh_profile_flow() {
        let app = setup_app().await;

        let reg = register_user(&app, "flow@example.com", "flowuser", "flowpass").await;
        let _access = reg["access_token"].as_str().unwrap();
        let refresh = reg["refresh_token"].as_str().unwrap();

        let resp = app
            .clone()
            .oneshot(make_request(
                "POST",
                "/api/auth/login",
                Some(r#"{"email":"flow@example.com","password":"flowpass"}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .clone()
            .oneshot(make_request("POST", "/api/auth/refresh", Some(&format!(r#"{{"refresh_token":"{}"}}"#, refresh))))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let new_tokens: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let new_access = new_tokens["access_token"].as_str().unwrap();

        let resp = app.clone().oneshot(auth_request("GET", "/api/auth/profile", new_access, None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let profile: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(profile["email"].as_str().unwrap(), "flow@example.com");

        let resp = app
            .oneshot(auth_request("PUT", "/api/auth/profile", new_access, Some(r#"{"name":"flow_updated"}"#)))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(updated["name"].as_str().unwrap(), "flow_updated");
    }

    // ==================== Search Favorites ====================

    async fn setup_app_with_favorites() -> Router {
        let store: Arc<dyn IngjooStore> = Arc::new(MockIngjooDb::new());
        let auth = JwtAuthProvider::new(&AuthConfig::new("test-secret"));
        let registry = Arc::new(ModelRegistry::new());
        ingjoo_core::pool::install_drivers();
        let (pool, dialect) =
            ingjoo_core::pool::connect_pool_with_options("sqlite::memory:", 1, 0, 5, 30, 60)
                .await
                .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS ir_search_favorite (\
             id INTEGER PRIMARY KEY AUTOINCREMENT, \
             user_id TEXT NOT NULL, \
             name TEXT NOT NULL, \
             model TEXT NOT NULL, \
             domain TEXT, \
             context TEXT, \
             is_default INTEGER NOT NULL DEFAULT 0, \
             created_at TEXT NOT NULL DEFAULT (datetime('now')), \
             updated_at TEXT NOT NULL DEFAULT (datetime('now')))",
        )
        .execute(&pool)
        .await
        .unwrap();

        let state = Arc::new(AppState::new(
            store,
            auth,
            registry,
            Arc::new(pool.clone()),
            dialect,
            Arc::new(DatabaseManager::new(pool.clone(), dialect)),
        ));
        ingjoo_infra::router::base_router(state)
    }

    #[tokio::test]
    async fn test_search_favorites_list_empty() {
        let app = setup_app_with_favorites().await;
        let reg = register_user(&app, "sf_empty@example.com", "sfuser", "pass123").await;
        let token = reg["access_token"].as_str().unwrap();

        let resp = app
            .oneshot(auth_request("GET", "/api/search-favorites?model=article", token, None))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(list.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_search_favorites_create_and_list() {
        let app = setup_app_with_favorites().await;
        let reg = register_user(&app, "sf_cl@example.com", "sfuser", "pass123").await;
        let token = reg["access_token"].as_str().unwrap();

        let resp = app
            .clone()
            .oneshot(auth_request(
                "POST",
                "/api/search-favorites",
                token,
                Some(r#"{"name":"My Filter","model":"article","domain":[["status","=","published"]],"is_default":false}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(auth_request("GET", "/api/search-favorites?model=article", token, None))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = list.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"].as_str().unwrap(), "My Filter");
        assert_eq!(arr[0]["model"].as_str().unwrap(), "article");
        assert!(!arr[0]["is_default"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_search_favorites_create_returns_id() {
        let app = setup_app_with_favorites().await;
        let reg = register_user(&app, "sf_id@example.com", "sfuser", "pass123").await;
        let token = reg["access_token"].as_str().unwrap();

        let resp = app
            .oneshot(auth_request(
                "POST",
                "/api/search-favorites",
                token,
                Some(r#"{"name":"Test Fav","model":"article","domain":null,"is_default":false}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let fav: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(fav["id"].as_i64().unwrap() > 0);
        assert_eq!(fav["name"].as_str().unwrap(), "Test Fav");
    }

    #[tokio::test]
    async fn test_search_favorites_delete_own() {
        let app = setup_app_with_favorites().await;
        let reg = register_user(&app, "sf_del@example.com", "sfuser", "pass123").await;
        let token = reg["access_token"].as_str().unwrap();

        let resp = app
            .clone()
            .oneshot(auth_request(
                "POST",
                "/api/search-favorites",
                token,
                Some(r#"{"name":"To Delete","model":"article","is_default":false}"#),
            ))
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let fav: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let fav_id = fav["id"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(auth_request("DELETE", &format!("/api/search-favorites/{}", fav_id), token, None))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(auth_request("GET", "/api/search-favorites?model=article", token, None))
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(list.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_search_favorites_delete_others_forbidden() {
        let app = setup_app_with_favorites().await;
        let reg1 = register_user(&app, "sf_u1@example.com", "user1", "pass123").await;
        let token1 = reg1["access_token"].as_str().unwrap();
        let reg2 = register_user(&app, "sf_u2@example.com", "user2", "pass123").await;
        let token2 = reg2["access_token"].as_str().unwrap();

        let resp = app
            .clone()
            .oneshot(auth_request(
                "POST",
                "/api/search-favorites",
                token1,
                Some(r#"{"name":"Owned by u1","model":"article","is_default":false}"#),
            ))
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let fav: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let fav_id = fav["id"].as_i64().unwrap();

        let resp = app
            .oneshot(auth_request("DELETE", &format!("/api/search-favorites/{}", fav_id), token2, None))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_search_favorites_set_default() {
        let app = setup_app_with_favorites().await;
        let reg = register_user(&app, "sf_def@example.com", "sfuser", "pass123").await;
        let token = reg["access_token"].as_str().unwrap();

        let resp = app
            .clone()
            .oneshot(auth_request(
                "POST",
                "/api/search-favorites",
                token,
                Some(r#"{"name":"Fav A","model":"article","is_default":false}"#),
            ))
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let fav_a: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let id_a = fav_a["id"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(auth_request(
                "POST",
                "/api/search-favorites",
                token,
                Some(r#"{"name":"Fav B","model":"article","is_default":true}"#),
            ))
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let fav_b: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let _id_b = fav_b["id"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(auth_request("PUT", &format!("/api/search-favorites/{}/default", id_a), token, None))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(auth_request("GET", "/api/search-favorites?model=article", token, None))
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = list.as_array().unwrap();
        let fav_a_item = arr.iter().find(|f| f["id"].as_i64().unwrap() == id_a).unwrap();
        assert!(fav_a_item["is_default"].as_bool().unwrap());
    }
}
