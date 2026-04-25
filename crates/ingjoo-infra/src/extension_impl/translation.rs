//! 翻译存储 — 基于 ir_translation 表的字段级多语言翻译

use async_trait::async_trait;
use ingjoo_core::extension::translation::{Translation, TranslationStore};
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use sqlx::Row as _;
use std::collections::HashMap;

pub struct DbTranslationStore {
    pool: Pool,
    dialect: Dialect,
}

impl DbTranslationStore {
    pub fn new(pool: Pool, dialect: Dialect) -> Self {
        Self { pool, dialect }
    }

    fn sql(&self, query: &str) -> String {
        self.dialect.prepare(query)
    }
}

#[async_trait]
impl TranslationStore for DbTranslationStore {
    async fn get(
        &self,
        lang: &str,
        model: &str,
        field: &str,
        record_id: &str,
    ) -> Result<Option<String>, anyhow::Error> {
        let row = sqlx::query(&self.sql(
            "SELECT value FROM ir_translation WHERE lang = ? AND model = ? AND field = ? AND record_id = ?"
        ))
        .bind(lang)
        .bind(model)
        .bind(field)
        .bind(record_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.get("value")))
    }

    async fn set(
        &self,
        translation: Translation,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(&self.sql(
            "INSERT INTO ir_translation (lang, model, field, record_id, value) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(lang, model, field, record_id) DO UPDATE SET value = excluded.value"
        ))
        .bind(&translation.lang)
        .bind(&translation.model)
        .bind(&translation.field)
        .bind(&translation.record_id)
        .bind(&translation.value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_batch(
        &self,
        lang: &str,
        model: &str,
        field: &str,
        record_ids: &[String],
    ) -> Result<HashMap<String, String>, anyhow::Error> {
        if record_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<&str> = record_ids.iter().map(|_| "?").collect();
        let sql = self.sql(&format!(
            "SELECT record_id, value FROM ir_translation \
             WHERE lang = ? AND model = ? AND field = ? AND record_id IN ({})",
            placeholders.join(",")
        ));

        let mut query = sqlx::query(&sql)
            .bind(lang)
            .bind(model)
            .bind(field);
        for id in record_ids {
            query = query.bind(id);
        }

        let rows = query.fetch_all(&self.pool).await?;
        let mut result = HashMap::new();
        for row in &rows {
            let record_id: String = row.get("record_id");
            let value: String = row.get("value");
            result.insert(record_id, value);
        }
        Ok(result)
    }

    async fn remove(
        &self,
        lang: &str,
        model: &str,
        field: &str,
        record_id: &str,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(&self.sql(
            "DELETE FROM ir_translation WHERE lang = ? AND model = ? AND field = ? AND record_id = ?"
        ))
        .bind(lang)
        .bind(model)
        .bind(field)
        .bind(record_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_languages(&self, model: &str) -> Result<Vec<String>, anyhow::Error> {
        let rows = sqlx::query(&self.sql(
            "SELECT DISTINCT lang FROM ir_translation WHERE model = ? ORDER BY lang"
        ))
        .bind(model)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| r.get("lang")).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> DbTranslationStore {
        let tmp = tempfile::Builder::new()
            .prefix("translation_test_")
            .suffix(".db")
            .tempfile()
            .unwrap();
        let db_path = tmp.path().to_str().unwrap().to_string();
        std::mem::forget(tmp);

        ingjoo_core::pool::install_drivers();
        let db_url = format!("sqlite://{}?mode=rwc", db_path);
        let (pool, dialect) = ingjoo_core::pool::connect_pool(&db_url).await.unwrap();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS ir_translation (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lang TEXT NOT NULL,
                model TEXT NOT NULL,
                field TEXT NOT NULL,
                record_id TEXT NOT NULL,
                value TEXT NOT NULL,
                UNIQUE(lang, model, field, record_id)
            )"
        )
        .execute(&pool)
        .await
        .unwrap();
        DbTranslationStore::new(pool, dialect)
    }

    #[tokio::test]
    async fn test_db_translation_store_constructs() {
        let pool = sqlx::AnyPool::connect_lazy("sqlite::memory:").unwrap();
        let _store = DbTranslationStore::new(pool, Dialect::Sqlite);
    }

    #[tokio::test]
    async fn translation_set_and_get() {
        let store = setup().await;
        let t = Translation {
            lang: "zh".into(),
            model: "product".into(),
            field: "name".into(),
            record_id: "p1".into(),
            value: "产品A".into(),
        };
        store.set(t).await.unwrap();

        let val = store.get("zh", "product", "name", "p1").await.unwrap();
        assert_eq!(val, Some("产品A".into()));

        let missing = store.get("en", "product", "name", "p1").await.unwrap();
        assert_eq!(missing, None);
    }

    #[tokio::test]
    async fn translation_set_upserts() {
        let store = setup().await;
        store.set(Translation {
            lang: "zh".into(), model: "product".into(), field: "name".into(),
            record_id: "p1".into(), value: "旧名称".into(),
        }).await.unwrap();
        store.set(Translation {
            lang: "zh".into(), model: "product".into(), field: "name".into(),
            record_id: "p1".into(), value: "新名称".into(),
        }).await.unwrap();

        let val = store.get("zh", "product", "name", "p1").await.unwrap();
        assert_eq!(val, Some("新名称".into()));
    }

    #[tokio::test]
    async fn translation_get_batch() {
        let store = setup().await;
        for (id, name) in [("p1", "产品A"), ("p2", "产品B"), ("p3", "产品C")] {
            store.set(Translation {
                lang: "zh".into(), model: "product".into(), field: "name".into(),
                record_id: id.into(), value: name.into(),
            }).await.unwrap();
        }

        let batch = store.get_batch("zh", "product", "name", &["p1".into(), "p3".into()]).await.unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch.get("p1").unwrap(), "产品A");
        assert_eq!(batch.get("p3").unwrap(), "产品C");
    }

    #[tokio::test]
    async fn translation_remove() {
        let store = setup().await;
        store.set(Translation {
            lang: "zh".into(), model: "product".into(), field: "name".into(),
            record_id: "p1".into(), value: "产品A".into(),
        }).await.unwrap();

        store.remove("zh", "product", "name", "p1").await.unwrap();
        let val = store.get("zh", "product", "name", "p1").await.unwrap();
        assert_eq!(val, None);
    }

    #[tokio::test]
    async fn translation_list_languages() {
        let store = setup().await;
        for lang in ["zh", "en", "ja"] {
            store.set(Translation {
                lang: lang.into(), model: "product".into(), field: "name".into(),
                record_id: "p1".into(), value: format!("name_{}", lang),
            }).await.unwrap();
        }
        store.set(Translation {
            lang: "zh".into(), model: "order".into(), field: "status".into(),
            record_id: "o1".into(), value: "已发货".into(),
        }).await.unwrap();

        let langs = store.list_languages("product").await.unwrap();
        assert_eq!(langs, vec!["en", "ja", "zh"]);

        let order_langs = store.list_languages("order").await.unwrap();
        assert_eq!(order_langs, vec!["zh"]);
    }

    #[tokio::test]
    async fn translation_get_batch_empty_ids() {
        let store = setup().await;
        let batch = store.get_batch("zh", "product", "name", &[]).await.unwrap();
        assert!(batch.is_empty());
    }
}
