use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub models: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub filters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHighlight {
    pub field: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub model: String,
    pub record_id: String,
    pub score: f64,
    pub data: serde_json::Value,
    pub highlights: Vec<SearchHighlight>,
}

#[async_trait]
pub trait SearchEngine: Send + Sync {
    async fn index_record(
        &self,
        model: &str,
        record_id: &str,
        data: &serde_json::Value,
    ) -> Result<(), anyhow::Error>;

    async fn remove_record(&self, model: &str, record_id: &str) -> Result<(), anyhow::Error>;

    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>, anyhow::Error>;

    async fn rebuild_index(&self, model: &str) -> Result<(), anyhow::Error>;
}
