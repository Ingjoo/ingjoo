//! 数据库连接池 — 驱动安装、连接创建与方言推断

use std::time::Duration;

use crate::Dialect;

/// 数据库连接池（sqlx::Any 后端）
pub type Pool = sqlx::pool::Pool<sqlx::Any>;

/// 安装 sqlx 默认数据库驱动（SQLite、Postgres）
pub fn install_drivers() {
    sqlx::any::install_default_drivers();
}

/// 根据 URL 连接数据库并推断方言（使用默认连接池配置）
pub async fn connect_pool(url: &str) -> Result<(Pool, Dialect), sqlx::Error> {
    let dialect = if url.starts_with("sqlite:") {
        Dialect::Sqlite
    } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
        Dialect::Postgres
    } else {
        return Err(sqlx::Error::Configuration(format!("Unsupported database URL: {}", url).into()));
    };
    let pool = Pool::connect(url).await?;
    Ok((pool, dialect))
}

/// 根据 URL 连接数据库并推断方言（使用自定义连接池配置）
pub async fn connect_pool_with_options(
    url: &str,
    max_connections: u32,
    min_connections: u32,
    acquire_timeout_secs: u64,
    idle_timeout_secs: u64,
    max_lifetime_secs: u64,
) -> Result<(Pool, Dialect), sqlx::Error> {
    let dialect = if url.starts_with("sqlite:") {
        Dialect::Sqlite
    } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
        Dialect::Postgres
    } else {
        return Err(sqlx::Error::Configuration(format!("Unsupported database URL: {}", url).into()));
    };
    let options = sqlx::pool::PoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(acquire_timeout_secs))
        .idle_timeout(Duration::from_secs(idle_timeout_secs))
        .max_lifetime(Duration::from_secs(max_lifetime_secs));
    let pool = options.connect(url).await?;
    Ok((pool, dialect))
}

/// 连接池统计信息
#[derive(Debug, serde::Serialize)]
pub struct PoolStats {
    /// 当前总连接数（活跃 + 空闲）
    pub total_connections: u32,
    /// 当前空闲连接数
    pub idle_connections: u32,
    /// 当前活跃连接数
    pub active_connections: u32,
    /// 最大连接数
    pub max_connections: u32,
}

impl PoolStats {
    /// 从连接池采集统计信息
    pub fn from_pool(pool: &Pool) -> Self {
        let size = pool.size();
        let idle = pool.num_idle() as u32;
        Self {
            total_connections: size,
            idle_connections: idle,
            active_connections: size.saturating_sub(idle),
            max_connections: pool.options().get_max_connections(),
        }
    }
}

/// 仅根据 URL 前缀推断方言，不创建连接
pub fn dialect_from_url(url: &str) -> Dialect {
    if url.starts_with("sqlite:") {
        Dialect::Sqlite
    } else {
        Dialect::Postgres
    }
}
