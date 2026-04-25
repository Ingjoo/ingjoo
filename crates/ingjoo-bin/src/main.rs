mod cli;

use anyhow::Result;
use cli::Cli;
use clap::Parser;
use ingjoo_core::pool;
use ingjoo_core::ModelRegistry;
use ingjoo_infra::{AppState, AuthConfig, JwtAuthProvider, IngjooDb, IngjooStore, PluginManager, RateLimitConfig};
use ingjoo_infra::db::database_manager::DatabaseManager;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(&cli.log_level)
        .init();

    pool::install_drivers();
    let (pool, dialect) = pool::connect_pool_with_options(
        &cli.database_url,
        10,    // max_connections
        1,     // min_connections
        30,    // acquire_timeout_secs
        600,   // idle_timeout_secs
        1800,  // max_lifetime_secs
    )
    .await?;
    IngjooDb::run_migrations(&pool, &dialect).await?;

    let mut db_manager = DatabaseManager::new(pool.clone(), dialect);
    if let Some(ref base_url) = cli.database_base_url {
        db_manager = db_manager.with_base_url(base_url.as_str());
        tracing::info!("多数据库模式已启用，基础 URL: {}", base_url);
    }
    let db_manager = Arc::new(db_manager);

    let scaff_store: Arc<dyn IngjooStore> = Arc::new(IngjooDb::with_dialect(pool.clone(), dialect));
    let auth_config = AuthConfig::new(&cli.jwt_secret);
    let auth_provider = JwtAuthProvider::new(&auth_config);
    let registry = Arc::new(ModelRegistry::new());

    let plugin_manager = Arc::new(PluginManager::new(
        registry.clone(),
        Arc::new(pool.clone()),
        dialect,
    ));

    let rate_limit = RateLimitConfig {
        max_tokens: std::env::var("INGJOO_RATE_LIMIT_MAX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10),
        refill_per_sec: std::env::var("INGJOO_RATE_LIMIT_REFILL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0),
    };

    let state = Arc::new(
        AppState::new(
            scaff_store,
            auth_provider,
            registry,
            Arc::new(pool.clone()),
            dialect,
            db_manager,
        )
        .with_plugin_manager(plugin_manager.clone())
        .with_rate_limit(rate_limit)
        .with_audit(build_audit(pool, dialect))
        .with_content_filter(build_content_filter())
        .with_data_mask(build_data_mask())
        .with_signature(build_signature())
    );

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

    let app = ingjoo_infra::router::base_router(state);

    tracing::info!("ingjoo-bin started — 莺竹框架: 模块如竹，随需生长");
    tracing::info!("Listening on {}", cli.bind);

    let listener = tokio::net::TcpListener::bind(&cli.bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

use ingjoo_core::extension::{AuditStore, ContentFilter, DataMask, SignatureVerifier};
use ingjoo_infra::extension_noop::*;

#[cfg(feature = "db")]
fn build_audit(pool: ingjoo_core::pool::Pool, dialect: ingjoo_core::Dialect) -> Arc<dyn AuditStore> {
    Arc::new(ingjoo_infra::extension_impl::DbAuditStore::new(pool, dialect))
}

#[cfg(not(feature = "db"))]
fn build_audit(_pool: ingjoo_core::pool::Pool, _dialect: ingjoo_core::Dialect) -> Arc<dyn AuditStore> {
    Arc::new(NoopAuditStore)
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
    Arc::new(ingjoo_infra::extension_impl::AesDataMask::new(
        &ingjoo_infra::extension_impl::AesDataMask::generate_key(),
    ))
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
