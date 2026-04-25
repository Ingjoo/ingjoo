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

    #[tokio::test]
    async fn test_db_translation_store_constructs() {
        let pool = sqlx::AnyPool::connect_lazy("sqlite::memory:").unwrap();
        let _store = DbTranslationStore::new(pool.into(), Dialect::Sqlite);
    }
}
