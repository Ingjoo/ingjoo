//! 数据库审计日志存储 — 基于 audit_logs 表

use async_trait::async_trait;
use ingjoo_core::extension::audit::{AuditEntry, AuditQuery, AuditStore};
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use sqlx::Row as _;

/// 基于 SQL 数据库的审计日志存储
///
/// 依赖迁移 v7 创建的 `audit_logs` 表。
/// 数据库审计日志存储
pub struct DbAuditStore {
    pool: Pool,
    dialect: Dialect,
}

impl DbAuditStore {
    pub fn new(pool: Pool, dialect: Dialect) -> Self {
        Self { pool, dialect }
    }

    fn sql(&self, query: &str) -> String {
        self.dialect.prepare(query)
    }

    fn row_to_entry(row: &sqlx::any::AnyRow) -> AuditEntry {
        AuditEntry {
            id: row.get("id"),
            user_id: row.get("user_id"),
            action: row.get("action"),
            resource: row.get("resource"),
            resource_id: row.get("resource_id"),
            detail: row.get::<Option<String>, _>("detail").and_then(|s| serde_json::from_str(&s).ok()),
            ip: row.get("ip"),
            created_at: row.get("created_at"),
        }
    }
}

#[async_trait]
impl AuditStore for DbAuditStore {
    async fn create_audit_log(
        &self,
        user_id: Option<&str>,
        action: &str,
        resource: &str,
        resource_id: Option<&str>,
        detail: Option<serde_json::Value>,
        ip: Option<&str>,
    ) -> Result<AuditEntry, anyhow::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let detail_str = detail.as_ref().map(serde_json::to_string).transpose()?.unwrap_or_default();

        sqlx::query(&self.sql(
            "INSERT INTO audit_logs (id, user_id, action, resource, resource_id, detail, ip) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        ))
        .bind(&id)
        .bind(user_id)
        .bind(action)
        .bind(resource)
        .bind(resource_id)
        .bind(&detail_str)
        .bind(ip)
        .execute(&self.pool)
        .await?;

        let created_at: String = sqlx::query_scalar("SELECT created_at FROM audit_logs WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.pool)
            .await
            .unwrap_or_default();

        Ok(AuditEntry {
            id,
            user_id: user_id.map(String::from),
            action: action.to_string(),
            resource: resource.to_string(),
            resource_id: resource_id.map(String::from),
            detail,
            ip: ip.map(String::from),
            created_at,
        })
    }

    async fn list_audit_logs(&self, query: AuditQuery) -> Result<Vec<AuditEntry>, anyhow::Error> {
        let mut conditions = Vec::new();
        let mut sql = "SELECT * FROM audit_logs".to_string();

        if query.user_id.is_some() {
            conditions.push("user_id = ?");
        }
        if query.action.is_some() {
            conditions.push("action = ?");
        }
        if query.resource.is_some() {
            conditions.push("resource = ?");
        }
        if query.resource_id.is_some() {
            conditions.push("resource_id = ?");
        }
        if query.ip.is_some() {
            conditions.push("ip = ?");
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY created_at DESC");

        let limit = query.limit.unwrap_or(100);
        let offset = query.offset.unwrap_or(0);
        sql.push_str(" LIMIT ? OFFSET ?");

        let prepared = self.sql(&sql);
        let mut q = sqlx::query(&prepared);
        if let Some(ref v) = query.user_id {
            q = q.bind(v);
        }
        if let Some(ref v) = query.action {
            q = q.bind(v);
        }
        if let Some(ref v) = query.resource {
            q = q.bind(v);
        }
        if let Some(ref v) = query.resource_id {
            q = q.bind(v);
        }
        if let Some(ref v) = query.ip {
            q = q.bind(v);
        }
        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows.iter().map(Self::row_to_entry).collect())
    }

    async fn get_audit_log(&self, id: &str) -> Result<Option<AuditEntry>, anyhow::Error> {
        let row =
            sqlx::query(&self.sql("SELECT * FROM audit_logs WHERE id = ?")).bind(id).fetch_optional(&self.pool).await?;
        Ok(row.as_ref().map(Self::row_to_entry))
    }

    async fn delete_logs_before(&self, before: &str) -> Result<u64, anyhow::Error> {
        let result = sqlx::query(&self.sql("DELETE FROM audit_logs WHERE created_at < ?"))
            .bind(before)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_db_audit_store_constructs() {
        let pool = sqlx::AnyPool::connect_lazy("sqlite::memory:").unwrap();
        let _store = DbAuditStore::new(pool, Dialect::Sqlite);
    }
}
