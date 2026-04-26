//! 配置级联查询 — 按优先级依次查找，返回首个非空值

use crate::db::error::StoreResult;
use crate::db::traits::ModuleSettingStore;

/// 配置级联工具 — 按 scope 优先级查找生效的配置值
pub struct CascadeConfig;

impl CascadeConfig {
    /// 按 scopes 顺序逐级查找配置，返回第一个非空值
    ///
    /// 典型用法：`[("document_collection", Some("col1")), ("system", None)]`
    /// 表示先查集合级配置，找不到则回退到系统默认。
    pub async fn get_effective<S: ModuleSettingStore + ?Sized>(
        store: &S,
        module: &str,
        key: &str,
        scopes: &[(&str, Option<&str>)],
    ) -> StoreResult<Option<String>> {
        for (scope, scope_id) in scopes {
            let val = store.get_module_setting(scope, *scope_id, module, key).await?;
            if val.is_some() {
                return Ok(val);
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::error::StoreResult;
    use crate::db::models::ModuleSetting;
    use async_trait::async_trait;

    struct MockSettings {
        data: std::collections::HashMap<(String, Option<String>, String, String), String>,
    }

    impl MockSettings {
        fn new() -> Self {
            Self { data: std::collections::HashMap::new() }
        }
        fn set(&mut self, scope: &str, scope_id: Option<&str>, module: &str, key: &str, value: &str) {
            self.data.insert(
                (scope.to_string(), scope_id.map(String::from), module.to_string(), key.to_string()),
                value.to_string(),
            );
        }
    }

    #[async_trait]
    impl ModuleSettingStore for MockSettings {
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
            let k = (scope.to_string(), scope_id.map(String::from), module.to_string(), key.to_string());
            Ok(self.data.get(&k).cloned())
        }
        async fn set_module_setting(
            &self,
            _id: &str,
            _scope: &str,
            _scope_id: Option<&str>,
            _module: &str,
            _key: &str,
            _value: &str,
        ) -> StoreResult<ModuleSetting> {
            unimplemented!()
        }
        async fn delete_module_setting(&self, _id: &str) -> StoreResult<bool> {
            unimplemented!()
        }
        async fn get_effective_setting(
            &self,
            module: &str,
            key: &str,
            collection_id: Option<&str>,
        ) -> StoreResult<Option<String>> {
            let scopes: Vec<(&str, Option<&str>)> = if let Some(cid) = collection_id {
                vec![("document_collection", Some(cid)), ("system", None)]
            } else {
                vec![("system", None)]
            };
            CascadeConfig::get_effective(self, module, key, &scopes).await
        }
    }

    #[tokio::test]
    async fn cascade_collection_overrides_system() {
        let mut store = MockSettings::new();
        store.set("system", None, "wiki", "theme", "light");
        store.set("document_collection", Some("col1"), "wiki", "theme", "dark");

        let val = store.get_effective_setting("wiki", "theme", Some("col1")).await.unwrap();
        assert_eq!(val, Some("dark".to_string()));
    }

    #[tokio::test]
    async fn cascade_fallback_to_system() {
        let mut store = MockSettings::new();
        store.set("system", None, "wiki", "theme", "light");

        let val = store.get_effective_setting("wiki", "theme", Some("col1")).await.unwrap();
        assert_eq!(val, Some("light".to_string()));
    }

    #[tokio::test]
    async fn cascade_no_value() {
        let store = MockSettings::new();
        let val = store.get_effective_setting("wiki", "theme", None).await.unwrap();
        assert_eq!(val, None);
    }
}
