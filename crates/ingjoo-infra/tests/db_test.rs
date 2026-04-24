use std::sync::Arc;

use ingjoo_core::db::ids::UserId;
use ingjoo_core::db::models::User;
use ingjoo_infra::{IngjooDb, IngjooStore};

async fn setup_db() -> Arc<dyn IngjooStore> {
    let tmp = tempfile::Builder::new()
        .prefix("infra_test_")
        .suffix(".db")
        .tempfile()
        .unwrap();
    let db_path = tmp.path().to_str().unwrap().to_string();
    std::mem::forget(tmp);

    let db_url = format!("sqlite://{}?mode=rwc", db_path);
    ingjoo_core::pool::install_drivers();
    let (pool, dialect) = ingjoo_core::pool::connect_pool(&db_url).await.unwrap();
    IngjooDb::run_migrations(&pool, &dialect).await.unwrap();
    Arc::new(IngjooDb::with_dialect(pool, dialect))
}

fn make_user(email: &str, name: &str) -> User {
    User {
        id: UserId::new(uuid::Uuid::new_v4().to_string()),
        email: email.to_string(),
        name: name.to_string(),
        password_hash: Some("hashed_password".to_string()),
        avatar_url: None,
        bio: None,
        role: "user".to_string(),
        oauth_provider: None,
        oauth_id: None,
        phone: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

// ==================== User CRUD ====================

#[tokio::test]
async fn test_create_and_get_user_by_id() {
    let store = setup_db().await;
    let user = make_user("crud@example.com", "cruduser");
    let created = store.create_user(&user).await.unwrap();
    assert_eq!(created.email, "crud@example.com");

    let found = store.get_user_by_id(&user.id).await.unwrap().unwrap();
    assert_eq!(found.email, "crud@example.com");
    assert_eq!(found.name, "cruduser");
}

#[tokio::test]
async fn test_create_and_get_user_by_email() {
    let store = setup_db().await;
    let user = make_user("byemail@example.com", "emailuser");
    store.create_user(&user).await.unwrap();

    let found = store.get_user_by_email("byemail@example.com").await.unwrap().unwrap();
    assert_eq!(found.name, "emailuser");
}

#[tokio::test]
async fn test_get_user_nonexistent_returns_none() {
    let store = setup_db().await;
    let result = store.get_user_by_email("nobody@example.com").await.unwrap();
    assert!(result.is_none());

    let result = store.get_user_by_id(&UserId::new("nonexistent-id")).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_update_user_name_and_bio() {
    let store = setup_db().await;
    let user = make_user("updateme@example.com", "original");
    store.create_user(&user).await.unwrap();

    let updated = store
        .update_user(&user.id, Some("updated_name"), None, Some("my bio"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.name, "updated_name");
    assert_eq!(updated.bio.unwrap(), "my bio");
}

#[tokio::test]
async fn test_update_user_role() {
    let store = setup_db().await;
    let user = make_user("role@example.com", "roleuser");
    store.create_user(&user).await.unwrap();

    let updated = store
        .update_user_role(&user.id, "admin")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.role, "admin");
}

#[tokio::test]
async fn test_update_user_password() {
    let store = setup_db().await;
    let user = make_user("pwd@example.com", "pwduser");
    store.create_user(&user).await.unwrap();

    let updated = store
        .update_user_password(&user.id, "new_hashed_password")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.password_hash.unwrap(), "new_hashed_password");
}

#[tokio::test]
async fn test_list_users_pagination() {
    let store = setup_db().await;
    for i in 0..5 {
        let user = make_user(&format!("list{}@example.com", i), &format!("listuser{}", i));
        store.create_user(&user).await.unwrap();
    }

    let page1 = store.list_users(2, 0).await.unwrap();
    assert_eq!(page1.items.len(), 2);
    assert_eq!(page1.total, 5);

    let page2 = store.list_users(2, 2).await.unwrap();
    assert_eq!(page2.items.len(), 2);
}

// ==================== Refresh Token ====================

#[tokio::test]
async fn test_refresh_token_create_get_delete() {
    let store = setup_db().await;
    let user = make_user("token@example.com", "tokenuser");
    store.create_user(&user).await.unwrap();

    let token_hash = "abc123hash";
    let expires_at = "2099-12-31T23:59:59+00:00";
    store
        .create_refresh_token("rt-1", &user.id, token_hash, expires_at)
        .await
        .unwrap();

    let (found_user_id, found_expires) = store
        .get_refresh_token(token_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found_user_id, user.id.to_string());
    assert_eq!(found_expires, expires_at);

    store.delete_refresh_token(token_hash).await.unwrap();
    let result = store.get_refresh_token(token_hash).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_nonexistent_refresh_token() {
    let store = setup_db().await;
    let result = store.get_refresh_token("nope").await.unwrap();
    assert!(result.is_none());
}

// ==================== Settings ====================

#[tokio::test]
async fn test_set_and_get_setting() {
    let store = setup_db().await;
    store.set_setting("app.name", "ingjoo").await.unwrap();

    let val = store.get_setting("app.name").await.unwrap().unwrap();
    assert_eq!(val, "ingjoo");
}

#[tokio::test]
async fn test_get_nonexistent_setting() {
    let store = setup_db().await;
    let result = store.get_setting("nonexistent.key").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_set_setting_overwrites() {
    let store = setup_db().await;
    store.set_setting("app.theme", "dark").await.unwrap();
    store.set_setting("app.theme", "light").await.unwrap();

    let val = store.get_setting("app.theme").await.unwrap().unwrap();
    assert_eq!(val, "light");
}

#[tokio::test]
async fn test_get_all_settings() {
    let store = setup_db().await;
    store.set_setting("k1", "v1").await.unwrap();
    store.set_setting("k2", "v2").await.unwrap();

    let all = store.get_all_settings().await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all.get("k1").unwrap(), "v1");
    assert_eq!(all.get("k2").unwrap(), "v2");
}

#[tokio::test]
async fn test_set_settings_batch() {
    let store = setup_db().await;
    store
        .set_settings(&[
            ("batch1".to_string(), "val1".to_string()),
            ("batch2".to_string(), "val2".to_string()),
        ])
        .await
        .unwrap();

    assert_eq!(
        store.get_setting("batch1").await.unwrap().unwrap(),
        "val1"
    );
    assert_eq!(
        store.get_setting("batch2").await.unwrap().unwrap(),
        "val2"
    );
}

#[tokio::test]
async fn test_seed_settings_no_overwrite() {
    let store = setup_db().await;
    store.set_setting("seed.key", "original").await.unwrap();
    store
        .seed_settings(&[("seed.key".to_string(), "new_value".to_string())])
        .await
        .unwrap();

    let val = store.get_setting("seed.key").await.unwrap().unwrap();
    assert_eq!(val, "original", "seed should not overwrite existing");
}

// ==================== Module Settings ====================

#[tokio::test]
async fn test_module_setting_set_and_get() {
    let store = setup_db().await;
    let ms = store
        .set_module_setting(
            "ms-1",
            "system",
            None,
            "auth",
            "max_attempts",
            "5",
        )
        .await
        .unwrap();
    assert_eq!(ms.value, "5");

    let val = store
        .get_module_setting("system", None, "auth", "max_attempts")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(val, "5");
}

#[tokio::test]
async fn test_module_setting_list() {
    let store = setup_db().await;
    store
        .set_module_setting("ms-2", "system", None, "auth", "k1", "v1")
        .await
        .unwrap();
    store
        .set_module_setting("ms-3", "system", None, "auth", "k2", "v2")
        .await
        .unwrap();
    store
        .set_module_setting("ms-4", "system", None, "email", "k3", "v3")
        .await
        .unwrap();

    let auth_settings = store
        .list_module_settings("system", None, Some("auth"))
        .await
        .unwrap();
    assert_eq!(auth_settings.len(), 2);

    let all_system = store
        .list_module_settings("system", None, None)
        .await
        .unwrap();
    assert_eq!(all_system.len(), 3);
}

#[tokio::test]
async fn test_effective_setting_collection_overrides_global() {
    let store = setup_db().await;
    store
        .set_module_setting("eff-1", "system", None, "mod", "key", "global_val")
        .await
        .unwrap();
    store
        .set_module_setting("eff-2", "document_collection", Some("col-1"), "mod", "key", "col_val")
        .await
        .unwrap();

    let val = store
        .get_effective_setting("mod", "key", Some("col-1"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(val, "col_val", "document_collection setting should override global");

    let val = store
        .get_effective_setting("mod", "key", None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(val, "global_val", "without collection, global is used");
}

#[tokio::test]
async fn test_delete_module_setting() {
    let store = setup_db().await;
    store
        .set_module_setting("del-1", "system", None, "mod", "key", "val")
        .await
        .unwrap();

    let deleted = store.delete_module_setting("del-1").await.unwrap();
    assert!(deleted);

    let deleted_again = store.delete_module_setting("del-1").await.unwrap();
    assert!(!deleted_again, "already deleted should return false");
}

// ==================== Password Reset ====================

#[tokio::test]
async fn test_password_reset_token_flow() {
    let store = setup_db().await;
    let user = make_user("reset@example.com", "resetuser");
    store.create_user(&user).await.unwrap();

    store
        .create_password_reset_token("prt-1", &user.id, "reset-token-abc", "2099-12-31T23:59:59+00:00")
        .await
        .unwrap();

    let (found_user_id, _expires, _used, found_id) = store
        .get_password_reset_token("reset-token-abc")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found_user_id, user.id.to_string());
    assert_eq!(found_id, "prt-1");

    store.mark_password_reset_used("prt-1").await.unwrap();
    let (_found_user_id, _expires, used, _found_id) = store
        .get_password_reset_token("reset-token-abc")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(used, 1, "token should be marked as used");
}

// ==================== Captcha ====================

#[tokio::test]
async fn test_captcha_create_get_mark_used() {
    let store = setup_db().await;
    store
        .create_captcha("cap-1", "42", "2099-12-31T23:59:59+00:00")
        .await
        .unwrap();

    let (answer, _used, _expires) = store.get_captcha("cap-1").await.unwrap().unwrap();
    assert_eq!(answer, "42");

    store.mark_captcha_used("cap-1").await.unwrap();
}

// ==================== SMS ====================

#[tokio::test]
async fn test_sms_code_create_and_get_latest() {
    let store = setup_db().await;
    store
        .create_sms_code("sms-1", "13800000001", "123456", "login", None, "2099-12-31T23:59:59+00:00")
        .await
        .unwrap();

    let (code, _id, _used, _expires) = store
        .get_latest_sms_code("13800000001", "login")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(code, "123456");
}

#[tokio::test]
async fn test_sms_code_mark_used() {
    let store = setup_db().await;
    store
        .create_sms_code("sms-2", "13800000002", "654321", "register", None, "2099-12-31T23:59:59+00:00")
        .await
        .unwrap();

    store.mark_sms_code_used("sms-2").await.unwrap();

    let (_code, _id, used, _expires) = store
        .get_latest_sms_code("13800000002", "register")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(used, 1);
}
