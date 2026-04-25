use ingjoo_core::module::registry::{FieldType, ModelDescriptor};
use ingjoo_macros::IngjooModel;

#[derive(IngjooModel)]
#[ingjoo(table = "articles")]
#[allow(dead_code)]
struct Article {
    #[ingjoo(type = "text", required)]
    title: String,
    #[ingjoo(type = "text")]
    body: String,
    #[ingjoo(type = "integer")]
    views: i64,
    #[ingjoo(type = "boolean")]
    published: bool,
}

#[test]
fn derive_basic_fields() {
    let desc = Article::descriptor();
    assert_eq!(desc.name, "Article");
    assert_eq!(desc.table_name, "articles");
    assert!(!desc.audit_fields);
    assert_eq!(desc.fields.len(), 4);

    let title = desc.find_field("title").unwrap();
    assert_eq!(title.field_type, FieldType::Text);
    assert!(title.required);
    assert!(!title.unique);

    let body = desc.find_field("body").unwrap();
    assert_eq!(body.field_type, FieldType::Text);
    assert!(!body.required);

    let views = desc.find_field("views").unwrap();
    assert_eq!(views.field_type, FieldType::Integer);

    let published = desc.find_field("published").unwrap();
    assert_eq!(published.field_type, FieldType::Boolean);
}

#[derive(IngjooModel)]
#[ingjoo(table = "products", audit)]
#[allow(dead_code)]
struct Product {
    #[ingjoo(type = "text", required, unique)]
    sku: String,
    #[ingjoo(type = "float")]
    price: f64,
    #[ingjoo(type = "many2one", related = "categories")]
    category_id: Option<String>,
}

#[test]
fn derive_audit_and_many2one() {
    let desc = Product::descriptor();
    assert_eq!(desc.name, "Product");
    assert_eq!(desc.table_name, "products");
    assert!(desc.audit_fields);

    let sku = desc.find_field("sku").unwrap();
    assert!(sku.required);
    // required+unique: required 优先
    assert!(!sku.unique);

    let price = desc.find_field("price").unwrap();
    assert_eq!(price.field_type, FieldType::Float);

    let cat = desc.find_field("category_id").unwrap();
    assert_eq!(cat.field_type, FieldType::Many2one);
    assert!(cat.relation.is_some());
    assert_eq!(cat.relation.as_ref().unwrap().related_model, "categories");
}

#[derive(IngjooModel)]
#[ingjoo(table = "logs")]
#[allow(dead_code)]
struct LogEntry {
    #[ingjoo(type = "json")]
    payload: serde_json::Value,
    #[ingjoo(type = "timestamp")]
    created_at: String,
}

#[test]
fn derive_json_and_timestamp() {
    let desc = LogEntry::descriptor();
    let payload = desc.find_field("payload").unwrap();
    assert_eq!(payload.field_type, FieldType::Json);

    let ts = desc.find_field("created_at").unwrap();
    assert_eq!(ts.field_type, FieldType::Timestamp);
}

#[test]
fn descriptor_matches_manual_builder() {
    let derived = Article::descriptor();
    let manual = ModelDescriptor::new("Article", "articles")
        .required_field("title", FieldType::Text)
        .field("body", FieldType::Text)
        .field("views", FieldType::Integer)
        .field("published", FieldType::Boolean);

    assert_eq!(derived.name, manual.name);
    assert_eq!(derived.table_name, manual.table_name);
    assert_eq!(derived.fields.len(), manual.fields.len());
    for (d, m) in derived.fields.iter().zip(manual.fields.iter()) {
        assert_eq!(d.name, m.name);
        assert_eq!(d.field_type, m.field_type);
        assert_eq!(d.required, m.required);
        assert_eq!(d.unique, m.unique);
    }
}
