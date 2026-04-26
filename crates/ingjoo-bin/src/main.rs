mod cli;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use ingjoo_core::pool;
use ingjoo_core::ModelRegistry;
use ingjoo_infra::auth::AuthProvider;
use ingjoo_infra::db::database_manager::DatabaseManager;
use ingjoo_infra::{
    AppState, AuthConfig, FileStorage, IngjooDb, IngjooStore, JwtAuthProvider, LocalStorage, PluginManager,
    RateLimitConfig,
};
#[cfg(feature = "s3")]
use ingjoo_infra::S3Storage;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.log_format.as_str() {
        "json" => {
            tracing_subscriber::fmt().with_env_filter(&cli.log_level).json().init();
        }
        _ => {
            tracing_subscriber::fmt().with_env_filter(&cli.log_level).init();
        }
    }

    pool::install_drivers();
    let (pool, dialect) = pool::connect_pool_with_options(
        &cli.database_url,
        10,   // max_connections
        1,    // min_connections
        30,   // acquire_timeout_secs
        600,  // idle_timeout_secs
        1800, // max_lifetime_secs
    )
    .await?;
    let auth_config = AuthConfig::new(&cli.jwt_secret);
    let auth_provider = JwtAuthProvider::new(&auth_config);
    let admin_hash = auth_provider.hash_password("admin").map_err(|e| anyhow::anyhow!("哈希管理员密码失败: {}", e))?;
    ingjoo_infra::db::init_database(&pool, &dialect, &admin_hash).await?;

    let mut db_manager = DatabaseManager::new(pool.clone(), dialect);
    if let Some(ref base_url) = cli.database_base_url {
        db_manager = db_manager.with_base_url(base_url.as_str());
        tracing::info!("多数据库模式已启用，基础 URL: {}", base_url);
    }
    let db_manager = Arc::new(db_manager);

    // 多数据库启动：确保数据目录存在 + 发现已有数据库
    #[cfg(feature = "multi-db")]
    {
        if db_manager.is_multi_db_enabled() {
            if let Some(base_url) = db_manager.base_url() {
                if base_url.starts_with("sqlite:") {
                    let dir =
                        base_url.trim_start_matches("sqlite:").trim_end_matches('/').split('?').next().unwrap_or(".");
                    std::fs::create_dir_all(dir)
                        .map_err(|e| anyhow::anyhow!("创建数据库目录 '{}' 失败: {}", dir, e))?;
                }
            }

            let discovered = db_manager.discover_and_register().await;
            if discovered.is_empty() {
                tracing::info!("多数据库模式已启用，未发现已有租户数据库");
            } else {
                tracing::info!("多数据库模式已启用，发现 {} 个租户数据库: {:?}", discovered.len(), discovered);
            }
        }
    }

    let scaff_store: Arc<dyn IngjooStore> = Arc::new(IngjooDb::with_dialect(pool.clone(), dialect));
    let registry = Arc::new(ModelRegistry::new());

    let plugin_manager = Arc::new(PluginManager::new(registry.clone(), Arc::new(pool.clone()), dialect));

    let rate_limit = RateLimitConfig {
        max_tokens: std::env::var("INGJOO_RATE_LIMIT_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
        refill_per_sec: std::env::var("INGJOO_RATE_LIMIT_REFILL").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0),
    };

    let app_state = AppState::new(scaff_store, auth_provider, registry, Arc::new(pool.clone()), dialect, db_manager)
        .with_plugin_manager(plugin_manager.clone())
        .with_rate_limit(rate_limit)
        .with_audit(build_audit(pool.clone(), dialect))
        .with_notification(build_notification(pool.clone(), dialect))
        .with_content_filter(build_content_filter())
        .with_data_mask(build_data_mask())
        .with_signature(build_signature())
        .with_relation_loader(build_relation_loader(pool.clone(), dialect))
        .with_translation(build_translation(pool.clone(), dialect))
        .with_search(build_search(pool.clone(), dialect))
        .with_state_machine(build_state_machine(pool, dialect))
        .with_text_splitter(build_text_splitter())
        .with_file_storage(build_file_storage().await);

    #[cfg(feature = "email")]
    let app_state = app_state.with_email(build_email());

    #[cfg(feature = "sms")]
    let app_state = app_state.with_sms(build_sms());

    #[cfg(feature = "multi-db")]
    let app_state = app_state.with_multi_db_config(cli.admin_passwd.clone(), cli.list_db, cli.dbfilter.clone());

    let state = Arc::new(app_state);

    let plugins_path = std::path::Path::new(&cli.plugins_dir);
    if plugins_path.exists() {
        match plugin_manager.load_from_dir(plugins_path).await {
            Ok(loaded) => {
                if !loaded.is_empty() {
                    tracing::info!("已加载 {} 个插件: {:?}", loaded.len(), loaded);
                }
            }
            Err(e) => tracing::warn!("加载插件失败: {}", e),
        }
    } else {
        tracing::info!("插件目录 {} 不存在，跳过自动加载", cli.plugins_dir);
    }

    let app = ingjoo_infra::router::base_router(state.clone());

    let audit_retention_days = cli.audit_retention_days;
    let cleanup_audit = state.audit.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400));
        loop {
            interval.tick().await;
            let cutoff = chrono::Utc::now() - chrono::Duration::days(audit_retention_days as i64);
            let cutoff_str = cutoff.to_rfc3339();
            match cleanup_audit.delete_logs_before(&cutoff_str).await {
                Ok(deleted) if deleted > 0 => {
                    tracing::info!("审计日志清理: 删除 {} 条超过 {} 天的记录", deleted, audit_retention_days);
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("审计日志清理失败: {}", e);
                }
            }
        }
    });

    tracing::info!("ingjoo-bin started — 莺竹框架: 模块如竹，随需生长");
    tracing::info!("Listening on {}", cli.bind);

    let listener = tokio::net::TcpListener::bind(&cli.bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

use ingjoo_core::extension::{
    AuditStore, ContentFilter, DataMask, NotificationStore, RelationLoader, SearchEngine, SignatureVerifier,
    StateMachine, TextSplitter, TranslationStore,
};
#[allow(unused_imports)]
use ingjoo_infra::extension_noop::{
    NoopAuditStore, NoopContentFilter, NoopDataMask, NoopNotificationStore, NoopRelationLoader, NoopSearchEngine,
    NoopSignatureVerifier, NoopStateMachine, NoopTranslationStore,
};
#[cfg(feature = "email")]
use ingjoo_infra::extension_noop::NoopEmailProvider;
#[cfg(feature = "email")]
use ingjoo_infra::email::EmailProvider;
#[cfg(feature = "sms")]
use ingjoo_infra::extension_noop::NoopSmsProvider;
#[cfg(feature = "sms")]
use ingjoo_infra::sms::SmsProvider;

#[cfg(feature = "db")]
fn build_audit(pool: ingjoo_core::pool::Pool, dialect: ingjoo_core::Dialect) -> Arc<dyn AuditStore> {
    Arc::new(ingjoo_infra::extension_impl::DbAuditStore::new(pool, dialect))
}

#[cfg(not(feature = "db"))]
fn build_audit(_pool: ingjoo_core::pool::Pool, _dialect: ingjoo_core::Dialect) -> Arc<dyn AuditStore> {
    Arc::new(NoopAuditStore)
}

#[cfg(feature = "db")]
fn build_notification(pool: ingjoo_core::pool::Pool, dialect: ingjoo_core::Dialect) -> Arc<dyn NotificationStore> {
    Arc::new(ingjoo_infra::extension_impl::DbNotificationStore::new(pool, dialect))
}

#[cfg(not(feature = "db"))]
fn build_notification(_pool: ingjoo_core::pool::Pool, _dialect: ingjoo_core::Dialect) -> Arc<dyn NotificationStore> {
    Arc::new(NoopNotificationStore)
}

#[cfg(feature = "content-filter")]
fn build_content_filter() -> Arc<dyn ContentFilter> {
    Arc::new(ingjoo_infra::extension_impl::KeywordContentFilter::with_defaults())
}

#[cfg(not(feature = "content-filter"))]
fn build_content_filter() -> Arc<dyn ContentFilter> {
    Arc::new(NoopContentFilter)
}

#[cfg(feature = "data-mask")]
fn build_data_mask() -> Arc<dyn DataMask> {
    Arc::new(ingjoo_infra::extension_impl::AesDataMask::new(&ingjoo_infra::extension_impl::AesDataMask::generate_key()))
}

#[cfg(not(feature = "data-mask"))]
fn build_data_mask() -> Arc<dyn DataMask> {
    Arc::new(NoopDataMask)
}

#[cfg(feature = "signature")]
fn build_signature() -> Arc<dyn SignatureVerifier> {
    Arc::new(ingjoo_infra::extension_impl::HmacSignatureVerifier::new())
}

#[cfg(not(feature = "signature"))]
fn build_signature() -> Arc<dyn SignatureVerifier> {
    Arc::new(NoopSignatureVerifier)
}

#[cfg(feature = "db")]
fn build_relation_loader(pool: ingjoo_core::pool::Pool, dialect: ingjoo_core::Dialect) -> Arc<dyn RelationLoader> {
    Arc::new(ingjoo_infra::extension_impl::DbRelationLoader::new(pool, dialect))
}

#[cfg(not(feature = "db"))]
fn build_relation_loader(_pool: ingjoo_core::pool::Pool, _dialect: ingjoo_core::Dialect) -> Arc<dyn RelationLoader> {
    Arc::new(NoopRelationLoader)
}

#[cfg(feature = "db")]
fn build_translation(pool: ingjoo_core::pool::Pool, dialect: ingjoo_core::Dialect) -> Arc<dyn TranslationStore> {
    Arc::new(ingjoo_infra::extension_impl::DbTranslationStore::new(pool, dialect))
}

#[cfg(not(feature = "db"))]
fn build_translation(_pool: ingjoo_core::pool::Pool, _dialect: ingjoo_core::Dialect) -> Arc<dyn TranslationStore> {
    Arc::new(NoopTranslationStore)
}

#[cfg(feature = "db")]
fn build_state_machine(pool: ingjoo_core::pool::Pool, dialect: ingjoo_core::Dialect) -> Arc<dyn StateMachine> {
    Arc::new(ingjoo_infra::extension_impl::DbStateMachine::new(pool, dialect))
}

#[cfg(not(feature = "db"))]
fn build_state_machine(_pool: ingjoo_core::pool::Pool, _dialect: ingjoo_core::Dialect) -> Arc<dyn StateMachine> {
    Arc::new(NoopStateMachine)
}

#[cfg(feature = "db")]
fn build_search(pool: ingjoo_core::pool::Pool, dialect: ingjoo_core::Dialect) -> Arc<dyn SearchEngine> {
    Arc::new(ingjoo_infra::extension_impl::DbSearchEngine::new(pool, dialect))
}

#[cfg(not(feature = "db"))]
fn build_search(_pool: ingjoo_core::pool::Pool, _dialect: ingjoo_core::Dialect) -> Arc<dyn SearchEngine> {
    Arc::new(NoopSearchEngine)
}

fn build_text_splitter() -> Arc<dyn TextSplitter> {
    Arc::new(ingjoo_infra::extension_impl::CharTextSplitter::with_defaults())
}

async fn build_file_storage() -> Arc<dyn FileStorage> {
    let backend = std::env::var("INGJOO_STORAGE_BACKEND").unwrap_or_else(|_| "local".to_string());

    match backend.as_str() {
        "s3" => build_s3_storage().await,
        _ => {
            tracing::info!("文件存储后端: {}", backend);
            build_local_storage(None)
        }
    }
}

fn build_local_storage(custom_path: Option<std::path::PathBuf>) -> Arc<dyn FileStorage> {
    let path = custom_path.unwrap_or_else(|| {
        std::env::var("INGJOO_STORAGE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("./data/uploads"))
    });
    Arc::new(LocalStorage::new(path))
}

#[cfg(feature = "s3")]
async fn build_s3_storage() -> Arc<dyn FileStorage> {
    let bucket = match std::env::var("INGJOO_S3_BUCKET") {
        Ok(b) => b,
        Err(_) => {
            tracing::warn!("S3 后端已选择但未配置 INGJOO_S3_BUCKET，回退到本地存储");
            return build_local_storage(None);
        }
    };

    if let Ok(endpoint) = std::env::var("INGJOO_S3_ENDPOINT") {
        let region = std::env::var("INGJOO_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        tracing::info!("文件存储后端: s3 (endpoint: {}, bucket: {}, region: {})", endpoint, bucket, region);
        match S3Storage::from_endpoint(endpoint, bucket, region).await {
            Ok(storage) => Arc::new(storage),
            Err(e) => {
                tracing::warn!("创建 S3 存储失败: {}，回退到本地存储", e);
                build_local_storage(None)
            }
        }
    } else {
        tracing::info!("文件存储后端: s3 (bucket: {})", bucket);
        match S3Storage::new(bucket).await {
            Ok(storage) => Arc::new(storage),
            Err(e) => {
                tracing::warn!("创建 S3 存储失败: {}，回退到本地存储", e);
                build_local_storage(None)
            }
        }
    }
}

#[cfg(not(feature = "s3"))]
async fn build_s3_storage() -> Arc<dyn FileStorage> {
    tracing::warn!("S3 后端已选择但 s3 feature 未启用，回退到本地存储");
    build_local_storage(None)
}

#[cfg(feature = "email")]
fn build_email() -> Arc<dyn EmailProvider> {
    Arc::new(NoopEmailProvider)
}

#[cfg(feature = "sms")]
fn build_sms() -> Arc<dyn SmsProvider> {
    Arc::new(NoopSmsProvider)
}
