mod cli;

use anyhow::Result;
use cli::Cli;
use clap::Parser;
use ingjoo_core::pool;
use ingjoo_core::ModelRegistry;
use ingjoo_infra::{AppState, AuthConfig, JwtAuthProvider, IngjooDb, IngjooStore, PluginManager, RateLimitConfig};
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
            Arc::new(pool),
            dialect,
        )
        .with_plugin_manager(plugin_manager.clone())
        .with_rate_limit(rate_limit),
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
