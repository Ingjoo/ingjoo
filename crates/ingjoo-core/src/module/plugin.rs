//! 插件清单 — 定义插件声明式注册所需的全部元数据

use crate::module::{ActionDescriptor, MenuDescriptor, ModelDescriptor, ViewDescriptor};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 插件状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// 已加载，正常运行
    Active,
    /// 已加载，但被停用
    Disabled,
    /// 加载失败
    Error,
}

/// 插件清单 — 从 JSON 文件加载的插件定义
///
/// 每个插件通过一个 JSON 文件声明其包含的模型、菜单、视图和动作。
/// 放置在 `plugins/` 目录下，由 PluginManager 在运行时加载。
///
/// # 示例 JSON
/// ```json
/// {
///   "name": "blog",
///   "version": "1.0.0",
///   "description": "博客模块",
///   "models": [
///     {
///       "name": "article",
///       "table_name": "articles",
///       "fields": [
///         {"name": "title", "field_type": "text", "required": true},
///         {"name": "body", "field_type": "text"}
///       ]
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// 插件唯一名称（如 "blog"、"crm"）
    pub name: String,
    /// 语义版本号（如 "1.0.0"）
    pub version: String,
    /// 插件描述
    #[serde(default)]
    pub description: String,
    /// 插件提供的动态模型
    #[serde(default)]
    pub models: Vec<ModelDescriptor>,
    /// 插件提供的菜单项
    #[serde(default)]
    pub menus: Vec<MenuDescriptor>,
    /// 插件提供的视图
    #[serde(default)]
    pub views: Vec<ViewDescriptor>,
    /// 插件提供的动作
    #[serde(default)]
    pub actions: Vec<ActionDescriptor>,
    /// 初始种子记录 — 模型名 → 记录列表
    ///
    /// 加载插件时自动插入，使用 INSERT OR IGNORE / ON CONFLICT DO NOTHING 保证幂等。
    #[serde(default)]
    pub records: Option<HashMap<String, Vec<serde_json::Value>>>,
}

/// 已加载插件的运行时信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// 插件名称
    pub name: String,
    /// 版本
    pub version: String,
    /// 描述
    pub description: String,
    /// 当前状态
    pub state: PluginState,
    /// 提供的模型数量
    pub model_count: usize,
    /// 提供的菜单数量
    pub menu_count: usize,
    /// 提供的视图数量
    pub view_count: usize,
    /// 提供的动作数量
    pub action_count: usize,
    /// 提供的种子记录模型数量
    pub record_model_count: usize,
}

impl PluginManifest {
    /// 从 JSON 字符串解析
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 序列化为 JSON 字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// 从文件路径加载
    pub async fn from_file(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let content = tokio::fs::read_to_string(path).await?;
        serde_json::from_str(&content).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// 转换为运行时信息
    pub fn to_info(&self, state: PluginState) -> PluginInfo {
        PluginInfo {
            name: self.name.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            state,
            model_count: self.models.len(),
            menu_count: self.menus.len(),
            view_count: self.views.len(),
            action_count: self.actions.len(),
            record_model_count: self.records.as_ref().map(|r| r.len()).unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_from_json() {
        let json = r#"{
            "name": "blog",
            "version": "1.0.0",
            "description": "博客模块",
            "models": [
                {
                    "name": "article",
                    "table_name": "articles",
                    "fields": [
                        {"name": "title", "field_type": "text", "required": true},
                        {"name": "body", "field_type": "text"}
                    ]
                }
            ]
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert_eq!(manifest.name, "blog");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.models.len(), 1);
        assert_eq!(manifest.models[0].name, "article");
        assert_eq!(manifest.menus.len(), 0);
        assert_eq!(manifest.views.len(), 0);
        assert_eq!(manifest.actions.len(), 0);
    }

    #[test]
    fn test_manifest_minimal() {
        let json = r#"{"name": "minimal", "version": "0.1.0"}"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert_eq!(manifest.name, "minimal");
        assert!(manifest.models.is_empty());
        assert!(manifest.description.is_empty());
    }

    #[test]
    fn test_manifest_roundtrip() {
        let original = PluginManifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "测试插件".to_string(),
            models: vec![ModelDescriptor::new("item", "items").required_field("name", crate::module::FieldType::Text)],
            menus: vec![],
            views: vec![],
            actions: vec![],
            records: None,
        };
        let json = original.to_json().unwrap();
        let restored = PluginManifest::from_json(&json).unwrap();
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.models.len(), 1);
        assert!(restored.records.is_none());
    }

    #[test]
    fn test_to_info() {
        let manifest = PluginManifest {
            name: "crm".to_string(),
            version: "2.0.0".to_string(),
            description: "CRM模块".to_string(),
            models: vec![ModelDescriptor::new("customer", "customers"), ModelDescriptor::new("deal", "deals")],
            menus: vec![],
            views: vec![],
            actions: vec![],
            records: Some({
                let mut m = HashMap::new();
                m.insert("customer".to_string(), vec![serde_json::json!({"name": "Acme"})]);
                m
            }),
        };
        let info = manifest.to_info(PluginState::Active);
        assert_eq!(info.name, "crm");
        assert_eq!(info.model_count, 2);
        assert_eq!(info.state, PluginState::Active);
        assert_eq!(info.record_model_count, 1);
    }

    #[test]
    fn test_invalid_json() {
        let result = PluginManifest::from_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_manifest_with_records_deserialization() {
        let json = r#"{
            "name": "blog",
            "version": "1.0.0",
            "models": [
                {"name": "article", "table_name": "articles", "fields": [
                    {"name": "title", "field_type": "text"}
                ]}
            ],
            "records": {
                "article": [
                    {"title": "第一篇文章", "status": "draft"},
                    {"title": "第二篇文章", "status": "published"}
                ]
            }
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert_eq!(manifest.name, "blog");
        let records = manifest.records.unwrap();
        assert_eq!(records.len(), 1);
        let articles = records.get("article").unwrap();
        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0]["title"], "第一篇文章");
    }

    #[test]
    fn test_manifest_records_optional() {
        let json = r#"{"name": "minimal", "version": "1.0.0", "models": []}"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert!(manifest.records.is_none());
    }
}
