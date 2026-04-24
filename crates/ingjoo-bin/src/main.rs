use anyhow::Result;
use ingjoo_core::pool;
use ingjoo_core::ModelRegistry;
use ingjoo_infra::{AppState, AuthConfig, JwtAuthProvider, IngjooDb, IngjooStore};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("ingjoo_bin=debug")
        .init();

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data/ingjoo.db?mode=rwc".to_string());

    pool::install_drivers();
    let (pool, dialect) = pool::connect_pool(&db_url).await?;
    IngjooDb::run_migrations(&pool, &dialect).await?;

    let scaff_store: Arc<dyn IngjooStore> = Arc::new(IngjooDb::with_dialect(pool.clone(), dialect));
    let auth_config = AuthConfig::new("ingjoo-default-secret-change-me");
    let auth_provider = JwtAuthProvider::new(&auth_config);
    let registry = Arc::new(ModelRegistry::new());

    let state = Arc::new(AppState::new(
        scaff_store,
        auth_provider,
        registry,
        Arc::new(pool),
        dialect,
    ));

    let app = ingjoo_infra::router::base_router(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("ingjoo-bin started — 莺竹框架: 模块如竹，随需生长");
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
