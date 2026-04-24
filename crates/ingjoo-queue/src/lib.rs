//! 莺竹框架队列系统
//!
//! 提供内存和 SQL 持久化两种队列实现，
//! 支持优先级排序、延迟执行、指数退避重试和可配置并发的工作池。

pub mod memory;
pub mod retry;
pub mod scheduler;
pub mod sql;
pub mod worker;

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ingjoo_core::db::error::StoreResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use memory::InMemoryQueue;
pub use retry::RetryPolicy;
pub use scheduler::{ScheduledJob, ScheduleStatus, ScheduleStore, Scheduler, SqlScheduleStore};
pub use sql::SqlQueue;
pub use worker::{JobHandler, WorkerPool};

/// 任务状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// 等待执行
    Pending,
    /// 正在执行
    Running,
    /// 执行成功
    Completed,
    /// 执行失败（可重试）
    Failed,
    /// 死信（超过最大重试次数）
    DeadLetter,
}

impl JobStatus {
    /// 状态的字符串表示（用于 DB 存储）
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::DeadLetter => "dead_letter",
        }
    }

    /// 从字符串解析状态
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "dead_letter" => Some(Self::DeadLetter),
            _ => None,
        }
    }
}

/// 任务执行结果
#[derive(Debug)]
pub enum JobResult {
    /// 成功完成
    Ok,
    /// 需要重试，附带建议延迟
    Retry { delay: Duration },
    /// 彻底失败，附带错误信息
    Failed(String),
}

/// 队列中的任务数据模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedJob {
    /// 任务唯一 ID（UUID）
    pub id: String,
    /// 队列名称，默认 "default"
    pub queue: String,
    /// 任务类型名称（用于路由到对应 handler）
    pub name: String,
    /// JSON 序列化参数
    pub payload: Value,
    /// 优先级（数值越小越优先），默认 0
    pub priority: i32,
    /// 当前状态
    pub status: JobStatus,
    /// 已尝试次数
    pub attempts: i32,
    /// 最大重试次数，默认 3
    pub max_attempts: i32,
    /// 延迟执行时间（为 None 则立即执行）
    pub run_at: Option<DateTime<Utc>>,
    /// 开始执行时间
    pub started_at: Option<DateTime<Utc>>,
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
    /// 错误信息
    pub error: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl QueuedJob {
    /// 创建新任务，使用默认值
    pub fn new(queue: &str, name: &str, payload: Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            queue: queue.to_string(),
            name: name.to_string(),
            payload,
            priority: 0,
            status: JobStatus::Pending,
            attempts: 0,
            max_attempts: 3,
            run_at: None,
            started_at: None,
            completed_at: None,
            error: None,
            created_at: Utc::now(),
        }
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// 设置延迟执行
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.run_at = Some(Utc::now() + chrono::Duration::from_std(delay).unwrap_or_default());
        self
    }

    /// 设置最大重试次数
    pub fn with_max_attempts(mut self, max: i32) -> Self {
        self.max_attempts = max;
        self
    }

    /// 是否已超过最大重试次数
    pub fn is_exhausted(&self) -> bool {
        self.attempts >= self.max_attempts
    }
}

/// 队列抽象 trait
#[async_trait]
pub trait Queue: Send + Sync {
    /// 将任务加入队列，返回任务 ID
    async fn enqueue(&self, job: QueuedJob) -> StoreResult<String>;

    /// 从队列中取出下一个待执行任务
    async fn dequeue(&self, queue: &str) -> StoreResult<Option<QueuedJob>>;

    /// 确认任务执行成功
    async fn ack(&self, job_id: &str) -> StoreResult<()>;

    /// 确认任务执行失败
    async fn nack(&self, job_id: &str, reason: &str) -> StoreResult<()>;

    /// 将任务重新入队（重试）
    async fn retry(&self, job_id: &str) -> StoreResult<()>;

    /// 将任务移入死信队列
    async fn dead_letter(&self, job_id: &str) -> StoreResult<()>;

    /// 获取队列中待处理任务数量
    async fn pending_count(&self, queue: &str) -> StoreResult<i64>;
}
