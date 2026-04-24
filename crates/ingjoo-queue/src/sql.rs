use async_trait::async_trait;
use chrono::Utc;
use ingjoo_core::db::error::StoreResult;
use ingjoo_core::{Dialect, pool::Pool};
use serde_json::Value;
use sqlx::Row;

use crate::{JobStatus, QueuedJob, Queue};

/// 基于 SQL 数据库的持久化队列实现
pub struct SqlQueue<'a> {
    pool: &'a Pool,
    dialect: &'a Dialect,
}

impl<'a> SqlQueue<'a> {
    pub fn new(pool: &'a Pool, dialect: &'a Dialect) -> Self {
        Self { pool, dialect }
    }

    fn sql(&self, s: &str) -> String {
        self.dialect.prepare(s)
    }

    fn row_to_job(&self, row: &sqlx::any::AnyRow) -> QueuedJob {
        let status_str: String = row.try_get("status").unwrap_or_default();
        let status = JobStatus::try_from_str(&status_str).unwrap_or(JobStatus::Pending);

        let run_at: Option<String> = row.try_get("run_at").ok().flatten();
        let started_at: Option<String> = row.try_get("started_at").ok().flatten();
        let completed_at: Option<String> = row.try_get("completed_at").ok().flatten();
        let error: Option<String> = row.try_get("error").ok().flatten();
        let created_at: String = row.try_get("created_at").unwrap_or_default();

        let payload_str: String = row.try_get("payload").unwrap_or_default();
        let payload: Value = serde_json::from_str(&payload_str).unwrap_or(Value::Null);

        QueuedJob {
            id: row.try_get("id").unwrap_or_default(),
            queue: row.try_get("queue").unwrap_or_else(|_| "default".to_string()),
            name: row.try_get("name").unwrap_or_default(),
            payload,
            priority: row.try_get("priority").unwrap_or(0),
            status,
            attempts: row.try_get("attempts").unwrap_or(0),
            max_attempts: row.try_get("max_attempts").unwrap_or(3),
            run_at: run_at.and_then(|s| s.parse().ok()),
            started_at: started_at.and_then(|s| s.parse().ok()),
            completed_at: completed_at.and_then(|s| s.parse().ok()),
            error,
            created_at: created_at.parse().unwrap_or_default(),
        }
    }
}

#[async_trait]
impl<'a> Queue for SqlQueue<'a> {
    async fn enqueue(&self, job: QueuedJob) -> StoreResult<String> {
        let id = job.id.clone();
        let payload = serde_json::to_string(&job.payload).unwrap_or_default();
        let run_at = job.run_at.map(|t| t.to_rfc3339());
        let status = job.status.as_str();

        let sql = self.sql(&format!(
            "INSERT INTO queue_jobs (id, queue, name, payload, priority, status, attempts, max_attempts, run_at, error, created_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
        ));

        sqlx::query(&sql)
            .bind(&job.id)
            .bind(&job.queue)
            .bind(&job.name)
            .bind(&payload)
            .bind(job.priority)
            .bind(status)
            .bind(job.attempts)
            .bind(job.max_attempts)
            .bind(&run_at)
            .bind(&job.error)
            .bind(job.created_at.to_rfc3339())
            .execute(self.pool)
            .await?;

        Ok(id)
    }

    async fn dequeue(&self, queue: &str) -> StoreResult<Option<QueuedJob>> {
        let now = Utc::now().to_rfc3339();
        let running = JobStatus::Running.as_str();

        let select_sql = self.sql(&format!(
            "SELECT * FROM queue_jobs WHERE queue = {} AND status = {} AND (run_at IS NULL OR run_at <= {}) ORDER BY priority ASC, created_at ASC LIMIT 1",
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
        ));

        let update_sql = self.sql(&format!(
            "UPDATE queue_jobs SET status = {}, started_at = {}, attempts = attempts + 1 WHERE id = {} AND status = {}",
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
        ));

        let row = sqlx::query(&select_sql)
            .bind(queue)
            .bind(JobStatus::Pending.as_str())
            .bind(&now)
            .fetch_optional(self.pool)
            .await?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let job_id: String = row.try_get("id").unwrap_or_default();

        let result = sqlx::query(&update_sql)
            .bind(running)
            .bind(&now)
            .bind(&job_id)
            .bind(JobStatus::Pending.as_str())
            .execute(self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        let mut job = self.row_to_job(&row);
        job.status = JobStatus::Running;
        job.attempts += 1;
        job.started_at = Some(Utc::now());
        Ok(Some(job))
    }

    async fn ack(&self, job_id: &str) -> StoreResult<()> {
        let now = Utc::now().to_rfc3339();
        let sql = self.sql(&format!(
            "UPDATE queue_jobs SET status = {}, completed_at = {} WHERE id = {}",
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
        ));
        sqlx::query(&sql)
            .bind(JobStatus::Completed.as_str())
            .bind(&now)
            .bind(job_id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    async fn nack(&self, job_id: &str, reason: &str) -> StoreResult<()> {
        let sql = self.sql(&format!(
            "UPDATE queue_jobs SET status = {}, error = {} WHERE id = {}",
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
        ));
        sqlx::query(&sql)
            .bind(JobStatus::Failed.as_str())
            .bind(reason)
            .bind(job_id)
            .execute(self.pool)
            .await?;

        let check_sql = self.sql(&format!(
            "SELECT attempts, max_attempts FROM queue_jobs WHERE id = {}",
            self.dialect.placeholder(1),
        ));
        let row = sqlx::query(&check_sql)
            .bind(job_id)
            .fetch_optional(self.pool)
            .await?;

        if let Some(row) = row {
            let attempts: i32 = row.try_get("attempts").unwrap_or(0);
            let max_attempts: i32 = row.try_get("max_attempts").unwrap_or(3);
            if attempts >= max_attempts {
                self.dead_letter(job_id).await?;
            }
        }

        Ok(())
    }

    async fn retry(&self, job_id: &str) -> StoreResult<()> {
        let sql = self.sql(&format!(
            "UPDATE queue_jobs SET status = {}, error = NULL, started_at = NULL WHERE id = {}",
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
        ));
        sqlx::query(&sql)
            .bind(JobStatus::Pending.as_str())
            .bind(job_id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    async fn dead_letter(&self, job_id: &str) -> StoreResult<()> {
        let now = Utc::now().to_rfc3339();
        let sql = self.sql(&format!(
            "UPDATE queue_jobs SET status = {}, completed_at = {} WHERE id = {}",
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
        ));
        sqlx::query(&sql)
            .bind(JobStatus::DeadLetter.as_str())
            .bind(&now)
            .bind(job_id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    async fn pending_count(&self, queue: &str) -> StoreResult<i64> {
        let sql = self.sql(&format!(
            "SELECT COUNT(*) FROM queue_jobs WHERE queue = {} AND status = {}",
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
        ));
        let count = sqlx::query_scalar::<_, i64>(&sql)
            .bind(queue)
            .bind(JobStatus::Pending.as_str())
            .fetch_one(self.pool)
            .await?;
        Ok(count)
    }
}
