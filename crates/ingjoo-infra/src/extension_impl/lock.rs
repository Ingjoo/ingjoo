use async_trait::async_trait;
use ingjoo_core::extension::{Lock, LockGuard};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct InnerLock {
    token: String,
    expires_at: Instant,
}

/// 基于进程内 HashMap 的简易分布式锁
pub struct InMemoryLock {
    locks: Mutex<HashMap<String, InnerLock>>,
}

impl InMemoryLock {
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn generate_token() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn is_expired(expires_at: &Instant) -> bool {
        Instant::now() >= *expires_at
    }
}

impl Default for InMemoryLock {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Lock for InMemoryLock {
    async fn acquire(&self, key: &str, ttl_ms: u64) -> Result<LockGuard, anyhow::Error> {
        loop {
            match self.try_acquire(key, ttl_ms).await? {
                Some(guard) => return Ok(guard),
                None => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    }

    async fn try_acquire(&self, key: &str, ttl_ms: u64) -> Result<Option<LockGuard>, anyhow::Error> {
        let mut locks = self.locks.lock().unwrap();
        let token = Self::generate_token();
        let expires_at = Instant::now() + Duration::from_millis(ttl_ms);

        match locks.get(key) {
            Some(existing) if !Self::is_expired(&existing.expires_at) => Ok(None),
            _ => {
                locks.insert(
                    key.to_string(),
                    InnerLock {
                        token: token.clone(),
                        expires_at,
                    },
                );
                Ok(Some(LockGuard {
                    key: key.to_string(),
                    token,
                    expires_at,
                }))
            }
        }
    }

    async fn extend(&self, guard: &LockGuard, ttl_ms: u64) -> Result<(), anyhow::Error> {
        let mut locks = self.locks.lock().unwrap();
        match locks.get_mut(&guard.key) {
            Some(existing) if existing.token == guard.token => {
                existing.expires_at = Instant::now() + Duration::from_millis(ttl_ms);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("锁不存在或 token 不匹配")),
        }
    }
}

impl InMemoryLock {
    pub async fn release(&self, guard: &LockGuard) -> Result<(), anyhow::Error> {
        let mut locks = self.locks.lock().unwrap();
        match locks.get(&guard.key) {
            Some(existing) if existing.token == guard.token => {
                locks.remove(&guard.key);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("锁不存在或 token 不匹配")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn try_acquire_basic() {
        let lock = InMemoryLock::new();
        let guard = lock.try_acquire("test_key", 5000).await.unwrap().unwrap();
        assert_eq!(guard.key, "test_key");
        assert!(!guard.is_expired());
    }

    #[tokio::test]
    async fn try_acquire_conflict() {
        let lock = InMemoryLock::new();
        let _g1 = lock.try_acquire("key", 5000).await.unwrap().unwrap();
        let result = lock.try_acquire("key", 5000).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn try_acquire_different_keys() {
        let lock = InMemoryLock::new();
        let g1 = lock.try_acquire("key1", 5000).await.unwrap();
        let g2 = lock.try_acquire("key2", 5000).await.unwrap();
        assert!(g1.is_some());
        assert!(g2.is_some());
    }

    #[tokio::test]
    async fn release_allows_reacquire() {
        let lock = InMemoryLock::new();
        let guard = lock.try_acquire("key", 5000).await.unwrap().unwrap();
        lock.release(&guard).await.unwrap();
        let g2 = lock.try_acquire("key", 5000).await.unwrap();
        assert!(g2.is_some());
    }

    #[tokio::test]
    async fn extend_lock() {
        let lock = InMemoryLock::new();
        let guard = lock.try_acquire("key", 100).await.unwrap().unwrap();
        lock.extend(&guard, 5000).await.unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;

        let result = lock.try_acquire("key", 100).await.unwrap();
        assert!(result.is_none(), "extended lock should still be held");
    }

    #[tokio::test]
    async fn ttl_expiry() {
        let lock = InMemoryLock::new();
        let _guard = lock.try_acquire("key", 50).await.unwrap();

        tokio::time::sleep(Duration::from_millis(80)).await;

        let g2 = lock.try_acquire("key", 5000).await.unwrap();
        assert!(g2.is_some(), "expired lock should be available");
    }

    #[tokio::test]
    async fn acquire_retries_until_success() {
        let lock = InMemoryLock::new();
        let lock_clone = InMemoryLock::new();

        let _g1 = lock.try_acquire("key", 100).await.unwrap().unwrap();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            let _ = lock_clone;
        });

        let guard = tokio::time::timeout(
            Duration::from_secs(2),
            lock.acquire("key", 5000),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(guard.key, "key");

        handle.await.unwrap();
    }
}
