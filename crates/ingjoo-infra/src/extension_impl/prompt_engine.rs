//! 提示词模板引擎 — 基于 ModuleSettingStore 的模板持久化与渲染

use std::collections::HashMap;
use std::sync::Arc;

use ingjoo_core::db::error::StoreResult;
use ingjoo_core::db::traits::ModuleSettingStore;
use ingjoo_core::extension::prompt::PromptManager;

/// 基于 ModuleSettingStore 的提示词模板管理器
///
/// 模板以 `module="prompt"`, `key=<template_key>` 存储在 module_settings 表中。
/// 支持存储、检索、列表和渲染操作。
/// 基于 module_settings 的提示词管理器
pub struct SettingsPromptManager {
    inner: PromptManager,
    store: Arc<dyn ModuleSettingStore>,
    scope: String,
    scope_id: Option<String>,
}

impl SettingsPromptManager {
    pub fn new(
        store: Arc<dyn ModuleSettingStore>,
        scope: impl Into<String>,
        scope_id: Option<String>,
    ) -> Self {
        Self {
            inner: PromptManager::new(),
            store,
            scope: scope.into(),
            scope_id,
        }
    }

    /// 获取模板内容
    pub async fn get_template(&self, key: &str) -> StoreResult<Option<String>> {
        self.store
            .get_module_setting(&self.scope, self.scope_id.as_deref(), "prompt", key)
            .await
    }

    /// 保存模板
    pub async fn save_template(
        &self,
        key: &str,
        template: &str,
    ) -> StoreResult<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.store
            .set_module_setting(
                &id,
                &self.scope,
                self.scope_id.as_deref(),
                "prompt",
                key,
                template,
            )
            .await?;
        Ok(())
    }

    /// 渲染模板 — 将 `{{variable}}` 占位符替换为实际值
    pub async fn render_template(
        &self,
        key: &str,
        vars: &HashMap<String, String>,
    ) -> Result<String, anyhow::Error> {
        let template = self
            .get_template(key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("模板不存在: {}", key))?;
        Ok(self.inner.render(&template, vars)?)
    }

    /// 使用原始模板字符串直接渲染（不经过存储）
    pub fn render_raw(
        &self,
        template: &str,
        vars: &HashMap<String, String>,
    ) -> Result<String, anyhow::Error> {
        self.inner.render(template, vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ingjoo_core::db::models::ModuleSetting;

    struct MockSettingsStore {
        settings: tokio::sync::Mutex<HashMap<String, String>>,
    }

    impl MockSettingsStore {
        fn new() -> Self {
            Self {
                settings: tokio::sync::Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl ModuleSettingStore for MockSettingsStore {
        async fn list_module_settings(
            &self,
            _scope: &str,
            _scope_id: Option<&str>,
            _module: Option<&str>,
        ) -> StoreResult<Vec<ModuleSetting>> {
            Ok(vec![])
        }
        async fn get_module_setting(
            &self,
            scope: &str,
            scope_id: Option<&str>,
            module: &str,
            key: &str,
        ) -> StoreResult<Option<String>> {
            let map = self.settings.lock().await;
            let store_key = format!("{}:{}:{}:{}", scope, scope_id.unwrap_or(""), module, key);
            Ok(map.get(&store_key).cloned())
        }
        async fn set_module_setting(
            &self,
            _id: &str,
            scope: &str,
            scope_id: Option<&str>,
            module: &str,
            key: &str,
            value: &str,
        ) -> StoreResult<ModuleSetting> {
            let mut map = self.settings.lock().await;
            let store_key = format!("{}:{}:{}:{}", scope, scope_id.unwrap_or(""), module, key);
            map.insert(store_key, value.to_string());
            Ok(ModuleSetting {
                id: _id.to_string(),
                scope: scope.to_string(),
                scope_id: scope_id.map(String::from),
                module: module.to_string(),
                key: key.to_string(),
                value: value.to_string(),
                updated_at: String::new(),
            })
        }
        async fn delete_module_setting(&self, _id: &str) -> StoreResult<bool> {
            Ok(false)
        }
        async fn get_effective_setting(
            &self,
            _module: &str,
            _key: &str,
            _collection_id: Option<&str>,
        ) -> StoreResult<Option<String>> {
            Ok(None)
        }
    }

    fn make_store() -> Arc<dyn ModuleSettingStore> {
        Arc::new(MockSettingsStore::new())
    }

    #[tokio::test]
    async fn test_save_and_get_template() {
        let store = make_store();
        let mgr = SettingsPromptManager::new(store, "system", None);
        mgr.save_template("greeting", "你好，{{name}}！")
            .await
            .unwrap();
        let tmpl = mgr.get_template("greeting").await.unwrap();
        assert_eq!(tmpl, Some("你好，{{name}}！".to_string()));
    }

    #[tokio::test]
    async fn test_render_template() {
        let store = make_store();
        let mgr = SettingsPromptManager::new(store, "system", None);
        mgr.save_template("hello", "Hello {{name}}, welcome to {{place}}!")
            .await
            .unwrap();
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        vars.insert("place".to_string(), "Wonderland".to_string());
        let result = mgr.render_template("hello", &vars).await.unwrap();
        assert_eq!(result, "Hello Alice, welcome to Wonderland!");
    }

    #[tokio::test]
    async fn test_render_nonexistent_template_fails() {
        let store = make_store();
        let mgr = SettingsPromptManager::new(store, "system", None);
        let result = mgr.render_template("nonexistent", &HashMap::new()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("模板不存在"));
    }

    #[test]
    fn test_render_raw() {
        let store = make_store();
        let mgr = SettingsPromptManager::new(store, "system", None);
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), "42".to_string());
        let result = mgr.render_raw("value={{x}}", &vars).unwrap();
        assert_eq!(result, "value=42");
    }
}
