use std::sync::Arc;
use std::time::Instant;

use crate::auth::JwtAuthProvider;
use crate::db::seed;
use crate::plugin::PluginManager;
use crate::IngjooStore;
use ingjoo_cache::FrameworkCache;
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use ingjoo_core::ModelRegistry;
use ingjoo_security::SecurityPolicy;
use tokio::sync::broadcast;

type EventSender = broadcast::Sender<String>;

/// 限流配置
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 令牌桶容量（最大突发请求数）
    pub max_tokens: usize,
    /// 每秒补充令牌数
    pub refill_per_sec: f64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_tokens: 10,
            refill_per_sec: 1.0,
        }
    }
}

/// 应用全局状态，通过 `Arc<AppState>` 在 handler 和中间件间共享
pub struct AppState {
    /// 数据库存储实现（聚合 trait）
    pub store: Arc<dyn IngjooStore>,
    /// JWT 认证提供者
    pub auth: JwtAuthProvider,
    /// 动态模型注册表
    pub registry: Arc<ModelRegistry>,
    /// 数据库连接池
    pub pool: Arc<Pool>,
    /// 数据库方言（SQLite / PostgreSQL）
    pub dialect: Dialect,
    /// 广播事件通道
    pub events: EventSender,
    /// 服务启动时间
    pub start_time: Instant,
    /// 插件管理器（可选，按需初始化）
    pub plugin_manager: Option<Arc<PluginManager>>,
    /// 限流配置
    pub rate_limit: RateLimitConfig,
    /// 权限策略缓存
    pub cache: Arc<FrameworkCache<SecurityPolicy, ()>>,
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
            plugin_manager: None,
            rate_limit: RateLimitConfig::default(),
            cache: Arc::new(FrameworkCache::new()),
        }
    }

    pub async fn seed_model_access_from_registry(&self) -> anyhow::Result<()> {
        let models: Vec<String> = self.registry.list().iter().map(|m| m.name.clone()).collect();
        let model_refs: Vec<&str> = models.iter().map(|s| s.as_str()).collect();
        seed::seed_model_access(&self.pool, &self.dialect, &model_refs).await
    }

    pub fn with_plugin_manager(mut self, pm: Arc<PluginManager>) -> Self {
        self.plugin_manager = Some(pm);
        self
    }

    pub fn with_rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit = config;
        self
    }
}
