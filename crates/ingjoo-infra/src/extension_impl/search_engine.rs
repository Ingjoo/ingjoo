//! 数据库搜索引擎 — 基于 SQL LIKE 的全文索引搜索
//!
//! 维护 `ir_search_index` 表，存储被索引的文档内容。
//! 使用 SQL LIKE 做模糊匹配，适用于中小规模数据。

use async_trait::async_trait;
use ingjoo_core::extension::search::{
    SearchEngine, SearchHighlight, SearchQuery, SearchResult,
};
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;

/// 基于 SQL LIKE 的数据库搜索引擎
pub struct DbSearchEngine {
    pool: Pool,
    dialect: Dialect,
}

impl DbSearchEngine {
    pub fn new(pool: Pool, dialect: Dialect) -> Self {
        Self { pool, dialect }
    }

    /// 执行搜索查询，返回匹配结果
    async fn do_search(
        &self,
        query: &SearchQuery,
    ) -> Result<Vec<SearchResult>, anyhow::Error> {
        let limit = query.limit.unwrap_or(20) as i64;
        let offset = query.offset.unwrap_or(0) as i64;
        let pattern = format!("%{}%", query.text);

        if query.models.is_empty() {
            return Ok(vec![]);
        }

        // 构建带占位符的 IN 子句
        let model_count = query.models.len();
        let placeholders: Vec<String> = (0..model_count).map(|_| "?".to_string()).collect();
        let in_clause = placeholders.join(",");

        let sql = self.dialect.prepare(&format!(
            "SELECT model, record_id, content, data FROM ir_search_index WHERE model IN ({}) AND content LIKE ? ORDER BY model, record_id LIMIT ? OFFSET ?",
            in_clause
        ));

        let mut sql_query = sqlx::query_as::<_, (String, String, String, String)>(&sql);
        for model in &query.models {
            sql_query = sql_query.bind(model);
        }
        sql_query = sql_query.bind(&pattern).bind(limit).bind(offset);

        let rows = sql_query.fetch_all(&self.pool).await?;
        let text_lower = query.text.to_lowercase();

        let results: Vec<SearchResult> = rows
            .into_iter()
            .map(|(model, record_id, content, data)| {
                // 计算简单相关度：匹配关键词在内容中出现的次数
                let score = content.to_lowercase().matches(&text_lower).count() as f64;

                // 提取高亮片段：找到匹配位置，截取前后上下文
                let highlights = extract_highlights(&content, &query.text);

                SearchResult {
                    model,
                    record_id,
                    score,
                    data: serde_json::from_str(&data).unwrap_or(serde_json::Value::Null),
                    highlights,
                }
            })
            .collect();

        Ok(results)
    }
}

/// 从内容中提取高亮片段
fn extract_highlights(content: &str, keyword: &str) -> Vec<SearchHighlight> {
    let content_lower = content.to_lowercase();
    let keyword_lower = keyword.to_lowercase();
    let context_len = 40;

    let mut highlights = Vec::new();
    let mut start = 0;

    while let Some(pos) = content_lower[start..].find(&keyword_lower) {
        let abs_pos = start + pos;
        let snippet_start_raw = abs_pos.saturating_sub(context_len);
        let snippet_end_raw = (abs_pos + keyword.len() + context_len).min(content.len());

        // 对齐到 UTF-8 char boundary，避免 panic
        let snippet_start = floor_char_boundary(content, snippet_start_raw);
        let snippet_end = ceil_char_boundary(content, snippet_end_raw);

        let mut snippet = content[snippet_start..snippet_end].to_string();
        if snippet_start > 0 {
            snippet = format!("...{}", snippet);
        }
        if snippet_end < content.len() {
            snippet = format!("{}...", snippet);
        }

        highlights.push(SearchHighlight {
            field: "content".to_string(),
            snippet,
        });

        start = abs_pos + keyword.len();
        if start >= content.len() {
            break;
        }
        // 最多 3 个高亮片段
        if highlights.len() >= 3 {
            break;
        }
    }

    highlights
}

fn floor_char_boundary(s: &str, i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    let mut pos = i;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn ceil_char_boundary(s: &str, i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    let mut pos = i;
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}

#[async_trait]
impl SearchEngine for DbSearchEngine {
    async fn index_record(
        &self,
        model: &str,
        record_id: &str,
        data: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        // 提取所有字符串字段值作为索引内容
        let content = extract_text(data);

        let sql = self.dialect.prepare(
            "INSERT INTO ir_search_index (model, record_id, content, data) VALUES (?, ?, ?, ?)
             ON CONFLICT(model, record_id) DO UPDATE SET content = excluded.content, data = excluded.data",
        );

        sqlx::query(&sql)
            .bind(model)
            .bind(record_id)
            .bind(&content)
            .bind(serde_json::to_string(data)?)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn remove_record(&self, model: &str, record_id: &str) -> Result<(), anyhow::Error> {
        let sql = self.dialect.prepare(
            "DELETE FROM ir_search_index WHERE model = ? AND record_id = ?",
        );

        sqlx::query(&sql)
            .bind(model)
            .bind(record_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn search(
        &self,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, anyhow::Error> {
        self.do_search(&query).await
    }

    async fn rebuild_index(&self, model: &str) -> Result<(), anyhow::Error> {
        // 清除指定模型的索引，由调用方重新写入
        let sql = self.dialect.prepare(
            "DELETE FROM ir_search_index WHERE model = ?",
        );

        sqlx::query(&sql)
            .bind(model)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// 从 JSON value 中递归提取所有字符串值，拼接为可搜索的文本
fn extract_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => map
            .values()
            .map(extract_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(extract_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (DbSearchEngine, tempfile::TempPath) {
        let tmp = tempfile::Builder::new()
            .prefix("ingjoo_search_test_")
            .suffix(".db")
            .tempfile()
            .unwrap();
        let db_path = tmp.into_temp_path();
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());

        ingjoo_core::pool::install_drivers();
        let (pool, dialect) = ingjoo_core::pool::connect_pool(&db_url).await.unwrap();

        // 创建 ir_search_index 表
        let sql = "CREATE TABLE IF NOT EXISTS ir_search_index (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            model TEXT NOT NULL,
            record_id TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            data TEXT NOT NULL DEFAULT '{}',
            UNIQUE(model, record_id)
        )";
        sqlx::query(sql).execute(&pool).await.unwrap();

        (DbSearchEngine::new(pool, dialect), db_path)
    }

    #[tokio::test]
    async fn search_index_and_find() {
        let (engine, _path) = setup().await;

        engine
            .index_record("product", "p1", &serde_json::json!({"name": "苹果手机", "desc": "最新款智能手机"}))
            .await
            .unwrap();
        engine
            .index_record("product", "p2", &serde_json::json!({"name": "华为笔记本", "desc": "轻薄商务本"}))
            .await
            .unwrap();

        let results = engine
            .search(SearchQuery {
                text: "手机".into(),
                models: vec!["product".into()],
                limit: Some(10),
                offset: None,
                filters: None,
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].record_id, "p1");
        assert!(results[0].score > 0.0);
        assert!(!results[0].highlights.is_empty());
    }

    #[tokio::test]
    async fn search_no_match() {
        let (engine, _path) = setup().await;

        engine
            .index_record("product", "p1", &serde_json::json!({"name": "苹果"}))
            .await
            .unwrap();

        let results = engine
            .search(SearchQuery {
                text: "香蕉".into(),
                models: vec!["product".into()],
                limit: Some(10),
                offset: None,
                filters: None,
            })
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_remove_record() {
        let (engine, _path) = setup().await;

        engine
            .index_record("product", "p1", &serde_json::json!({"name": "苹果手机"}))
            .await
            .unwrap();

        engine.remove_record("product", "p1").await.unwrap();

        let results = engine
            .search(SearchQuery {
                text: "苹果".into(),
                models: vec!["product".into()],
                limit: Some(10),
                offset: None,
                filters: None,
            })
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_upsert_overwrites() {
        let (engine, _path) = setup().await;

        engine
            .index_record("product", "p1", &serde_json::json!({"name": "旧名称"}))
            .await
            .unwrap();
        engine
            .index_record("product", "p1", &serde_json::json!({"name": "新名称手机"}))
            .await
            .unwrap();

        let results = engine
            .search(SearchQuery {
                text: "手机".into(),
                models: vec!["product".into()],
                limit: Some(10),
                offset: None,
                filters: None,
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].record_id, "p1");
    }

    #[tokio::test]
    async fn search_multi_model_filter() {
        let (engine, _path) = setup().await;

        engine
            .index_record("product", "p1", &serde_json::json!({"name": "手机"}))
            .await
            .unwrap();
        engine
            .index_record("article", "a1", &serde_json::json!({"title": "手机评测"}))
            .await
            .unwrap();

        // 只搜索 product 模型
        let results = engine
            .search(SearchQuery {
                text: "手机".into(),
                models: vec!["product".into()],
                limit: Some(10),
                offset: None,
                filters: None,
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].model, "product");
    }

    #[tokio::test]
    async fn search_rebuild_index_clears() {
        let (engine, _path) = setup().await;

        engine
            .index_record("product", "p1", &serde_json::json!({"name": "手机"}))
            .await
            .unwrap();

        engine.rebuild_index("product").await.unwrap();

        let results = engine
            .search(SearchQuery {
                text: "手机".into(),
                models: vec!["product".into()],
                limit: Some(10),
                offset: None,
                filters: None,
            })
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_empty_models_returns_empty() {
        let (engine, _path) = setup().await;

        engine
            .index_record("product", "p1", &serde_json::json!({"name": "手机"}))
            .await
            .unwrap();

        let results = engine
            .search(SearchQuery {
                text: "手机".into(),
                models: vec![],
                limit: Some(10),
                offset: None,
                filters: None,
            })
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn extract_text_from_json() {
        let data = serde_json::json!({
            "name": "苹果手机",
            "desc": "最新款",
            "count": 42,
            "tags": ["电子", "手机"]
        });
        let text = super::extract_text(&data);
        assert!(text.contains("苹果手机"));
        assert!(text.contains("最新款"));
        assert!(text.contains("电子"));
        assert!(!text.contains("42"));
    }

    #[test]
    fn extract_highlights_finds_matches() {
        let highlights = super::extract_highlights(
            "这是一段包含手机关键词的文本内容，手机是重要产品",
            "手机",
        );
        assert!(!highlights.is_empty());
        assert!(highlights.len() <= 3);
        assert_eq!(highlights[0].field, "content");
    }
}
