use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use ingjoo_core::pool::{connect_pool_with_options, Pool};
use ingjoo_core::Dialect;

#[cfg(feature = "db")]
use sqlx::Row;

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

    /// 返回所有已存在的非默认数据库名称
    ///
    /// 从文件系统（SQLite）或 pg_catalog（PostgreSQL）发现数据库，
    /// 不仅仅是内存中已注册的连接池。
    pub async fn list_databases(&self) -> Vec<String> {
        match &self.base_url {
            None => {
                // 单数据库模式，无额外数据库
                Vec::new()
            }
            Some(base_url) => {
                if base_url.starts_with("sqlite:") {
                    self.list_sqlite_databases(base_url)
                } else {
                    // PostgreSQL: 查询 pg_database
                    self.list_postgres_databases().await
                }
            }
        }
    }

    /// 扫描 SQLite 目录发现 .db 文件
    fn list_sqlite_databases(&self, base_url: &str) -> Vec<String> {
        let dir = base_url.trim_start_matches("sqlite:");
        let dir = dir.trim_end_matches('/');
        // 如果有查询参数，去掉它们
        let dir = dir.split('?').next().unwrap_or(dir);

        let path = std::path::Path::new(dir);
        if !path.exists() {
            return Vec::new();
        }

        let mut names = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if let Some(ext) = entry_path.extension() {
                    if ext == "db" {
                        if let Some(name) = entry_path.file_stem() {
                            let name = name.to_string_lossy().to_string();
                            if name != "main" && name != "ingjoo" {
                                names.push(name);
                            }
                        }
                    }
                }
            }
        }

        names.sort();
        names
    }

    /// 查询 PostgreSQL pg_catalog 发现数据库
    #[cfg(feature = "db")]
    async fn list_postgres_databases(&self) -> Vec<String> {
        let result = sqlx::query(
            "SELECT datname FROM pg_database WHERE datistemplate = false AND datname != current_database()",
        )
        .fetch_all(&*self.default_pool)
        .await;

        match result {
            Ok(rows) => rows
                .iter()
                .filter_map(|row| row.try_get::<String, _>("datname").ok())
                .filter(|name| name != "main" && name != "postgres")
                .collect(),
            Err(_) => {
                let pools = self.pools.read().await;
                pools.keys().cloned().collect()
            }
        }
    }

    /// PostgreSQL 发现不可用，回退到内存池列表
    #[cfg(not(feature = "db"))]
    async fn list_postgres_databases(&self) -> Vec<String> {
        let pools = self.pools.read().await;
        pools.keys().cloned().collect()
    }

    /// 启动时发现并注册所有已存在的数据库连接池
    ///
    /// 仅在多数据库模式下工作。扫描文件系统/目录获取数据库列表，
    /// 并为每个数据库预创建连接池。
    pub async fn discover_and_register(&self) -> Vec<String> {
        let db_names = self.list_databases().await;
        for name in &db_names {
            // get_pool 会自动创建并缓存连接池
            if let Err(e) = self.get_pool(name).await {
                tracing::warn!("启动时注册数据库 '{}' 连接池失败: {}", name, e);
            }
        }
        db_names
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
        let dialect = if base_url.starts_with("sqlite:") { Dialect::Sqlite } else { Dialect::Postgres };

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

    /// 创建新数据库（物理层面）
    ///
    /// - SQLite: 连接时自动创建文件（mode=rwc），然后返回连接池
    /// - PostgreSQL: 通过默认连接池执行 `CREATE DATABASE`，然后创建新连接池
    pub async fn create_database(&self, db_name: &str) -> Result<(Arc<Pool>, Dialect)> {
        if db_name.is_empty() || db_name == DEFAULT_DB_NAME {
            anyhow::bail!("不能创建默认数据库");
        }

        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("多数据库模式未启用"))?;

        let is_sqlite = base_url.starts_with("sqlite:");

        if !is_sqlite {
            // PostgreSQL: 先在默认库上执行 CREATE DATABASE
            let escaped = db_name.replace('\'', "''");
            sqlx::query(&format!("CREATE DATABASE {}", escaped))
                .execute(&*self.default_pool)
                .await
                .map_err(|e| anyhow::anyhow!("创建 PostgreSQL 数据库 '{}' 失败: {}", db_name, e))?;
        }

        // 连接到新数据库（SQLite 自动创建文件）
        let url = Self::build_database_url(base_url, db_name, &self.default_dialect);
        let (pool, dialect) = connect_pool_with_options(&url, 5, 1, 30, 600, 1800).await.map_err(|e| {
            // PostgreSQL 创建成功但连接失败 → 尝试清理
            if !is_sqlite {
                let escaped = db_name.replace('\'', "''");
                let default_pool = self.default_pool.clone();
                let cleanup_name = db_name.to_string();
                tokio::spawn(async move {
                    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {}", escaped.replace('\'', "''")))
                        .execute(&*default_pool)
                        .await;
                    tracing::warn!("创建数据库 '{}' 连接失败，已尝试清理", cleanup_name);
                });
            }
            anyhow::anyhow!("连接新数据库 '{}' 失败: {}", db_name, e)
        })?;

        let pool = Arc::new(pool);
        {
            let mut pools = self.pools.write().await;
            pools.insert(db_name.to_string(), (pool.clone(), dialect));
        }

        Ok((pool, dialect))
    }

    /// 删除数据库（物理层面）
    ///
    /// - SQLite: 关闭连接池 + 删除文件
    /// - PostgreSQL: 关闭连接池 + 执行 `DROP DATABASE`
    pub async fn drop_database(&self, db_name: &str) -> Result<()> {
        if db_name.is_empty() || db_name == DEFAULT_DB_NAME {
            anyhow::bail!("不能删除默认数据库");
        }

        let base_url = self.base_url.as_ref();

        // 先关闭连接池
        self.remove_pool(db_name).await?;

        // 物理删除
        if let Some(base_url) = base_url {
            if base_url.starts_with("sqlite:") {
                let dir = base_url.trim_start_matches("sqlite:").trim_end_matches('/').split('?').next().unwrap_or(".");
                let file_path = format!("{}/{}.db", dir, db_name);
                if std::path::Path::new(&file_path).exists() {
                    std::fs::remove_file(&file_path)
                        .map_err(|e| anyhow::anyhow!("删除 SQLite 文件失败: {}", e))?;
                }
            } else {
                // PostgreSQL: DROP DATABASE
                let escaped = db_name.replace('\'', "''");
                sqlx::query(&format!("DROP DATABASE IF EXISTS {}", escaped))
                    .execute(&*self.default_pool)
                    .await
                    .map_err(|e| anyhow::anyhow!("删除 PostgreSQL 数据库 '{}' 失败: {}", db_name, e))?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url_sqlite() {
        let url = DatabaseManager::build_database_url("sqlite:./data/", "tenant_acme", &Dialect::Sqlite);
        assert_eq!(url, "sqlite:./data/tenant_acme.db");
    }

    #[test]
    fn test_build_url_sqlite_with_params() {
        let url = DatabaseManager::build_database_url("sqlite:./data/?mode=rwc", "tenant_acme", &Dialect::Sqlite);
        assert_eq!(url, "sqlite:./data/tenant_acme.db?mode=rwc");
    }

    #[test]
    fn test_build_url_postgres() {
        let url =
            DatabaseManager::build_database_url("postgres://user:pass@localhost/", "tenant_acme", &Dialect::Postgres);
        assert_eq!(url, "postgres://user:pass@localhost/tenant_acme");
    }

    #[test]
    fn test_build_url_postgres_no_trailing_slash() {
        let url =
            DatabaseManager::build_database_url("postgres://user:pass@localhost", "tenant_acme", &Dialect::Postgres);
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
    async fn test_list_sqlite_databases_with_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("tenant_test.db");
        std::fs::File::create(&db_path).unwrap();

        let base_url = format!("sqlite:{}/", temp_dir.path().display());
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite).with_base_url(&base_url);

        let dbs = mgr.list_databases().await;
        assert_eq!(dbs, vec!["tenant_test"]);
    }

    #[tokio::test]
    async fn test_list_sqlite_databases_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_url = format!("sqlite:{}/", temp_dir.path().display());
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite).with_base_url(&base_url);

        let dbs = mgr.list_databases().await;
        assert!(dbs.is_empty());
    }

    #[tokio::test]
    async fn test_list_sqlite_ignores_main_and_ingjoo() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::File::create(temp_dir.path().join("main.db")).unwrap();
        std::fs::File::create(temp_dir.path().join("ingjoo.db")).unwrap();
        std::fs::File::create(temp_dir.path().join("tenant_a.db")).unwrap();

        let base_url = format!("sqlite:{}/", temp_dir.path().display());
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite).with_base_url(&base_url);

        let dbs = mgr.list_databases().await;
        assert_eq!(dbs, vec!["tenant_a"]);
    }

    #[tokio::test]
    async fn test_list_databases_returns_empty_in_single_db_mode() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite);
        assert!(mgr.list_databases().await.is_empty());
    }

    #[tokio::test]
    async fn test_list_databases_nonexistent_dir() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite).with_base_url("sqlite:/nonexistent/path/");
        assert!(mgr.list_databases().await.is_empty());
    }

    #[tokio::test]
    async fn test_remove_pool() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_url = format!("sqlite:{}/", temp_dir.path().display());
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite).with_base_url(&base_url);

        let tenant_pool = Arc::new(Pool::connect_lazy("sqlite::memory:").unwrap());
        mgr.register_pool("tenant_b", tenant_pool, Dialect::Sqlite).await;

        mgr.remove_pool("tenant_b").await.unwrap();
    }

    #[tokio::test]
    async fn test_remove_default_pool_fails() {
        let pool = Pool::connect_lazy("sqlite::memory:").unwrap();
        let mgr = DatabaseManager::new(pool, Dialect::Sqlite);
        assert!(mgr.remove_pool("main").await.is_err());
    }
}
