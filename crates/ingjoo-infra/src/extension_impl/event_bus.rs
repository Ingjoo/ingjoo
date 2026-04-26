use async_trait::async_trait;
use ingjoo_core::extension::{Event, EventBus, EventHandler};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::RwLock;

type HandlerMap = HashMap<String, Vec<(String, EventHandler)>>;

/// 基于 tokio broadcast 的进程内事件总线
pub struct BroadcastEventBus {
    sender: broadcast::Sender<String>,
    handlers: RwLock<HandlerMap>,
    next_id: Mutex<u64>,
}

impl BroadcastEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, handlers: RwLock::new(HashMap::new()), next_id: Mutex::new(0) }
    }

    pub fn from_sender(sender: broadcast::Sender<String>) -> Self {
        Self { sender, handlers: RwLock::new(HashMap::new()), next_id: Mutex::new(0) }
    }

    fn alloc_id(&self) -> String {
        let mut next = self.next_id.lock().unwrap();
        let id = format!("sub_{}", *next);
        *next += 1;
        id
    }
}

#[async_trait]
impl EventBus for BroadcastEventBus {
    async fn publish(&self, event: Event) -> Result<(), anyhow::Error> {
        let payload = serde_json::to_string(&event)?;
        let _ = self.sender.send(payload);

        let handlers = self.handlers.read().await;
        if let Some(list) = handlers.get(&event.topic) {
            for (_id, handler) in list.iter() {
                let _ = handler(event.clone()).await;
            }
        }
        drop(handlers);

        Ok(())
    }

    async fn subscribe(&self, topic: &str, handler: EventHandler) -> Result<String, anyhow::Error> {
        let sub_id = self.alloc_id();
        let mut handlers = self.handlers.write().await;
        handlers.entry(topic.to_string()).or_default().push((sub_id.clone(), handler));
        Ok(sub_id)
    }

    async fn unsubscribe(&self, subscription_id: &str) -> Result<(), anyhow::Error> {
        let mut handlers = self.handlers.write().await;
        for (_topic, list) in handlers.iter_mut() {
            list.retain(|(id, _)| id != subscription_id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn make_event(topic: &str, payload: &str) -> Event {
        Event { topic: topic.to_string(), payload: serde_json::json!({ "data": payload }), source: "test".to_string() }
    }

    #[tokio::test]
    async fn publish_broadcasts_to_channel() {
        let bus = BroadcastEventBus::new(64);
        let mut rx = bus.sender.subscribe();

        let event = make_event("orders", "created");
        bus.publish(event).await.unwrap();

        let received = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await.unwrap().unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&received).unwrap();
        assert_eq!(parsed["topic"], "orders");
    }

    #[tokio::test]
    async fn subscribe_and_handle() {
        let bus = BroadcastEventBus::new(64);
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let handler: EventHandler = Box::new(move |_event| {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        });

        let sub_id = bus.subscribe("test_topic", handler).await.unwrap();

        bus.publish(make_event("test_topic", "a")).await.unwrap();
        bus.publish(make_event("test_topic", "b")).await.unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 2);

        bus.unsubscribe(&sub_id).await.unwrap();
        bus.publish(make_event("test_topic", "c")).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn different_topics_isolated() {
        let bus = BroadcastEventBus::new(64);
        let counter_a = Arc::new(AtomicUsize::new(0));
        let counter_b = Arc::new(AtomicUsize::new(0));

        let ca = counter_a.clone();
        let handler_a: EventHandler = Box::new(move |_| {
            let c = ca.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        });

        let cb = counter_b.clone();
        let handler_b: EventHandler = Box::new(move |_| {
            let c = cb.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        });

        bus.subscribe("a", handler_a).await.unwrap();
        bus.subscribe("b", handler_b).await.unwrap();

        bus.publish(make_event("a", "1")).await.unwrap();
        bus.publish(make_event("b", "1")).await.unwrap();
        bus.publish(make_event("a", "2")).await.unwrap();

        assert_eq!(counter_a.load(Ordering::SeqCst), 2);
        assert_eq!(counter_b.load(Ordering::SeqCst), 1);
    }
}
