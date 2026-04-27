//! 内存向量存储 — 开发和测试用，无需 pgvector 依赖

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use ingjoo_core::extension::vector::{VectorResult, VectorStore};

/// 内存向量条目
struct VectorEntry {
    vector: Vec<f32>,
    metadata: Option<serde_json::Value>,
}

/// 内存向量存储 — 基于 `Mutex<HashMap>` 实现，用于开发和测试
///
/// 数据结构：collection → id → (vector, metadata)
/// 搜索使用余弦相似度排序
/// 进程内余弦相似度向量存储
pub struct InMemoryVectorStore {
    /// collection_name → (id → VectorEntry)
    collections: Mutex<HashMap<String, HashMap<String, VectorEntry>>>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self { collections: Mutex::new(HashMap::new()) }
    }

    /// 计算余弦相似度
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn store(
        &self,
        collection: &str,
        id: &str,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<(), anyhow::Error> {
        let mut collections = self.collections.lock().unwrap();
        let col = collections.entry(collection.to_string()).or_default();
        col.insert(id.to_string(), VectorEntry { vector: vector.to_vec(), metadata });
        Ok(())
    }

    async fn search(
        &self,
        collection: &str,
        query: &[f32],
        top_k: usize,
        _filter: Option<serde_json::Value>,
    ) -> Result<Vec<VectorResult>, anyhow::Error> {
        let collections = self.collections.lock().unwrap();
        let Some(col) = collections.get(collection) else {
            return Ok(vec![]);
        };
        let mut scored: Vec<VectorResult> = col
            .iter()
            .map(|(id, entry)| VectorResult {
                id: id.clone(),
                score: Self::cosine_similarity(query, &entry.vector),
                metadata: entry.metadata.clone(),
            })
            .collect();
        // 按分数降序排列，取 top_k
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        Ok(scored)
    }

    async fn delete(&self, collection: &str, id: &str) -> Result<(), anyhow::Error> {
        let mut collections = self.collections.lock().unwrap();
        if let Some(col) = collections.get_mut(collection) {
            col.remove(id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_search() {
        let store = InMemoryVectorStore::new();

        // 存入两个向量
        store.store("test", "vec1", &[1.0, 0.0, 0.0], None).await.unwrap();
        store.store("test", "vec2", &[0.0, 1.0, 0.0], Some(serde_json::json!({"label": "y-axis"}))).await.unwrap();

        // 搜索与 x-axis 最相似的
        let results = store.search("test", &[1.0, 0.0, 0.0], 5, None).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "vec1");
        assert!(results[0].score > 0.99); // cosine sim ≈ 1.0
    }

    #[tokio::test]
    async fn test_delete() {
        let store = InMemoryVectorStore::new();
        store.store("test", "vec1", &[1.0, 0.0], None).await.unwrap();
        store.delete("test", "vec1").await.unwrap();

        let results = store.search("test", &[1.0, 0.0], 5, None).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_upsert() {
        let store = InMemoryVectorStore::new();
        store.store("test", "vec1", &[1.0, 0.0], None).await.unwrap();
        store.store("test", "vec1", &[0.0, 1.0], None).await.unwrap(); // 更新

        let results = store.search("test", &[0.0, 1.0], 5, None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0.99);
    }

    #[tokio::test]
    async fn test_empty_collection() {
        let store = InMemoryVectorStore::new();
        let results = store.search("nonexistent", &[1.0, 0.0], 5, None).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_cosine_similarity_zero_vectors() {
        let sim = InMemoryVectorStore::cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]);
        assert_eq!(sim, 0.0);
    }

    #[tokio::test]
    async fn test_collection_isolation() {
        let store = InMemoryVectorStore::new();
        store.store("col_a", "v1", &[1.0, 0.0], None).await.unwrap();
        store.store("col_b", "v2", &[0.0, 1.0], None).await.unwrap();

        let results_a = store.search("col_a", &[1.0, 0.0], 5, None).await.unwrap();
        let results_b = store.search("col_b", &[0.0, 1.0], 5, None).await.unwrap();

        assert_eq!(results_a.len(), 1);
        assert_eq!(results_b.len(), 1);
        assert_eq!(results_a[0].id, "v1");
        assert_eq!(results_b[0].id, "v2");
    }

    #[tokio::test]
    async fn test_search_top_k_limit() {
        let store = InMemoryVectorStore::new();
        for i in 0..10 {
            let v = vec![i as f32 / 10.0, 1.0];
            store.store("test", &format!("v{}", i), &v, None).await.unwrap();
        }

        let results = store.search("test", &[1.0, 1.0], 3, None).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_metadata_preserved() {
        let store = InMemoryVectorStore::new();
        let meta = serde_json::json!({"source": "test", "version": 2});
        store.store("test", "v1", &[1.0, 0.0], Some(meta.clone())).await.unwrap();

        let results = store.search("test", &[1.0, 0.0], 1, None).await.unwrap();
        assert_eq!(results[0].metadata, Some(meta));
    }

    #[tokio::test]
    async fn test_delete_idempotent() {
        let store = InMemoryVectorStore::new();
        // 删除不存在的记录也不报错
        store.delete("test", "nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn test_default_trait() {
        let store = InMemoryVectorStore::default();
        let results = store.search("test", &[1.0], 5, None).await.unwrap();
        assert!(results.is_empty());
    }
}
