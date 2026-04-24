//! 关系加载器 — One2Many 和 Many2One 批量加载

use async_trait::async_trait;

/// 关系加载器 — 批量加载关联数据，避免 N+1 查询
#[async_trait]
pub trait RelationLoader: Send + Sync {
    /// 批量加载一对多关系（每个 ID 对应多条记录）
    async fn load_one2many(
        &self,
        model: &str,
        field: &str,
        ids: &[String],
    ) -> Result<std::collections::HashMap<String, Vec<serde_json::Value>>, anyhow::Error>;

    /// 批量加载多对一关系（每个 ID 对应零或一条记录）
    async fn load_many2one(
        &self,
        model: &str,
        field: &str,
        ids: &[String],
    ) -> Result<std::collections::HashMap<String, Option<serde_json::Value>>, anyhow::Error>;
}

/// 多对多关系配置（中间表描述）
#[derive(Debug, Clone)]
pub struct Many2Many {
    pub table: String,
    pub column_a: String,
    pub column_b: String,
    pub model_a: String,
    pub model_b: String,
}

impl Many2Many {
    /// 创建多对多关系配置，列名按 `{model}_id` 约定自动生成
    pub fn new(table: &str, model_a: &str, model_b: &str) -> Self {
        Self {
            table: table.to_string(),
            column_a: format!("{}_id", model_a),
            column_b: format!("{}_id", model_b),
            model_a: model_a.to_string(),
            model_b: model_b.to_string(),
        }
    }
}
