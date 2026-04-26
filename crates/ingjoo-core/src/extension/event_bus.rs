//! 事件总线 — 发布/订阅模式的事件分发机制

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

/// 事件消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub topic: String,
    pub payload: serde_json::Value,
    pub source: String,
}

/// 异步事件处理器类型
pub type EventHandler =
    Box<dyn Fn(Event) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>> + Send + Sync>;

/// 事件总线 — 支持主题订阅和异步事件分发
#[async_trait]
pub trait EventBus: Send + Sync {
    /// 发布事件到总线
    async fn publish(&self, event: Event) -> Result<(), anyhow::Error>;

    /// 订阅主题，返回订阅 ID
    async fn subscribe(&self, topic: &str, handler: EventHandler) -> Result<String, anyhow::Error>;

    /// 取消订阅
    async fn unsubscribe(&self, subscription_id: &str) -> Result<(), anyhow::Error>;
}
