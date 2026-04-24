use std::sync::Arc;
use std::time::Instant;

use crate::auth::JwtAuthProvider;
use crate::db::seed;
use crate::IngjooStore;
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use ingjoo_core::ModelRegistry;
use tokio::sync::broadcast;

type EventSender = broadcast::Sender<String>;

pub struct AppState {
    pub store: Arc<dyn IngjooStore>,
    pub auth: JwtAuthProvider,
    pub registry: Arc<ModelRegistry>,
    pub pool: Arc<Pool>,
    pub dialect: Dialect,
    pub events: EventSender,
    pub start_time: Instant,
}

impl AppState {
    pub fn new(
        store: Arc<dyn IngjooStore>,
        auth: JwtAuthProvider,
        registry: Arc<ModelRegistry>,
        pool: Arc<Pool>,
        dialect: Dialect,
    ) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            store,
            auth,
            registry,
            pool,
            dialect,
            events,
            start_time: Instant::now(),
        }
    }

    pub async fn seed_model_access_from_registry(&self) -> anyhow::Result<()> {
        let models: Vec<String> = self.registry.list().iter().map(|m| m.name.clone()).collect();
        let model_refs: Vec<&str> = models.iter().map(|s| s.as_str()).collect();
        seed::seed_model_access(&self.pool, &self.dialect, &model_refs).await
    }
}
