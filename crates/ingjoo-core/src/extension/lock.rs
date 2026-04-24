use async_trait::async_trait;

#[async_trait]
pub trait Lock: Send + Sync {
    async fn acquire(&self, key: &str, ttl_ms: u64) -> Result<LockGuard, anyhow::Error>;

    async fn try_acquire(&self, key: &str, ttl_ms: u64) -> Result<Option<LockGuard>, anyhow::Error>;

    async fn extend(&self, guard: &LockGuard, ttl_ms: u64) -> Result<(), anyhow::Error>;
}

#[derive(Debug, Clone)]
pub struct LockGuard {
    pub key: String,
    pub token: String,
    pub expires_at: std::time::Instant,
}

impl LockGuard {
    pub fn is_expired(&self) -> bool {
        std::time::Instant::now() >= self.expires_at
    }
}
