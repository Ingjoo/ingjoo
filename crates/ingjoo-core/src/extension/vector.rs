use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorResult {
    pub id: String,
    pub score: f32,
    pub metadata: Option<serde_json::Value>,
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    /// 存储向量 — 将向量与元数据写入指定集合
    async fn store(
        &self,
        collection: &str,
        id: &str,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<(), anyhow::Error>;

    /// 向量检索 — 在指定集合中搜索最相似的 top_k 个结果
    async fn search(
        &self,
        collection: &str,
        query: &[f32],
        top_k: usize,
        filter: Option<serde_json::Value>,
    ) -> Result<Vec<VectorResult>, anyhow::Error>;

    /// 删除向量 — 从集合中移除指定 ID 的向量
    async fn delete(&self, collection: &str, id: &str) -> Result<(), anyhow::Error>;
}
