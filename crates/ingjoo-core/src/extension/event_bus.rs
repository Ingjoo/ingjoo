use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub topic: String,
    pub payload: serde_json::Value,
    pub source: String,
}

pub type EventHandler = Box<
    dyn Fn(Event) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>> + Send + Sync,
>;

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: Event) -> Result<(), anyhow::Error>;

    async fn subscribe(
        &self,
        topic: &str,
        handler: EventHandler,
    ) -> Result<String, anyhow::Error>;

    async fn unsubscribe(&self, subscription_id: &str) -> Result<(), anyhow::Error>;
}
