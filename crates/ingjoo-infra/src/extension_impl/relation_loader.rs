//! 关系加载器 — 基于 SQL 的批量关联数据加载

use async_trait::async_trait;
use ingjoo_core::extension::RelationLoader;
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use sqlx::Row as _;
use std::collections::HashMap;

/// 基于 SQL 数据库的关系加载器
pub struct DbRelationLoader {
    pool: Pool,
    dialect: Dialect,
}

impl DbRelationLoader {
    pub fn new(pool: Pool, dialect: Dialect) -> Self {
        Self { pool, dialect }
    }

    fn sql(&self, query: &str) -> String {
        self.dialect.prepare(query)
    }
}

#[async_trait]
impl RelationLoader for DbRelationLoader {
    async fn load_one2many(
        &self,
        model: &str,
        field: &str,
        ids: &[String],
    ) -> Result<HashMap<String, Vec<serde_json::Value>>, anyhow::Error> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
        let sql =
            self.sql(&format!("SELECT id, {} FROM {} WHERE {} IN ({})", field, model, field, placeholders.join(",")));

        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }

        let rows = query.fetch_all(&self.pool).await?;

        let mut result: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
        for id in ids {
            result.entry(id.clone()).or_default();
        }

        for row in &rows {
            let fk_value: String = row.get(field);
            if let Some(entries) = result.get_mut(&fk_value) {
                let mut map = serde_json::Map::new();
                for col in row.columns().iter() {
                    let col_name: &str = &col.name;
                    if col_name == field {
                        continue;
                    }
                    if let Ok(v) = try_raw_to_value(row, col_name) {
                        map.insert(col_name.to_string(), v);
                    }
                }
                entries.push(serde_json::Value::Object(map));
            }
        }

        Ok(result)
    }

    async fn load_many2one(
        &self,
        model: &str,
        field: &str,
        ids: &[String],
    ) -> Result<HashMap<String, Option<serde_json::Value>>, anyhow::Error> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut result: HashMap<String, Option<serde_json::Value>> = HashMap::new();
        for id in ids {
            result.insert(id.clone(), None);
        }

        let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
        let sql = self.sql(&format!("SELECT id, {} FROM {} WHERE id IN ({})", field, model, placeholders.join(",")));

        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }

        let rows = query.fetch_all(&self.pool).await?;

        for row in &rows {
            let record_id: String = row.get("id");
            let fk_value: Option<String> = row.try_get(field).ok();
            result.insert(record_id, fk_value.map(serde_json::Value::String));
        }

        Ok(result)
    }
}

fn try_raw_to_value(row: &sqlx::any::AnyRow, col: &str) -> Result<serde_json::Value, ()> {
    if let Ok(v) = row.try_get::<String, _>(col) {
        Ok(serde_json::Value::String(v))
    } else if let Ok(v) = row.try_get::<i64, _>(col) {
        Ok(serde_json::json!(v))
    } else if let Ok(v) = row.try_get::<f64, _>(col) {
        Ok(serde_json::json!(v))
    } else if let Ok(v) = row.try_get::<bool, _>(col) {
        Ok(serde_json::json!(v))
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_relation_loader_constructs() {
        let pool = sqlx::AnyPool::connect_lazy("sqlite::memory:").unwrap();
        let _loader = DbRelationLoader::new(pool, Dialect::Sqlite);
    }
}
