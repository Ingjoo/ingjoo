use async_trait::async_trait;

#[async_trait]
pub trait RelationLoader: Send + Sync {
    async fn load_one2many(
        &self,
        model: &str,
        field: &str,
        ids: &[String],
    ) -> Result<std::collections::HashMap<String, Vec<serde_json::Value>>, anyhow::Error>;

    async fn load_many2one(
        &self,
        model: &str,
        field: &str,
        ids: &[String],
    ) -> Result<std::collections::HashMap<String, Option<serde_json::Value>>, anyhow::Error>;
}

#[derive(Debug, Clone)]
pub struct Many2Many {
    pub table: String,
    pub column_a: String,
    pub column_b: String,
    pub model_a: String,
    pub model_b: String,
}

impl Many2Many {
    pub fn new(table: &str, model_a: &str, model_b: &str) -> Self {
        Self {
            table: table.to_string(),
            column_a: format!("{}_id", model_a),
            column_b: format!("{}_id", model_b),
            model_a: model_a.to_string(),
            model_b: model_b.to_string(),
        }
    }
}
