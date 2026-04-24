//! 分布式锁 — 互斥访问控制

use async_trait::async_trait;

/// 分布式锁 — 支持获取、尝试获取和续期
#[async_trait]
pub trait Lock: Send + Sync {
    /// 阻塞式获取锁，直到成功或超时
    async fn acquire(&self, key: &str, ttl_ms: u64) -> Result<LockGuard, anyhow::Error>;

    /// 非阻塞式获取锁，失败返回 None
    async fn try_acquire(&self, key: &str, ttl_ms: u64) -> Result<Option<LockGuard>, anyhow::Error>;

    /// 续期已持有的锁
    async fn extend(&self, guard: &LockGuard, ttl_ms: u64) -> Result<(), anyhow::Error>;
}

/// 锁守卫 — 持有锁的凭证
#[derive(Debug, Clone)]
pub struct LockGuard {
    pub key: String,
    pub token: String,
    pub expires_at: std::time::Instant,
}

impl LockGuard {
    /// 检查锁是否已过期
    pub fn is_expired(&self) -> bool {
        std::time::Instant::now() >= self.expires_at
    }
}
