use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Context, Result};
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use ingjoo_core::ModelRegistry;
use ingjoo_core::module::plugin::{PluginInfo, PluginManifest, PluginState};

use crate::db::generic::GenericRecordStore;
use crate::db::generic::GenericDb;

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
        Self {
            plugins: RwLock::new(HashMap::new()),
            registry,
            pool,
            dialect,
        }
    }

    pub async fn load_from_dir(&self, dir: &Path) -> Result<Vec<String>> {
        if !dir.is_dir() {
            return Err(anyhow!("插件目录不存在: {}", dir.display()));
        }

        let mut entries = tokio::fs::read_dir(dir)
            .await
            .with_context(|| format!("无法读取插件目录: {}", dir.display()))?;

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
        let manifest = PluginManifest::from_file(path)
            .await
            .with_context(|| format!("解析插件清单失败: {}", path.display()))?;

        let name = manifest.name.clone();

        let db = GenericDb::new(&*self.pool, &self.dialect);
        for model in &manifest.models {
            db.ensure_table(model)
                .await
                .with_context(|| format!("创建表失败: {}", model.table_name))?;
            self.registry.register(model.clone());
        }

        let loaded = LoadedPlugin {
            manifest,
            source_path: path.to_path_buf(),
        };

        let mut plugins = self.plugins.write().map_err(|e| anyhow!("获取写锁失败: {}", e))?;
        plugins.insert(name.clone(), loaded);

        Ok(name)
    }

    pub fn unload_plugin(&self, name: &str) -> Result<()> {
        let mut plugins = self.plugins.write().map_err(|e| anyhow!("获取写锁失败: {}", e))?;

        let loaded = plugins
            .remove(name)
            .ok_or_else(|| anyhow!("插件不存在: {}", name))?;

        for model in &loaded.manifest.models {
            self.registry.unregister(&model.name);
        }

        Ok(())
    }

    pub async fn reload_plugin(&self, name: &str) -> Result<()> {
        let source_path = {
            let plugins = self.plugins.read().map_err(|e| anyhow!("获取读锁失败: {}", e))?;
            plugins
                .get(name)
                .ok_or_else(|| anyhow!("插件不存在: {}", name))?
                .source_path
                .clone()
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
        plugins
            .values()
            .map(|lp| lp.manifest.to_info(PluginState::Active))
            .collect()
    }

    pub fn get_plugin(&self, name: &str) -> Option<PluginInfo> {
        let plugins = self.plugins.read().unwrap_or_else(|e| e.into_inner());
        plugins.get(name).map(|lp| lp.manifest.to_info(PluginState::Active))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingjoo_core::module::ModelDescriptor;

    fn test_registry() -> Arc<ModelRegistry> {
        Arc::new(ModelRegistry::new())
    }

    async fn test_pool_and_dialect() -> (Arc<Pool>, Dialect) {
        ingjoo_core::pool::install_drivers();
        let (pool, dialect) = ingjoo_core::pool::connect_pool("sqlite::memory:").await.unwrap();
        (Arc::new(pool), dialect)
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
}
