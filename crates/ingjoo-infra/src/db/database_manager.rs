use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use ingjoo_core::pool::{connect_pool_with_options, Pool};
use ingjoo_core::Dialect;

const DEFAULT_DB_NAME: &str = "main";

/// 多数据库连接池管理器
///
/// 维护命名数据库到连接池的映射，支持动态创建/移除连接池。
/// 当 `base_url` 为 None 时退化为单数据库模式，所有操作走默认连接池。
pub struct DatabaseManager {
    default_pool: Arc<Pool>,
    default_dialect: Dialect,
    base_url: Option<String>,
    pools: RwLock<HashMap<String, (Arc<Pool>, Dialect)>>,
}

impl DatabaseManager {
    /// 用默认连接池创建管理器（单数据库模式）
    pub fn new(pool: Pool, dialect: Dialect) -> Self {
        Self {
            default_pool: Arc::new(pool),
            default_dialect: dialect,
            base_url: None,
            pools: RwLock::new(HashMap::new()),
        }
    }

    /// 启用多数据库模式，设置基础 URL 模板
    ///
    /// SQLite 示例: `sqlite:./data/`（拼接后为 `sqlite:./data/{name}.db`）
    /// PostgreSQL 示例: `postgres://user:pass@localhost/`（拼接后为 `postgres://user:pass@localhost/{name}`）
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// 是否启用了多数据库模式
    pub fn is_multi_db_enabled(&self) -> bool {
        self.base_url.is_some()
    }

    /// 获取指定数据库的连接池和方言
    ///
    /// 若 `db_name` 为 "main" 或空，返回默认连接池。
    /// 若连接池不存在且启用了多数据库模式，动态创建并缓存。
    pub async fn get_pool(&self, db_name: &str) -> Result<(Arc<Pool>, Dialect)> {
        if db_name.is_empty() || db_name == DEFAULT_DB_NAME {
            return Ok((self.default_pool.clone(), self.default_dialect));
        }

        {
            let pools = self.pools.read().await;
            if let Some((pool, dialect)) = pools.get(db_name) {
                return Ok((pool.clone(), *dialect));
            }
        }

        let base_url = match &self.base_url {
            Some(url) => url,
            None => return Ok((self.default_pool.clone(), self.default_dialect)),
        };

        let url = Self::build_database_url(base_url, db_name, &self.default_dialect);
        let (pool, dialect) = connect_pool_with_options(&url, 5, 1, 30, 600, 1800).await?;
        let pool = Arc::new(pool);

        {
            let mut pools = self.pools.write().await;
            pools.insert(db_name.to_string(), (pool.clone(), dialect));
        }

        Ok((pool, dialect))
    }

    /// 关闭并移除指定数据库的连接池
    pub async fn remove_pool(&self, db_name: &str) -> Result<()> {
        if db_name.is_empty() || db_name == DEFAULT_DB_NAME {
            anyhow::bail!("不能移除默认数据库连接池");
        }

        let mut pools = self.pools.write().await;
        if let Some((pool, _)) = pools.remove(db_name) {
            pool.close().await;
        }
        Ok(())
    }

    /// 返回所有当前激活的非默认数据库名称
    pub async fn list_databases(&self) -> Vec<String> {
        let pools = self.pools.read().await;
        pools.keys().cloned().collect()
    }

    /// 获取默认连接池
    pub fn default_pool(&self) -> &Arc<Pool> {
        &self.default_pool
    }

    /// 获取默认方言
    pub fn default_dialect(&self) -> &Dialect {
        &self.default_dialect
    }

    /// 获取基础 URL（多数据库模式）
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }

    /// 注册一个已有的连接池（用于创建数据库后将连接池加入管理）
    pub async fn register_pool(&self, db_name: &str, pool: Arc<Pool>, dialect: Dialect) {
        let mut pools = self.pools.write().await;
        pools.insert(db_name.to_string(), (pool, dialect));
    }

    /// 根据基础 URL 和数据库名构建完整连接 URL
    fn build_database_url(base_url: &str, db_name: &str, _default_dialect: &Dialect) -> String {
        let dialect = if base_url.starts_with("sqlite:") {
            Dialect::Sqlite
        } else {
            Dialect::Postgres
        };

        match dialect {
            Dialect::Sqlite => {
                let dir = base_url.trim_start_matches("sqlite:");
                let dir = dir.trim_end_matches('/');
                if dir.contains('?') {
                    let (path, params) = dir.split_once('?').unwrap();
                    let path = path.trim_end_matches('/');
                    format!("sqlite:{}/{}.db?{}", path, db_name, params)
                } else {
                    format!("sqlite:{}/{}.db", dir, db_name)
                }
            }
            Dialect::Postgres => {
                let base = base_url.trim_end_matches('/');
                format!("{}/{}", base, db_name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url_sqlite() {
        let url = DatabaseManager::build_database_url(
            "sqlite:./data/",
            "tenant_acme",
            &Dialect::Sqlite,
        );
        assert_eq!(url, "sqlite:./data/tenant_acme.db");
    }

    #[test]
    fn test_build_url_sqlite_with_params() {
        let url = DatabaseManager::build_database_url(
            "sqlite:./data/?mode=rwc",
            "tenant_acme",
            &Dialect::Sqlite,
        );
        assert_eq!(url, "sqlite:./data/tenant_acme.db?mode=rwc");
    }

    #[test]
    fn test_build_url_postgres() {
        let url = DatabaseManager::build_database_url(
            "postgres://user:pass@localhost/",
            "tenant_acme",
            &Dialect::Postgres,
        );
        assert_eq!(url, "postgres://user:pass@localhost/tenant_acme");
    }

    #[test]
    fn test_build_url_postgres_no_trailing_slash() {
        let url = DatabaseManager::build_database_url(
            "postgres://user:pass@localhost",
            "tenant_acme",
            &Dialect::Postgres,
        );
        assert_eq!(url, "postgres://user:pass@localhost/tenant_acme");
    }

    #[tokio::test]
    async fn test_is_multi_db_disabled_by_default() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite);
        assert!(!mgr.is_multi_db_enabled());
    }

    #[tokio::test]
    async fn test_is_multi_db_enabled_with_base_url() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite).with_base_url("sqlite:./data/");
        assert!(mgr.is_multi_db_enabled());
    }

    #[tokio::test]
    async fn test_get_pool_returns_default_for_main() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite);
        let (p, d) = mgr.get_pool("main").await.unwrap();
        assert_eq!(d, Dialect::Sqlite);
        assert!(Arc::ptr_eq(&p, mgr.default_pool()));
    }

    #[tokio::test]
    async fn test_get_pool_returns_default_for_empty() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite);
        let (p, _) = mgr.get_pool("").await.unwrap();
        assert!(Arc::ptr_eq(&p, mgr.default_pool()));
    }

    #[tokio::test]
    async fn test_get_pool_returns_default_when_multi_db_disabled() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite);
        let (p, _) = mgr.get_pool("other_db").await.unwrap();
        assert!(Arc::ptr_eq(&p, mgr.default_pool()));
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite).with_base_url("sqlite:./data/");

        let tenant_pool = Arc::new(Pool::connect_lazy("sqlite::memory:").unwrap());
        mgr.register_pool("tenant_a", tenant_pool, Dialect::Sqlite).await;

        let dbs = mgr.list_databases().await;
        assert_eq!(dbs, vec!["tenant_a"]);
    }

    #[tokio::test]
    async fn test_remove_pool() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite).with_base_url("sqlite:./data/");

        let tenant_pool = Arc::new(Pool::connect_lazy("sqlite::memory:").unwrap());
        mgr.register_pool("tenant_b", tenant_pool, Dialect::Sqlite).await;

        mgr.remove_pool("tenant_b").await.unwrap();
        let dbs = mgr.list_databases().await;
        assert!(dbs.is_empty());
    }

    #[tokio::test]
    async fn test_remove_default_pool_fails() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite);
        assert!(mgr.remove_pool("main").await.is_err());
    }
}
