//! 搜索引擎 — 全文索引和搜索

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 搜索查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub models: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub filters: Option<serde_json::Value>,
}

/// 搜索高亮片段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHighlight {
    pub field: String,
    pub snippet: String,
}

/// 搜索结果条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub model: String,
    pub record_id: String,
    pub score: f64,
    pub data: serde_json::Value,
    pub highlights: Vec<SearchHighlight>,
}

/// 搜索引擎 — 全文索引、搜索和索引管理
#[async_trait]
pub trait SearchEngine: Send + Sync {
    /// 索引单条记录
    async fn index_record(&self, model: &str, record_id: &str, data: &serde_json::Value) -> Result<(), anyhow::Error>;

    /// 从索引中移除记录
    async fn remove_record(&self, model: &str, record_id: &str) -> Result<(), anyhow::Error>;

    /// 执行搜索查询
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>, anyhow::Error>;

    /// 重建指定模型的全文索引
    async fn rebuild_index(&self, model: &str) -> Result<(), anyhow::Error>;
}
