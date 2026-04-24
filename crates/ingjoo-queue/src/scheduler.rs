//! Cron 定时任务调度器
//!
//! 基于 `cron` crate 解析 cron 表达式，自适应睡眠调度循环。
//! 支持 SQLite 和 PostgreSQL，通过 Dialect 抽象差异。

use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cron::Schedule;
use ingjoo_core::db::error::StoreResult;
use ingjoo_core::{Dialect, pool::Pool};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;

use crate::{QueuedJob, Queue};

/// 定时任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleStatus {
    Active,
    Paused,
    Completed,
}

impl ScheduleStatus {
    /// 返回状态的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
        }
    }

    /// 从字符串解析状态
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

/// 定时任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJob {
    pub id: String,
    pub name: String,
    pub cron_expr: String,
    pub queue: String,
    pub job_name: String,
    pub payload: Value,
    pub status: ScheduleStatus,
    pub max_attempts: i32,
    pub last_fire_time: Option<DateTime<Utc>>,
    pub next_fire_time: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ScheduledJob {
    /// 创建新的定时任务（自动生成 UUID）
    pub fn new(name: &str, cron_expr: &str, queue: &str, job_name: &str, payload: Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            cron_expr: cron_expr.to_string(),
            queue: queue.to_string(),
            job_name: job_name.to_string(),
            payload,
            status: ScheduleStatus::Active,
            max_attempts: 3,
            last_fire_time: None,
            next_fire_time: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// 设置最大重试次数
    pub fn with_max_attempts(mut self, max: i32) -> Self {
        self.max_attempts = max;
        self
    }

    /// 替换任务载荷
    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    /// 解析 cron 表达式，计算下次触发时间
    pub fn compute_next_fire_time(&self) -> StoreResult<DateTime<Utc>> {
        let schedule = Schedule::from_str(&self.cron_expr)
            .map_err(|e| ingjoo_core::db::error::StoreError::Config(
                format!("无效 cron 表达式 '{}': {}", self.cron_expr, e)
            ))?;
        schedule
            .upcoming(Utc)
            .next()
            .ok_or_else(|| ingjoo_core::db::error::StoreError::Config(
                format!("cron 表达式 '{}' 无未来触发时间", self.cron_expr)
            ))
    }
}

/// 调度器存储 trait
#[async_trait]
pub trait ScheduleStore: Send + Sync {
    async fn create(&self, job: &ScheduledJob) -> StoreResult<String>;
    async fn get(&self, id: &str) -> StoreResult<Option<ScheduledJob>>;
    async fn list(&self, status: Option<ScheduleStatus>) -> StoreResult<Vec<ScheduledJob>>;
    async fn update(&self, job: &ScheduledJob) -> StoreResult<()>;
    async fn delete(&self, id: &str) -> StoreResult<()>;
    /// 获取所有到期的活跃任务
    async fn fetch_due(&self) -> StoreResult<Vec<ScheduledJob>>;
}

/// SQL 实现的调度存储
pub struct SqlScheduleStore<'a> {
    pool: &'a Pool,
    dialect: &'a Dialect,
}

impl<'a> SqlScheduleStore<'a> {
    /// 创建 SQL 调度存储实例
    pub fn new(pool: &'a Pool, dialect: &'a Dialect) -> Self {
        Self { pool, dialect }
    }

    fn sql(&self, s: &str) -> String {
        self.dialect.prepare(s)
    }

    fn row_to_job(&self, row: &sqlx::any::AnyRow) -> ScheduledJob {
        let status_str: String = row.try_get("status").unwrap_or_default();
        let status = ScheduleStatus::try_from_str(&status_str).unwrap_or(ScheduleStatus::Paused);

        let payload_str: String = row.try_get("payload").unwrap_or_default();
        let payload: Value = serde_json::from_str(&payload_str).unwrap_or(Value::Null);

        let last_fire: Option<String> = row.try_get("last_fire_time").ok().flatten();
        let next_fire: Option<String> = row.try_get("next_fire_time").ok().flatten();
        let created_at: String = row.try_get("created_at").unwrap_or_default();
        let updated_at: String = row.try_get("updated_at").unwrap_or_default();

        ScheduledJob {
            id: row.try_get("id").unwrap_or_default(),
            name: row.try_get("name").unwrap_or_default(),
            cron_expr: row.try_get("cron_expr").unwrap_or_default(),
            queue: row.try_get("queue").unwrap_or_else(|_| "default".to_string()),
            job_name: row.try_get("job_name").unwrap_or_default(),
            payload,
            status,
            max_attempts: row.try_get("max_attempts").unwrap_or(3),
            last_fire_time: last_fire.and_then(|s| s.parse().ok()),
            next_fire_time: next_fire.and_then(|s| s.parse().ok()),
            created_at: created_at.parse().unwrap_or_default(),
            updated_at: updated_at.parse().unwrap_or_default(),
        }
    }
}

#[async_trait]
impl<'a> ScheduleStore for SqlScheduleStore<'a> {
    async fn create(&self, job: &ScheduledJob) -> StoreResult<String> {
        let id = job.id.clone();
        let payload = serde_json::to_string(&job.payload).unwrap_or_default();
        let next_fire = job.next_fire_time.map(|t| t.to_rfc3339());
        let status = job.status.as_str();

        let sql = self.sql(&format!(
            "INSERT INTO scheduled_jobs (id, name, cron_expr, queue, job_name, payload, status, max_attempts, next_fire_time, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
            .bind(&job.name)
            .bind(&job.cron_expr)
            .bind(&job.queue)
            .bind(&job.job_name)
            .bind(&payload)
            .bind(status)
            .bind(job.max_attempts)
            .bind(&next_fire)
            .bind(job.created_at.to_rfc3339())
            .bind(job.updated_at.to_rfc3339())
            .execute(self.pool)
            .await?;

        Ok(id)
    }

    async fn get(&self, id: &str) -> StoreResult<Option<ScheduledJob>> {
        let sql = self.sql(&format!(
            "SELECT * FROM scheduled_jobs WHERE id = {}",
            self.dialect.placeholder(1),
        ));
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        Ok(row.map(|r| self.row_to_job(&r)))
    }

    async fn list(&self, status: Option<ScheduleStatus>) -> StoreResult<Vec<ScheduledJob>> {
        let rows = match status {
            Some(s) => {
                let sql = self.sql(&format!(
                    "SELECT * FROM scheduled_jobs WHERE status = {} ORDER BY created_at DESC",
                    self.dialect.placeholder(1),
                ));
                sqlx::query(&sql)
                    .bind(s.as_str())
                    .fetch_all(self.pool)
                    .await?
            }
            None => {
                let sql = self.sql("SELECT * FROM scheduled_jobs ORDER BY created_at DESC");
                sqlx::query(&sql)
                    .fetch_all(self.pool)
                    .await?
            }
        };
        Ok(rows.iter().map(|r| self.row_to_job(r)).collect())
    }

    async fn update(&self, job: &ScheduledJob) -> StoreResult<()> {
        let payload = serde_json::to_string(&job.payload).unwrap_or_default();
        let next_fire = job.next_fire_time.map(|t| t.to_rfc3339());
        let last_fire = job.last_fire_time.map(|t| t.to_rfc3339());
        let status = job.status.as_str();
        let now = Utc::now().to_rfc3339();

        let sql = self.sql(&format!(
            "UPDATE scheduled_jobs SET name={}, cron_expr={}, queue={}, job_name={}, payload={}, status={}, max_attempts={}, last_fire_time={}, next_fire_time={}, updated_at={} WHERE id={}",
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
            .bind(&job.name)
            .bind(&job.cron_expr)
            .bind(&job.queue)
            .bind(&job.job_name)
            .bind(&payload)
            .bind(status)
            .bind(job.max_attempts)
            .bind(&last_fire)
            .bind(&next_fire)
            .bind(&now)
            .bind(&job.id)
            .execute(self.pool)
            .await?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> StoreResult<()> {
        let sql = self.sql(&format!(
            "DELETE FROM scheduled_jobs WHERE id = {}",
            self.dialect.placeholder(1),
        ));
        sqlx::query(&sql)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    async fn fetch_due(&self) -> StoreResult<Vec<ScheduledJob>> {
        let now = Utc::now().to_rfc3339();
        let sql = self.sql(&format!(
            "SELECT * FROM scheduled_jobs WHERE status = {} AND (next_fire_time IS NULL OR next_fire_time <= {}) ORDER BY next_fire_time ASC",
            self.dialect.placeholder(1),
            self.dialect.placeholder(1),
        ));
        let rows = sqlx::query(&sql)
            .bind(ScheduleStatus::Active.as_str())
            .bind(&now)
            .fetch_all(self.pool)
            .await?;
        Ok(rows.iter().map(|r| self.row_to_job(r)).collect())
    }
}

/// 调度器：负责触发到期任务并更新下次触发时间
pub struct Scheduler<'a, Q: Queue> {
    store: SqlScheduleStore<'a>,
    queue: &'a Q,
}

impl<'a, Q: Queue> Scheduler<'a, Q> {
    /// 创建调度器，绑定连接池、方言和队列
    pub fn new(pool: &'a Pool, dialect: &'a Dialect, queue: &'a Q) -> Self {
        Self {
            store: SqlScheduleStore::new(pool, dialect),
            queue,
        }
    }

    /// 执行一次 tick：取出到期任务，入队执行，更新触发时间
    pub async fn tick(&self) -> StoreResult<usize> {
        let due = self.store.fetch_due().await?;
        let mut fired = 0;

        for mut job in due {
            let now = Utc::now();

            // 构造队列任务并入队
            let queued = QueuedJob::new(&job.queue, &job.job_name, job.payload.clone())
                .with_max_attempts(job.max_attempts);
            self.queue.enqueue(queued).await?;

            // 更新触发时间
            job.last_fire_time = Some(now);
            match job.compute_next_fire_time() {
                Ok(next) => {
                    job.next_fire_time = Some(next);
                }
                Err(_) => {
                    // 无法计算下次触发时间，标记完成
                    job.status = ScheduleStatus::Completed;
                }
            }
            self.store.update(&job).await?;
            fired += 1;

            tracing::debug!("定时任务 '{}' 已触发，下次执行: {:?}", job.name, job.next_fire_time);
        }

        Ok(fired)
    }

    /// 计算到最近一个到期任务的等待时长
    pub async fn compute_sleep_duration(&self) -> std::time::Duration {
        let active = self.store.list(Some(ScheduleStatus::Active)).await;
        let Ok(jobs) = active else {
            return std::time::Duration::from_secs(30);
        };

        let now = Utc::now();
        let min_wait = jobs.iter().filter_map(|job| job.next_fire_time).map(|next| {
            (next - now).to_std().unwrap_or(std::time::Duration::from_millis(100))
        }).min();

        min_wait
            .unwrap_or(std::time::Duration::from_secs(30))
            .clamp(
                std::time::Duration::from_millis(100),
                std::time::Duration::from_secs(60),
            )
    }

    /// 启动后台调度循环
    pub async fn run(self: Arc<Self>) {
        loop {
            match self.tick().await {
                Ok(fired) => {
                    if fired > 0 {
                        tracing::info!("调度器 tick: 触发了 {} 个任务", fired);
                    }
                }
                Err(e) => {
                    tracing::error!("调度器 tick 失败: {:?}", e);
                }
            }

            let sleep = self.compute_sleep_duration().await;
            tokio::time::sleep(sleep).await;
        }
    }
}
