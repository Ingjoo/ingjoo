use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDescriptor {
    pub name: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationConfig {
    pub related_model: String,
    #[serde(default)]
    pub foreign_key: Option<String>,
    #[serde(default)]
    pub through: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdType {
    Text,
    Integer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub name: String,
    pub table_name: String,
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
    pub fn new(name: &str, table_name: &str) -> Self {
        Self {
            name: name.to_string(),
            table_name: table_name.to_string(),
            fields: Vec::new(),
            id_type: IdType::Text,
            audit_fields: false,
        }
    }

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

    pub fn with_id_type(mut self, id_type: IdType) -> Self {
        self.id_type = id_type;
        self
    }

    pub fn with_audit_fields(mut self) -> Self {
        self.audit_fields = true;
        self
    }

    pub fn field_names(&self) -> Vec<&str> {
        self.fields.iter().map(|f| f.name.as_str()).collect()
    }

    pub fn find_field(&self, name: &str) -> Option<&FieldDescriptor> {
        self.fields.iter().find(|f| f.name == name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModelRegistry {
    models: HashMap<String, ModelDescriptor>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, model: ModelDescriptor) {
        self.models.insert(model.name.clone(), model);
    }

    pub fn get(&self, name: &str) -> Option<&ModelDescriptor> {
        self.models.get(name)
    }

    pub fn list(&self) -> Vec<&ModelDescriptor> {
        self.models.values().collect()
    }

    pub fn unregister(&mut self, name: &str) -> bool {
        self.models.remove(name).is_some()
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
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
        let mut registry = ModelRegistry::new();
        let model = ModelDescriptor::new("article", "articles")
            .required_field("title", FieldType::Text);
        registry.register(model);

        assert!(registry.get("article").is_some());
        assert!(registry.get("nonexistent").is_none());
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn test_field_names() {
        let model = ModelDescriptor::new("article", "articles")
            .field("title", FieldType::Text)
            .field("body", FieldType::Text);
        assert_eq!(model.field_names(), vec!["title", "body"]);
    }

    #[test]
    fn test_unregister() {
        let mut registry = ModelRegistry::new();
        registry.register(ModelDescriptor::new("article", "articles"));
        assert!(registry.unregister("article"));
        assert!(!registry.unregister("article"));
        assert!(registry.is_empty());
    }
}
