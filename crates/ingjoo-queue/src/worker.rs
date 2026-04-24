use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{Semaphore, broadcast};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{JobResult, QueuedJob, Queue, RetryPolicy};

/// 任务处理器 trait，按任务名称路由到具体实现
pub trait JobHandler: Send + Sync {
    fn handle(&self, payload: &Value) -> Pin<Box<dyn Future<Output = JobResult> + Send + '_>>;
}

/// 可配置并发的工作池，轮询队列并分发任务到已注册的处理器
pub struct WorkerPool {
    queue: Arc<dyn Queue>,
    handlers: HashMap<String, Arc<dyn JobHandler>>,
    concurrency: usize,
    retry_policy: RetryPolicy,
    poll_interval: Duration,
}

impl WorkerPool {
    /// 创建默认配置的工作池（并发数 4）
    pub fn new(queue: Arc<dyn Queue>) -> Self {
        Self {
            queue,
            handlers: HashMap::new(),
            concurrency: 4,
            retry_policy: RetryPolicy::new(),
            poll_interval: Duration::from_millis(100),
        }
    }

    /// 设置最大并发数
    pub fn with_concurrency(mut self, n: usize) -> Self {
        self.concurrency = n.max(1);
        self
    }

    /// 设置重试策略
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// 设置轮询间隔
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// 注册任务处理器
    pub fn register_handler(mut self, name: &str, handler: impl JobHandler + 'static) -> Self {
        self.handlers.insert(name.to_string(), Arc::new(handler));
        self
    }

    /// 启动工作池主循环，监听多个队列直到收到关闭信号
    pub async fn run(self: Arc<Self>, queues: Vec<String>, mut shutdown: broadcast::Receiver<()>) {
        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        info!("工作池启动，并发数: {}", self.concurrency);

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("收到关闭信号，等待进行中的任务完成...");
                    break;
                }
                _ = sleep(self.poll_interval) => {}
            }

            for queue_name in &queues {
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                match self.queue.dequeue(queue_name).await {
                    Ok(Some(job)) => {
                        let handler = self.handlers.get(&job.name).cloned();
                        let pool = self.clone();
                        let queue = self.queue.clone();
                        let retry_policy = self.retry_policy.clone();

                        tokio::spawn(async move {
                            let _permit = permit;
                            pool.process_job(queue.as_ref(), handler.as_deref(), &retry_policy, job).await;
                        });
                    }
                    Ok(None) => {}
                    Err(e) => {
                        error!("dequeue 失败: {}", e);
                    }
                }
            }
        }
    }

    async fn process_job(
        &self,
        queue: &dyn Queue,
        handler: Option<&dyn JobHandler>,
        retry_policy: &RetryPolicy,
        job: QueuedJob,
    ) {
        let handler = match handler {
            Some(h) => h,
            None => {
                warn!("未注册的处理器: {}", job.name);
                let _ = queue.dead_letter(&job.id).await;
                return;
            }
        };

        match handler.handle(&job.payload).await {
            JobResult::Ok => {
                if let Err(e) = queue.ack(&job.id).await {
                    error!("ack 失败 {}: {}", job.id, e);
                }
            }
            JobResult::Retry { delay } => {
                if let Err(e) = queue.nack(&job.id, "handler 请求重试").await {
                    error!("nack 失败 {}: {}", job.id, e);
                } else {
                    tokio::spawn(async move {
                        sleep(delay).await;
                    });
                }
            }
            JobResult::Failed(reason) => {
                if job.is_exhausted() {
                    warn!("任务 {} 超过最大重试次数，移入死信: {}", job.id, reason);
                    let _ = queue.dead_letter(&job.id).await;
                } else {
                    let delay = retry_policy.delay_for_attempt(job.attempts);
                    if let Err(e) = queue.nack(&job.id, &reason).await {
                        error!("nack 失败 {}: {}", job.id, e);
                    } else {
                        tokio::spawn(async move {
                            sleep(delay).await;
                        });
                    }
                }
            }
        }
    }
}

impl Clone for WorkerPool {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            handlers: self.handlers.clone(),
            concurrency: self.concurrency,
            retry_policy: self.retry_policy.clone(),
            poll_interval: self.poll_interval,
        }
    }
}
