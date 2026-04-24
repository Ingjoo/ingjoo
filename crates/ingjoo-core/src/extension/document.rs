use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub content: String,
    pub metadata: serde_json::Value,
}

#[async_trait]
pub trait DocumentLoader: Send + Sync {
    /// 加载文档 — 从指定来源加载文档列表
    async fn load(&self, source: &str) -> Result<Vec<Document>, anyhow::Error>;
}

/// 文档分割器（同步操作）
pub trait TextSplitter: Send + Sync {
    /// 分割文档 — 将文档拆分为更小的片段
    fn split(&self, documents: &[Document]) -> Vec<Document>;
}
