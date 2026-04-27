//! 动态模型注册表 — 运行时定义模型结构与关系

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// 字段类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Integer,
    Float,
    Boolean,
    Timestamp,
    Json,
    Many2one,
}

/// 字段描述 — 模型中单个字段的元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDescriptor {
    /// 字段名
    pub name: String,
    /// 字段类型
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub relation: Option<RelationConfig>,
}

/// 关系配置 — Many2one 字段的关联信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationConfig {
    /// 关联的目标模型名
    pub related_model: String,
    #[serde(default)]
    pub foreign_key: Option<String>,
    #[serde(default)]
    pub through: Option<String>,
}

/// ID 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdType {
    Text,
    Integer,
}

/// 模型描述 — 定义一个动态模型的完整结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDescriptor {
    /// 模型名称（如 `"article"`）
    pub name: String,
    /// 对应数据库表名（如 `"articles"`）
    pub table_name: String,
    /// 字段列表
    pub fields: Vec<FieldDescriptor>,
    #[serde(default = "default_id_type")]
    pub id_type: IdType,
    #[serde(default)]
    pub audit_fields: bool,
}

fn default_id_type() -> IdType {
    IdType::Text
}

impl ModelDescriptor {
    /// 创建新的模型描述
    pub fn new(name: &str, table_name: &str) -> Self {
        Self {
            name: name.to_string(),
            table_name: table_name.to_string(),
            fields: Vec::new(),
            id_type: IdType::Text,
            audit_fields: false,
        }
    }

    /// 添加普通字段
    pub fn field(mut self, name: &str, field_type: FieldType) -> Self {
        self.fields.push(FieldDescriptor {
            name: name.to_string(),
            field_type,
            required: false,
            default_value: None,
            unique: false,
            relation: None,
        });
        self
    }

    /// 添加必填字段
    pub fn required_field(mut self, name: &str, field_type: FieldType) -> Self {
        self.fields.push(FieldDescriptor {
            name: name.to_string(),
            field_type,
            required: true,
            default_value: None,
            unique: false,
            relation: None,
        });
        self
    }

    /// 添加唯一字段
    pub fn unique_field(mut self, name: &str, field_type: FieldType) -> Self {
        self.fields.push(FieldDescriptor {
            name: name.to_string(),
            field_type,
            required: false,
            default_value: None,
            unique: true,
            relation: None,
        });
        self
    }

    /// 添加 Many2one 关系字段
    pub fn many2one(mut self, name: &str, related_model: &str) -> Self {
        self.fields.push(FieldDescriptor {
            name: name.to_string(),
            field_type: FieldType::Many2one,
            required: false,
            default_value: None,
            unique: false,
            relation: Some(RelationConfig {
                related_model: related_model.to_string(),
                foreign_key: None,
                through: None,
            }),
        });
        self
    }

    /// 设置 ID 类型
    pub fn with_id_type(mut self, id_type: IdType) -> Self {
        self.id_type = id_type;
        self
    }

    /// 启用审计字段（created_at / updated_at）
    pub fn with_audit_fields(mut self) -> Self {
        self.audit_fields = true;
        self
    }

    /// 返回所有字段名列表
    pub fn field_names(&self) -> Vec<&str> {
        self.fields.iter().map(|f| f.name.as_str()).collect()
    }

    /// 按名称查找字段
    pub fn find_field(&self, name: &str) -> Option<&FieldDescriptor> {
        self.fields.iter().find(|f| f.name == name)
    }
}

/// 模型注册表 — 运行时管理所有已注册的动态模型（线程安全）
#[derive(Debug, Default)]
pub struct ModelRegistry {
    models: RwLock<HashMap<String, ModelDescriptor>>,
}

impl ModelRegistry {
    /// 创建空注册表
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个模型（同名则覆盖）
    pub fn register(&self, model: ModelDescriptor) {
        self.models.write().unwrap().insert(model.name.clone(), model);
    }

    /// 按名称获取模型描述
    pub fn get(&self, name: &str) -> Option<ModelDescriptor> {
        self.models.read().unwrap().get(name).cloned()
    }

    /// 返回所有已注册模型
    pub fn list(&self) -> Vec<ModelDescriptor> {
        self.models.read().unwrap().values().cloned().collect()
    }

    /// 注销一个模型，返回是否成功
    pub fn unregister(&self, name: &str) -> bool {
        self.models.write().unwrap().remove(name).is_some()
    }

    /// 已注册模型数量
    pub fn len(&self) -> usize {
        self.models.read().unwrap().len()
    }

    /// 注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.models.read().unwrap().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_descriptor_builder() {
        let model = ModelDescriptor::new("article", "articles")
            .required_field("title", FieldType::Text)
            .field("body", FieldType::Text)
            .field("views", FieldType::Integer)
            .field("published", FieldType::Boolean);

        assert_eq!(model.name, "article");
        assert_eq!(model.table_name, "articles");
        assert_eq!(model.fields.len(), 4);
        assert!(model.find_field("title").unwrap().required);
        assert!(!model.find_field("body").unwrap().required);
    }

    #[test]
    fn test_model_registry() {
        let registry = ModelRegistry::new();
        let model = ModelDescriptor::new("article", "articles").required_field("title", FieldType::Text);
        registry.register(model);

        assert!(registry.get("article").is_some());
        assert!(registry.get("nonexistent").is_none());
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn test_field_names() {
        let model =
            ModelDescriptor::new("article", "articles").field("title", FieldType::Text).field("body", FieldType::Text);
        assert_eq!(model.field_names(), vec!["title", "body"]);
    }

    #[test]
    fn test_unregister() {
        let registry = ModelRegistry::new();
        registry.register(ModelDescriptor::new("article", "articles"));
        assert!(registry.unregister("article"));
        assert!(!registry.unregister("article"));
        assert!(registry.is_empty());
    }
}
