use ingjoo_core::module::{FieldType, IdType, ModelDescriptor};
use ingjoo_core::pool::Pool;
use ingjoo_core::query::domain::Domain;
use ingjoo_core::Dialect;
use ingjoo_infra::db::generic::{GenericDb, GenericRecordStore};
use serde_json::json;

async fn setup_pool() -> Pool {
    let tmp = tempfile::Builder::new().prefix("generic_test_").suffix(".db").tempfile().unwrap();
    let db_path = tmp.path().to_str().unwrap().to_string();
    std::mem::forget(tmp);

    let db_url = format!("sqlite://{}?mode=rwc", db_path);
    ingjoo_core::pool::install_drivers();
    let (pool, _) = ingjoo_core::pool::connect_pool(&db_url).await.unwrap();
    pool
}

fn make_test_model() -> ModelDescriptor {
    ModelDescriptor::new("article", "test_articles")
        .required_field("title", FieldType::Text)
        .field("body", FieldType::Text)
        .field("views", FieldType::Integer)
        .field("published", FieldType::Boolean)
}

fn make_test_model_integer_id() -> ModelDescriptor {
    ModelDescriptor::new("log_entry", "test_log_entries")
        .with_id_type(IdType::Integer)
        .required_field("message", FieldType::Text)
        .field("level", FieldType::Text)
}

fn make_test_model_with_audit() -> ModelDescriptor {
    ModelDescriptor::new("order", "test_orders")
        .required_field("title", FieldType::Text)
        .field("amount", FieldType::Float)
        .with_audit_fields()
}

#[tokio::test]
async fn test_ensure_table_text_id() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model();

    db.ensure_table(&model).await.unwrap();

    let row: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name='test_articles'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(row.is_some());
}

#[tokio::test]
async fn test_ensure_table_integer_id() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model_integer_id();

    db.ensure_table(&model).await.unwrap();

    let row: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name='test_log_entries'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(row.is_some());
}

#[tokio::test]
async fn test_create_and_read() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model();
    db.ensure_table(&model).await.unwrap();

    let data = json!({
        "title": "测试文章",
        "body": "正文内容",
        "views": 42,
        "published": true
    });

    let created = db.generic_create(&model, &data, None).await.unwrap();

    assert!(created.get("id").is_some());
    assert_eq!(created["title"], "测试文章");
    assert_eq!(created["views"], 42);
    assert_eq!(created["published"], true);

    let id = created["id"].as_str().unwrap();
    let read = db.generic_read(&model, id).await.unwrap().unwrap();
    assert_eq!(read["title"], "测试文章");
}

#[tokio::test]
async fn test_create_required_field_missing() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model();
    db.ensure_table(&model).await.unwrap();

    let data = json!({
        "body": "没有标题"
    });

    let result = db.generic_create(&model, &data, None).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("缺少必填字段"));
    assert!(err_msg.contains("title"));
}

#[tokio::test]
async fn test_update_record() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model();
    db.ensure_table(&model).await.unwrap();

    let data = json!({"title": "原标题", "body": "原内容"});
    let created = db.generic_create(&model, &data, None).await.unwrap();
    let id = created["id"].as_str().unwrap();

    let update_data = json!({"title": "新标题", "views": 10});
    let updated = db.generic_update(&model, id, &update_data, None).await.unwrap().unwrap();

    assert_eq!(updated["title"], "新标题");
    assert_eq!(updated["views"], 10);
    assert_eq!(updated["body"], "原内容");
}

#[tokio::test]
async fn test_delete_record() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model();
    db.ensure_table(&model).await.unwrap();

    let data = json!({"title": "要删除的"});
    let created = db.generic_create(&model, &data, None).await.unwrap();
    let id = created["id"].as_str().unwrap();

    let deleted = db.generic_delete(&model, id).await.unwrap();
    assert!(deleted);

    let read = db.generic_read(&model, id).await.unwrap();
    assert!(read.is_none());
}

#[tokio::test]
async fn test_list_with_pagination() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model();
    db.ensure_table(&model).await.unwrap();

    for i in 0..5 {
        let data = json!({"title": format!("文章{}", i)});
        db.generic_create(&model, &data, None).await.unwrap();
    }

    let page1 = db.generic_list(&model, None, 2, 0).await.unwrap();
    assert_eq!(page1.items.len(), 2);
    assert_eq!(page1.total, 5);

    let page2 = db.generic_list(&model, None, 2, 2).await.unwrap();
    assert_eq!(page2.items.len(), 2);

    let page3 = db.generic_list(&model, None, 2, 4).await.unwrap();
    assert_eq!(page3.items.len(), 1);
}

#[tokio::test]
async fn test_list_with_domain_filter() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = ModelDescriptor::new("item", "test_filter_items")
        .required_field("title", FieldType::Text)
        .field("status", FieldType::Text);
    db.ensure_table(&model).await.unwrap();

    db.generic_create(&model, &json!({"title": "A", "status": "published"}), None).await.unwrap();
    db.generic_create(&model, &json!({"title": "B", "status": "draft"}), None).await.unwrap();
    db.generic_create(&model, &json!({"title": "C", "status": "published"}), None).await.unwrap();

    let domain = Domain::from_json(r#"["status", "=", "published"]"#).unwrap();
    let result = db.generic_list(&model, Some(&domain), 50, 0).await.unwrap();
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);
}

#[tokio::test]
async fn test_count() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model();
    db.ensure_table(&model).await.unwrap();

    for i in 0..3 {
        db.generic_create(&model, &json!({"title": format!("t{}", i)}), None).await.unwrap();
    }

    let count = db.generic_count(&model, None).await.unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_count_with_domain() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = ModelDescriptor::new("product", "test_count_products")
        .required_field("name", FieldType::Text)
        .field("active", FieldType::Boolean);
    db.ensure_table(&model).await.unwrap();

    db.generic_create(&model, &json!({"name": "P1", "active": true}), None).await.unwrap();
    db.generic_create(&model, &json!({"name": "P2", "active": false}), None).await.unwrap();
    db.generic_create(&model, &json!({"name": "P3", "active": true}), None).await.unwrap();

    let domain = Domain::from_json(r#"["active", "=", true]"#).unwrap();
    let count = db.generic_count(&model, Some(&domain)).await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_audit_fields_auto_filled() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model_with_audit();
    db.ensure_table(&model).await.unwrap();

    let data = json!({"title": "订单1", "amount": 99.5});
    let created = db.generic_create(&model, &data, Some("user_abc")).await.unwrap();

    assert_eq!(created["create_uid"], "user_abc");
    assert_eq!(created["write_uid"], "user_abc");
    assert!(created["create_date"].is_string());
    assert!(created["write_date"].is_string());

    let id = created["id"].as_str().unwrap();
    let update_data = json!({"amount": 199.0});
    let updated = db.generic_update(&model, id, &update_data, Some("user_xyz")).await.unwrap().unwrap();

    assert_eq!(updated["create_uid"], "user_abc");
    assert_eq!(updated["write_uid"], "user_xyz");
}

#[tokio::test]
async fn test_delete_nonexistent() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model();
    db.ensure_table(&model).await.unwrap();

    let deleted = db.generic_delete(&model, "nonexistent-id").await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn test_update_noop_returns_current() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let db = GenericDb::new(&pool, &dialect);
    let model = make_test_model();
    db.ensure_table(&model).await.unwrap();

    let data = json!({"title": "原始"});
    let created = db.generic_create(&model, &data, None).await.unwrap();
    let id = created["id"].as_str().unwrap();

    let update_data = json!({});
    let updated = db.generic_update(&model, id, &update_data, None).await.unwrap().unwrap();
    assert_eq!(updated["title"], "原始");
}
