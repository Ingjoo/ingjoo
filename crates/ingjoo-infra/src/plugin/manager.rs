use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Context, Result};
use ingjoo_core::module::plugin::{PluginInfo, PluginManifest, PluginState};
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use ingjoo_core::ModelRegistry;

use crate::db::generic::GenericDb;
use crate::db::generic::GenericRecordStore;
use crate::db::seed::{seed_plugin_metadata, seed_plugin_records};

struct LoadedPlugin {
    manifest: PluginManifest,
    source_path: PathBuf,
}

pub struct PluginManager {
    plugins: RwLock<HashMap<String, LoadedPlugin>>,
    registry: Arc<ModelRegistry>,
    pool: Arc<Pool>,
    dialect: Dialect,
}

impl PluginManager {
    pub fn new(registry: Arc<ModelRegistry>, pool: Arc<Pool>, dialect: Dialect) -> Self {
        Self { plugins: RwLock::new(HashMap::new()), registry, pool, dialect }
    }

    pub async fn load_from_dir(&self, dir: &Path) -> Result<Vec<String>> {
        if !dir.is_dir() {
            return Err(anyhow!("插件目录不存在: {}", dir.display()));
        }

        let mut entries =
            tokio::fs::read_dir(dir).await.with_context(|| format!("无法读取插件目录: {}", dir.display()))?;

        let mut loaded = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match self.load_plugin(&path).await {
                    Ok(name) => loaded.push(name),
                    Err(e) => {
                        tracing::warn!("加载插件失败 {}: {}", path.display(), e);
                    }
                }
            }
        }
        Ok(loaded)
    }

    pub async fn load_plugin(&self, path: &Path) -> Result<String> {
        let manifest =
            PluginManifest::from_file(path).await.with_context(|| format!("解析插件清单失败: {}", path.display()))?;

        let name = manifest.name.clone();

        let db = GenericDb::new(&self.pool, &self.dialect);
        for model in &manifest.models {
            db.ensure_table(model).await.with_context(|| format!("创建表失败: {}", model.table_name))?;
            self.registry.register(model.clone());
        }

        if let Err(e) =
            seed_plugin_metadata(&self.pool, &self.dialect, &manifest.menus, &manifest.views, &manifest.actions).await
        {
            tracing::warn!("插件 {} 元数据播种失败: {}", name, e);
        }

        if let Some(records) = &manifest.records {
            if let Err(e) = seed_plugin_records(&self.pool, &self.dialect, records).await {
                tracing::warn!("插件 {} 记录播种失败: {}", name, e);
            }
        }

        let loaded = LoadedPlugin { manifest, source_path: path.to_path_buf() };

        let mut plugins = self.plugins.write().map_err(|e| anyhow!("获取写锁失败: {}", e))?;
        plugins.insert(name.clone(), loaded);

        Ok(name)
    }

    pub fn unload_plugin(&self, name: &str) -> Result<()> {
        let mut plugins = self.plugins.write().map_err(|e| anyhow!("获取写锁失败: {}", e))?;

        let loaded = plugins.remove(name).ok_or_else(|| anyhow!("插件不存在: {}", name))?;

        for model in &loaded.manifest.models {
            self.registry.unregister(&model.name);
        }

        Ok(())
    }

    pub async fn reload_plugin(&self, name: &str) -> Result<()> {
        let source_path = {
            let plugins = self.plugins.read().map_err(|e| anyhow!("获取读锁失败: {}", e))?;
            plugins.get(name).ok_or_else(|| anyhow!("插件不存在: {}", name))?.source_path.clone()
        };

        self.unload_plugin(name)?;
        self.load_plugin(&source_path).await?;
        Ok(())
    }

    pub async fn reload_all(&self, dir: &Path) -> Result<Vec<String>> {
        let names: Vec<String> = {
            let plugins = self.plugins.read().map_err(|e| anyhow!("获取读锁失败: {}", e))?;
            plugins.keys().cloned().collect()
        };
        for name in &names {
            self.unload_plugin(name)?;
        }
        self.load_from_dir(dir).await
    }

    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        let plugins = self.plugins.read().unwrap_or_else(|e| e.into_inner());
        plugins.values().map(|lp| lp.manifest.to_info(PluginState::Active)).collect()
    }

    pub fn get_plugin(&self, name: &str) -> Option<PluginInfo> {
        let plugins = self.plugins.read().unwrap_or_else(|e| e.into_inner());
        plugins.get(name).map(|lp| lp.manifest.to_info(PluginState::Active))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> Arc<ModelRegistry> {
        Arc::new(ModelRegistry::new())
    }

    async fn test_pool_and_dialect() -> (Arc<Pool>, Dialect) {
        ingjoo_core::pool::install_drivers();
        let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let url = format!("sqlite:file:test_{id}?mode=memory&cache=shared");
        let (pool, dialect) = ingjoo_core::pool::connect_pool(&url).await.unwrap();
        (Arc::new(pool), dialect)
    }

    async fn ensure_ir_tables(pool: &Pool) {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ir_menu (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, parent_id TEXT,
                sequence INTEGER NOT NULL DEFAULT 10, action_id TEXT, web_icon TEXT,
                active INTEGER NOT NULL DEFAULT 1, group_ids TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')), updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
        "#,
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ir_view (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, model TEXT NOT NULL,
                type TEXT NOT NULL DEFAULT 'form', priority INTEGER NOT NULL DEFAULT 16,
                arch TEXT NOT NULL DEFAULT '{}', inherit_id TEXT,
                active INTEGER NOT NULL DEFAULT 1, group_ids TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')), updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
        "#,
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ir_action (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, type TEXT NOT NULL DEFAULT 'act_window',
                res_model TEXT, view_mode TEXT NOT NULL DEFAULT 'list,form', view_ids TEXT NOT NULL DEFAULT '[]',
                domain TEXT, context TEXT, page_limit INTEGER DEFAULT 80,
                target TEXT NOT NULL DEFAULT 'current', search_view_id TEXT, url TEXT, help TEXT,
                group_ids TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')), updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
        "#,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_load_manifest_from_json() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{
            "name": "blog",
            "version": "1.0.0",
            "description": "博客插件",
            "models": [
                {
                    "name": "article",
                    "table_name": "articles",
                    "fields": [
                        {"name": "title", "field_type": "text", "required": true}
                    ]
                }
            ]
        }"#;
        let path = dir.path().join("blog.json");
        tokio::fs::write(&path, json).await.unwrap();

        let registry = test_registry();
        let (pool, dialect) = test_pool_and_dialect().await;
        let manager = PluginManager::new(registry.clone(), pool, dialect);

        let name = manager.load_plugin(&path).await.unwrap();
        assert_eq!(name, "blog");
        assert!(registry.get("article").is_some());
    }

    #[tokio::test]
    async fn test_unload_removes_models() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{
            "name": "shop",
            "version": "1.0.0",
            "models": [
                {"name": "product", "table_name": "products", "fields": []}
            ]
        }"#;
        let path = dir.path().join("shop.json");
        tokio::fs::write(&path, json).await.unwrap();

        let registry = test_registry();
        let (pool, dialect) = test_pool_and_dialect().await;
        let manager = PluginManager::new(registry.clone(), pool, dialect);

        manager.load_plugin(&path).await.unwrap();
        assert!(registry.get("product").is_some());

        manager.unload_plugin("shop").unwrap();
        assert!(registry.get("product").is_none());
    }

    #[tokio::test]
    async fn test_list_plugins() {
        let dir = tempfile::tempdir().unwrap();

        let json_a = r#"{"name":"alpha","version":"1.0.0","models":[{"name":"alpha_model","table_name":"alpha_models","fields":[]}]}"#;
        let json_b = r#"{"name":"beta","version":"2.0.0","models":[]}"#;
        tokio::fs::write(dir.path().join("alpha.json"), json_a).await.unwrap();
        tokio::fs::write(dir.path().join("beta.json"), json_b).await.unwrap();

        let registry = test_registry();
        let (pool, dialect) = test_pool_and_dialect().await;
        let manager = PluginManager::new(registry.clone(), pool, dialect);

        let loaded = manager.load_from_dir(dir.path()).await.unwrap();
        assert_eq!(loaded.len(), 2);

        let list = manager.list_plugins();
        assert_eq!(list.len(), 2);

        let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[tokio::test]
    async fn test_load_plugin_seeds_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{
            "name": "blog",
            "version": "1.0.0",
            "models": [
                {"name": "article", "table_name": "articles", "fields": [
                    {"name": "title", "field_type": "text"}
                ]}
            ],
            "menus": [
                {"id": "menu_blog", "name": "博客", "web_icon": "fa fa-book", "action_id": "action_articles"}
            ],
            "views": [
                {"id": "view_article_list", "name": "文章列表", "model": "article", "type": "list", "arch": {"columns": [{"name": "title"}]}}
            ],
            "actions": [
                {"id": "action_articles", "name": "文章管理", "type": "act_window", "res_model": "article", "view_mode": ["list", "form"]}
            ]
        }"#;
        let path = dir.path().join("blog.json");
        tokio::fs::write(&path, json).await.unwrap();

        let registry = test_registry();
        let (pool, dialect) = test_pool_and_dialect().await;
        ensure_ir_tables(&pool).await;

        let manager = PluginManager::new(registry.clone(), pool.clone(), dialect);
        manager.load_plugin(&path).await.unwrap();

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM ir_menu WHERE id = 'menu_blog'").fetch_one(&*pool).await.unwrap();
        assert_eq!(count, 1);

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ir_view WHERE id = 'view_article_list'")
            .fetch_one(&*pool)
            .await
            .unwrap();
        assert_eq!(count, 1);

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ir_action WHERE id = 'action_articles'")
            .fetch_one(&*pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_load_plugin_seeds_records() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{
            "name": "blog",
            "version": "1.0.0",
            "models": [
                {"name": "article", "table_name": "articles", "fields": [
                    {"name": "title", "field_type": "text"},
                    {"name": "status", "field_type": "text"}
                ]}
            ],
            "records": {
                "articles": [
                    {"title": "Hello World", "status": "published"},
                    {"title": "Draft Post", "status": "draft"}
                ]
            }
        }"#;
        let path = dir.path().join("blog.json");
        tokio::fs::write(&path, json).await.unwrap();

        let registry = test_registry();
        let (pool, dialect) = test_pool_and_dialect().await;
        ensure_ir_tables(&pool).await;

        let manager = PluginManager::new(registry.clone(), pool.clone(), dialect);
        manager.load_plugin(&path).await.unwrap();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM articles").fetch_one(&*pool).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_seed_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{
            "name": "blog",
            "version": "1.0.0",
            "models": [
                {"name": "tag", "table_name": "tags", "fields": [
                    {"name": "name", "field_type": "text"}
                ]}
            ],
            "menus": [
                {"id": "menu_tags", "name": "标签"}
            ],
            "records": {
                "tags": [{"name": "Rust"}]
            }
        }"#;
        let path = dir.path().join("blog.json");
        tokio::fs::write(&path, json).await.unwrap();

        let registry = test_registry();
        let (pool, dialect) = test_pool_and_dialect().await;
        ensure_ir_tables(&pool).await;

        let manager = PluginManager::new(registry.clone(), pool.clone(), dialect);
        manager.load_plugin(&path).await.unwrap();
        manager.unload_plugin("blog").unwrap();
        manager.load_plugin(&path).await.unwrap();

        let (menu_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM ir_menu WHERE id = 'menu_tags'").fetch_one(&*pool).await.unwrap();
        assert_eq!(menu_count, 1);
    }
}
