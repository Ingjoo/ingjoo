use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use ingjoo_core::db::error::StoreResult;
use tracing::warn;

use crate::{JobStatus, QueuedJob, Queue};

/// 基于内存的双端队列实现，用于开发和测试
pub struct InMemoryQueue {
    jobs: Mutex<VecDeque<QueuedJob>>,
}

impl InMemoryQueue {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(VecDeque::new()),
        }
    }
}

impl Default for InMemoryQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Queue for InMemoryQueue {
    async fn enqueue(&self, job: QueuedJob) -> StoreResult<String> {
        let id = job.id.clone();
        let mut jobs = self.jobs.lock().unwrap();
        jobs.push_back(job);
        Ok(id)
    }

    async fn dequeue(&self, queue: &str) -> StoreResult<Option<QueuedJob>> {
        let mut jobs = self.jobs.lock().unwrap();
        let now = Utc::now();

        let mut best_idx: Option<usize> = None;
        let mut best_priority = i32::MAX;

        for (i, j) in jobs.iter().enumerate() {
            if j.queue != queue || j.status != JobStatus::Pending {
                continue;
            }
            if let Some(run_at) = j.run_at {
                if run_at > now {
                    continue;
                }
            }
            if j.priority < best_priority {
                best_priority = j.priority;
                best_idx = Some(i);
            }
        }

        if let Some(idx) = best_idx {
            let job = &mut jobs[idx];
            job.status = JobStatus::Running;
            job.started_at = Some(now);
            job.attempts += 1;
            Ok(Some(job.clone()))
        } else {
            Ok(None)
        }
    }

    async fn ack(&self, job_id: &str) -> StoreResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        for j in jobs.iter_mut() {
            if j.id == job_id {
                j.status = JobStatus::Completed;
                j.completed_at = Some(Utc::now());
                return Ok(());
            }
        }
        warn!("ack 未找到任务: {}", job_id);
        Ok(())
    }

    async fn nack(&self, job_id: &str, reason: &str) -> StoreResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        for j in jobs.iter_mut() {
            if j.id == job_id {
                j.status = JobStatus::Failed;
                j.error = Some(reason.to_string());
                if j.is_exhausted() {
                    j.status = JobStatus::DeadLetter;
                }
                return Ok(());
            }
        }
        warn!("nack 未找到任务: {}", job_id);
        Ok(())
    }

    async fn retry(&self, job_id: &str) -> StoreResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        for j in jobs.iter_mut() {
            if j.id == job_id {
                j.status = JobStatus::Pending;
                j.error = None;
                j.started_at = None;
                return Ok(());
            }
        }
        warn!("retry 未找到任务: {}", job_id);
        Ok(())
    }

    async fn dead_letter(&self, job_id: &str) -> StoreResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        for j in jobs.iter_mut() {
            if j.id == job_id {
                j.status = JobStatus::DeadLetter;
                j.completed_at = Some(Utc::now());
                return Ok(());
            }
        }
        warn!("dead_letter 未找到任务: {}", job_id);
        Ok(())
    }

    async fn pending_count(&self, queue: &str) -> StoreResult<i64> {
        let jobs = self.jobs.lock().unwrap();
        Ok(jobs.iter().filter(|j| j.queue == queue && j.status == JobStatus::Pending).count() as i64)
    }
}
