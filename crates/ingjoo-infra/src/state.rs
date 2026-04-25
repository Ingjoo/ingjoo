use std::sync::Arc;
use std::time::Instant;

use crate::auth::JwtAuthProvider;
use crate::db::database_manager::DatabaseManager;
use crate::db::seed;
use crate::extension_noop::*;
use crate::plugin::PluginManager;
use crate::storage::FileStorage;
use crate::IngjooStore;
use ingjoo_cache::FrameworkCache;
use ingjoo_core::extension::{
    AuditStore, ContentFilter, DataMask, DocumentLoader, InputSanitizer,
    LlmProvider, PaymentProvider, RelationLoader, SearchEngine, SignatureVerifier,
    StateMachine, TextSplitter, TranslationStore, VectorStore,
};
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
    // ── 核心基础设施 ──
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
    /// 多数据库连接池管理器
    pub db_manager: Arc<DatabaseManager>,
    /// 广播事件通道
    pub events: EventSender,
    /// 服务启动时间
    pub start_time: Instant,
    /// 插件管理器（可选，按需初始化）
    pub plugin_manager: Option<Arc<PluginManager>>,
    /// 文件存储
    pub file_storage: Arc<dyn FileStorage>,
    /// 限流配置
    pub rate_limit: RateLimitConfig,
    /// 权限策略缓存
    pub cache: Arc<FrameworkCache<SecurityPolicy, ()>>,

    // ── 扩展 trait（均有 noop 默认） ──
    /// 审计日志
    pub audit: Arc<dyn AuditStore>,
    /// 内容过滤
    pub content_filter: Arc<dyn ContentFilter>,
    /// 数据脱敏
    pub data_mask: Arc<dyn DataMask>,
    /// 文档加载
    pub document_loader: Arc<dyn DocumentLoader>,
    /// 文档分割
    pub text_splitter: Arc<dyn TextSplitter>,
    /// LLM 服务
    pub llm: Arc<dyn LlmProvider>,
    /// 输入净化
    pub sanitizer: Arc<dyn InputSanitizer>,
    /// 签名验证
    pub signature: Arc<dyn SignatureVerifier>,
    /// 状态机
    pub state_machine: Arc<dyn StateMachine>,
    /// 搜索引擎
    pub search: Arc<dyn SearchEngine>,
    /// 支付
    pub payment: Arc<dyn PaymentProvider>,
    /// 关系加载
    pub relation_loader: Arc<dyn RelationLoader>,
    /// 翻译
    pub translation: Arc<dyn TranslationStore>,
    /// 向量存储
    pub vector: Arc<dyn VectorStore>,
}

impl AppState {
    pub fn new(
        store: Arc<dyn IngjooStore>,
        auth: JwtAuthProvider,
        registry: Arc<ModelRegistry>,
        pool: Arc<Pool>,
        dialect: Dialect,
        db_manager: Arc<DatabaseManager>,
    ) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            store,
            auth,
            registry,
            pool,
            dialect,
            db_manager,
            events,
            start_time: Instant::now(),
            plugin_manager: None,
            file_storage: Arc::new(crate::storage::LocalStorage::new(std::path::PathBuf::from("./data/uploads"))),
            rate_limit: RateLimitConfig::default(),
            cache: Arc::new(FrameworkCache::new()),

            // 扩展 trait — 默认 noop
            audit: Arc::new(NoopAuditStore),
            content_filter: Arc::new(NoopContentFilter),
            data_mask: Arc::new(NoopDataMask),
            document_loader: Arc::new(NoopDocumentLoader),
            text_splitter: Arc::new(NoopTextSplitter),
            llm: Arc::new(NoopLlmProvider),
            sanitizer: Arc::new(NoopInputSanitizer),
            signature: Arc::new(NoopSignatureVerifier),
            state_machine: Arc::new(NoopStateMachine),
            search: Arc::new(NoopSearchEngine),
            payment: Arc::new(NoopPaymentProvider),
            relation_loader: Arc::new(NoopRelationLoader),
            translation: Arc::new(NoopTranslationStore),
            vector: Arc::new(NoopVectorStore),
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

    pub fn with_file_storage(mut self, storage: Arc<dyn FileStorage>) -> Self {
        self.file_storage = storage;
        self
    }

    // ── 扩展 trait builder 方法 ──

    pub fn with_audit(mut self, audit: Arc<dyn AuditStore>) -> Self {
        self.audit = audit;
        self
    }

    pub fn with_content_filter(mut self, filter: Arc<dyn ContentFilter>) -> Self {
        self.content_filter = filter;
        self
    }

    pub fn with_data_mask(mut self, mask: Arc<dyn DataMask>) -> Self {
        self.data_mask = mask;
        self
    }

    pub fn with_document_loader(mut self, loader: Arc<dyn DocumentLoader>) -> Self {
        self.document_loader = loader;
        self
    }

    pub fn with_text_splitter(mut self, splitter: Arc<dyn TextSplitter>) -> Self {
        self.text_splitter = splitter;
        self
    }

    pub fn with_llm(mut self, llm: Arc<dyn LlmProvider>) -> Self {
        self.llm = llm;
        self
    }

    pub fn with_sanitizer(mut self, sanitizer: Arc<dyn InputSanitizer>) -> Self {
        self.sanitizer = sanitizer;
        self
    }

    pub fn with_signature(mut self, verifier: Arc<dyn SignatureVerifier>) -> Self {
        self.signature = verifier;
        self
    }

    pub fn with_state_machine(mut self, sm: Arc<dyn StateMachine>) -> Self {
        self.state_machine = sm;
        self
    }

    pub fn with_search(mut self, engine: Arc<dyn SearchEngine>) -> Self {
        self.search = engine;
        self
    }

    pub fn with_payment(mut self, provider: Arc<dyn PaymentProvider>) -> Self {
        self.payment = provider;
        self
    }

    pub fn with_relation_loader(mut self, loader: Arc<dyn RelationLoader>) -> Self {
        self.relation_loader = loader;
        self
    }

    pub fn with_translation(mut self, store: Arc<dyn TranslationStore>) -> Self {
        self.translation = store;
        self
    }

    pub fn with_vector(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.vector = store;
        self
    }
}
