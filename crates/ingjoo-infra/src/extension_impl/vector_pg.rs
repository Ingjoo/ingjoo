//! PgVectorStore — 基于 PostgreSQL pgvector 扩展的向量存储实现
//!
//! 使用前需启用 PostgreSQL 扩展: `CREATE EXTENSION IF NOT EXISTS vector;`
//! 每个集合对应一张独立的表 `vec_<collection>`，包含 id / embedding / metadata 列。

use async_trait::async_trait;
use ingjoo_core::extension::vector::{VectorResult, VectorStore};
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use sqlx::Row as _;

/// 基于 PostgreSQL pgvector 扩展的向量存储
///
/// 相似度计算使用余弦距离 (`<=>` 操作符)，分数 = `1 - cosine_distance`。
/// PostgreSQL pgvector 向量存储实现
pub struct PgVectorStore {
    pool: Pool,
    dialect: Dialect,
    dimension: usize,
}

impl PgVectorStore {
    /// 创建 PgVectorStore 实例
    ///
    /// * `pool` — 数据库连接池
    /// * `dialect` — 数据库方言（必须为 PostgreSQL）
    /// * `dimension` — 向量维度（如 OpenAI text-embedding-ada-002 为 1536）
    pub fn new(pool: Pool, dialect: Dialect, dimension: usize) -> Self {
        Self {
            pool,
            dialect,
            dimension,
        }
    }

    /// 生成集合对应的表名（仅保留字母数字和下划线，防止 SQL 注入）
    fn table_name(collection: &str) -> String {
        let safe: String = collection
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_lowercase();
        if safe.is_empty() {
            return "vec_default".to_string();
        }
        format!("vec_{}", safe)
    }

    /// 将 f32 切片转换为 pgvector 字面量字符串 `[0.1,0.2,...]`
    fn vector_literal(v: &[f32]) -> String {
        let inner: Vec<String> = v.iter().map(|f| f.to_string()).collect();
        format!("[{}]", inner.join(","))
    }

    fn sql(&self, query: &str) -> String {
        self.dialect.prepare(query)
    }

    /// 确保集合表存在（自动建表）
    async fn ensure_table(&self, collection: &str) -> Result<(), anyhow::Error> {
        let table = Self::table_name(collection);
        let dim = self.dimension;
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (\
                id VARCHAR(128) PRIMARY KEY, \
                embedding vector({}), \
                metadata TEXT\
            )",
            table, dim
        );
        sqlx::query(&self.sql(&sql))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl VectorStore for PgVectorStore {
    async fn store(
        &self,
        collection: &str,
        id: &str,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<(), anyhow::Error> {
        self.ensure_table(collection).await?;
        let table = Self::table_name(collection);
        let vec_str = Self::vector_literal(vector);
        let meta_str = metadata
            .map(|v| serde_json::to_string(&v))
            .transpose()?
            .unwrap_or_default();

        let sql = format!(
            "INSERT INTO {} (id, embedding, metadata) VALUES (?, ?::vector, ?) \
             ON CONFLICT (id) DO UPDATE SET embedding = EXCLUDED.embedding, metadata = EXCLUDED.metadata",
            table
        );
        sqlx::query(&self.sql(&sql))
            .bind(id)
            .bind(&vec_str)
            .bind(&meta_str)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn search(
        &self,
        collection: &str,
        query: &[f32],
        top_k: usize,
        _filter: Option<serde_json::Value>,
    ) -> Result<Vec<VectorResult>, anyhow::Error> {
        self.ensure_table(collection).await?;
        let table = Self::table_name(collection);
        let vec_str = Self::vector_literal(query);

        let sql = format!(
            "SELECT id, 1 - (embedding <=> ?::vector) AS score, metadata \
             FROM {} \
             ORDER BY embedding <=> ?::vector \
             LIMIT ?",
            table
        );
        let rows = sqlx::query(&self.sql(&sql))
            .bind(&vec_str)
            .bind(&vec_str)
            .bind(top_k as i64)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in &rows {
            let id: String = row.try_get("id")?;
            let score: f64 = row.try_get("score")?;
            let meta_str: Option<String> = row.try_get("metadata").ok();
            let metadata = meta_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .filter(|v: &serde_json::Value| !v.is_null());
            results.push(VectorResult {
                id,
                score: score as f32,
                metadata,
            });
        }
        Ok(results)
    }

    async fn delete(&self, collection: &str, id: &str) -> Result<(), anyhow::Error> {
        let table = Self::table_name(collection);
        sqlx::query(&self.sql(&format!(
            "DELETE FROM {} WHERE id = ?",
            table
        )))
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_name_sanitization() {
        assert_eq!(PgVectorStore::table_name("products"), "vec_products");
        assert_eq!(PgVectorStore::table_name("my-collection"), "vec_mycollection");
        assert_eq!(
            PgVectorStore::table_name("DROP TABLE users;"),
            "vec_droptableusers"
        );
        assert_eq!(PgVectorStore::table_name(""), "vec_default");
        assert_eq!(PgVectorStore::table_name("CamelCase"), "vec_camelcase");
    }

    #[test]
    fn test_vector_literal() {
        let vec = &[0.1, 0.2, 0.3];
        let literal = PgVectorStore::vector_literal(vec);
        assert_eq!(literal, "[0.1,0.2,0.3]");
    }

    #[test]
    fn test_vector_literal_empty() {
        let literal = PgVectorStore::vector_literal(&[]);
        assert_eq!(literal, "[]");
    }
}
