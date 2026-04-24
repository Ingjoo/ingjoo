use anyhow::Result;
use ingjoo_core::pool;
use ingjoo_infra::{ScaffDb, ScaffStore};
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
    ScaffDb::run_migrations(&pool, &dialect).await?;

    let scaff_store: Arc<dyn ScaffStore> = Arc::new(ScaffDb::new(pool.clone()));

    tracing::info!("ingjoo-bin started — 莺竹框架: 模块如竹，随需生长");

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);

    Ok(())
}
