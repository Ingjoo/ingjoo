//! 三层安全模型 (Odoo ir.model.access + ir.rule 简化版)
//!
//! Layer 1: 模型级访问 — 角色 × 实体 × 操作 (CRUD)
//! Layer 2: 记录级规则 — Domain 过滤 (行级隔离)
//! Layer 3: 集合隔离 — 自动注入 collection_id 过滤

use anyhow::Result;
use ingjoo_core::query::domain::{Domain, SqlCondition};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub enum AccessOp {
    Read,
    Write,
    Create,
    Delete,
    Import,
    Export,
}

#[derive(Clone, Debug)]
pub struct ModelAccess {
    pub model: String,
    pub role: String,
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub delete: bool,
    pub import: bool,
    pub export: bool,
}

#[derive(Clone, Debug)]
pub struct RecordRule {
    pub model: String,
    pub role: String,
    pub domain: Domain,
    pub perm_read: bool,
    pub perm_write: bool,
    pub perm_create: bool,
    pub perm_delete: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SecurityPolicy {
    model_accesses: Vec<ModelAccess>,
    record_rules: Vec<RecordRule>,
}

impl SecurityPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_model_access(&mut self, access: ModelAccess) {
        self.model_accesses.push(access);
    }

    pub fn add_record_rule(&mut self, rule: RecordRule) {
        self.record_rules.push(rule);
    }

    /// Layer 1: 检查角色是否有权限执行操作
    pub fn check_access(&self, model: &str, role: &str, op: AccessOp) -> bool {
        for access in &self.model_accesses {
            if access.model == model && access.role == role {
                return match op {
                    AccessOp::Read => access.read,
                    AccessOp::Write => access.write,
                    AccessOp::Create => access.create,
                    AccessOp::Delete => access.delete,
                    AccessOp::Import => access.import,
                    AccessOp::Export => access.export,
                };
            }
        }
        // 未定义规则 → admin 全部放行，其他角色拒绝
        role == "admin"
    }

    /// Layer 2: 获取记录级过滤 Domain
    ///
    /// 返回的 SqlCondition 用于追加到查询的 WHERE 子句
    pub fn record_filter(&self, model: &str, role: &str, _user_id: &str, op: &AccessOp) -> SqlCondition {
        let mut domains: Vec<Domain> = Vec::new();

        for rule in &self.record_rules {
            if rule.model != model || rule.role != role {
                continue;
            }
            let applicable = match op {
                AccessOp::Read => rule.perm_read,
                AccessOp::Write => rule.perm_write,
                AccessOp::Create => rule.perm_create,
                AccessOp::Delete => rule.perm_delete,
                AccessOp::Import | AccessOp::Export => false,
            };
            if applicable {
                domains.push(rule.domain.clone());
            }
        }

        if domains.is_empty() {
            SqlCondition::empty()
        } else if domains.len() == 1 {
            domains.remove(0).to_sql(None)
        } else {
            Domain::And(domains).to_sql(None)
        }
    }

    /// Layer 3: 集合隔离 — 生成 collection_id IN (...) 条件
    pub fn collection_isolation(&self, collection_ids: &[String], alias: Option<&str>) -> SqlCondition {
        if collection_ids.is_empty() {
            return SqlCondition::expr("1=0".to_string());
        }
        let col = if let Some(a) = alias {
            format!("{}.collection_id", a)
        } else {
            "collection_id".to_string()
        };
        let placeholders: Vec<&str> = collection_ids.iter().map(|_| "?").collect();
        SqlCondition {
            clause: format!("{} IN ({})", col, placeholders.join(", ")),
            params: collection_ids.to_vec(),
        }
    }
}

/// 策略文档 — 用于从 module_settings 的 JSON 值反序列化
#[derive(Serialize, Deserialize, Debug)]
pub struct PolicyDocument {
    #[serde(default)]
    pub model_accesses: Vec<ModelAccessDef>,
    #[serde(default)]
    pub record_rules: Vec<RecordRuleDef>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ModelAccessDef {
    pub model: String,
    pub role: String,
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub delete: bool,
    #[serde(default)]
    pub import: bool,
    #[serde(default)]
    pub export: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RecordRuleDef {
    pub model: String,
    pub role: String,
    /// Domain DSL JSON 字符串，如 `["status", "=", "published"]`
    pub domain: String,
    pub perm_read: bool,
    pub perm_write: bool,
    pub perm_create: bool,
    pub perm_delete: bool,
}

/// 从数据库构建 SecurityPolicy
pub struct SecurityBuilder {
    policy: SecurityPolicy,
}

impl SecurityBuilder {
    pub fn new() -> Self {
        Self {
            policy: SecurityPolicy::new(),
        }
    }

    pub fn policy(&mut self) -> &mut SecurityPolicy {
        &mut self.policy
    }

    pub fn build(self) -> SecurityPolicy {
        self.policy
    }

    /// 从 JSON 字符串构建策略
    pub fn from_policy_json(json: &str) -> Result<Self> {
        let doc: PolicyDocument = serde_json::from_str(json)?;
        let mut builder = Self::new();
        for def in doc.model_accesses {
            builder.policy.add_model_access(ModelAccess {
                model: def.model,
                role: def.role,
                read: def.read,
                write: def.write,
                create: def.create,
                delete: def.delete,
                import: def.import,
                export: def.export,
            });
        }
        for def in doc.record_rules {
            let domain = Domain::from_json(&def.domain)?;
            builder.policy.add_record_rule(RecordRule {
                model: def.model,
                role: def.role,
                domain,
                perm_read: def.perm_read,
                perm_write: def.perm_write,
                perm_create: def.perm_create,
                perm_delete: def.perm_delete,
            });
        }
        Ok(builder)
    }
}

/// 安全检查错误
#[derive(Debug)]
pub enum SecurityError {
    AccessDenied {
        model: String,
        op: String,
        role: String,
    },
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityError::AccessDenied { model, op, role } => {
                write!(f, "角色 '{}' 无权对 '{}' 执行 {} 操作", role, model, op)
            }
        }
    }
}

impl std::error::Error for SecurityError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_policy() -> SecurityPolicy {
        let mut builder = SecurityBuilder::new();
        for model in &["collection", "entry", "source", "project"] {
            builder.policy.add_model_access(ModelAccess {
                model: model.to_string(),
                role: "admin".to_string(),
                read: true, write: true, create: true, delete: true,
                import: true, export: true,
            });
        }
        for model in &["collection", "entry", "source", "project"] {
            builder.policy.add_model_access(ModelAccess {
                model: model.to_string(),
                role: "viewer".to_string(),
                read: true, write: false, create: false, delete: false,
                import: false, export: true,
            });
        }
        for model in &["collection", "entry", "source", "project"] {
            builder.policy.add_model_access(ModelAccess {
                model: model.to_string(),
                role: "user".to_string(),
                read: true, write: true, create: true, delete: false,
                import: true, export: true,
            });
        }
        builder.policy.add_record_rule(RecordRule {
            model: "entry".to_string(),
            role: "viewer".to_string(),
            domain: Domain::from_json(r#"["status", "=", "published"]"#).unwrap(),
            perm_read: true, perm_write: false, perm_create: false, perm_delete: false,
        });
        builder.policy.add_record_rule(RecordRule {
            model: "source".to_string(),
            role: "viewer".to_string(),
            domain: Domain::from_json(r#"["status", "=", "compiled"]"#).unwrap(),
            perm_read: true, perm_write: false, perm_create: false, perm_delete: false,
        });
        builder.build()
    }

    #[test]
    fn test_admin_full_access() {
        let policy = test_policy();
        for model in &["collection", "entry", "source", "project"] {
            assert!(policy.check_access(model, "admin", AccessOp::Read));
            assert!(policy.check_access(model, "admin", AccessOp::Write));
            assert!(policy.check_access(model, "admin", AccessOp::Create));
            assert!(policy.check_access(model, "admin", AccessOp::Delete));
        }
    }

    #[test]
    fn test_viewer_read_only() {
        let policy = test_policy();
        assert!(policy.check_access("entry", "viewer", AccessOp::Read));
        assert!(!policy.check_access("entry", "viewer", AccessOp::Write));
        assert!(!policy.check_access("entry", "viewer", AccessOp::Create));
        assert!(!policy.check_access("entry", "viewer", AccessOp::Delete));
    }

    #[test]
    fn test_user_standard() {
        let policy = test_policy();
        assert!(policy.check_access("entry", "user", AccessOp::Read));
        assert!(policy.check_access("entry", "user", AccessOp::Write));
        assert!(policy.check_access("entry", "user", AccessOp::Create));
        assert!(!policy.check_access("entry", "user", AccessOp::Delete));
    }

    #[test]
    fn test_viewer_entry_filter() {
        let policy = test_policy();
        let filter = policy.record_filter("entry", "viewer", "u1", &AccessOp::Read);
        assert_eq!(filter.clause, "status = ?");
        assert_eq!(filter.params, vec!["published"]);
    }

    #[test]
    fn test_viewer_source_filter() {
        let policy = test_policy();
        let filter = policy.record_filter("source", "viewer", "u1", &AccessOp::Read);
        assert_eq!(filter.clause, "status = ?");
        assert_eq!(filter.params, vec!["compiled"]);
    }

    #[test]
    fn test_admin_no_record_filter() {
        let policy = test_policy();
        let filter = policy.record_filter("entry", "admin", "u1", &AccessOp::Read);
        assert!(filter.clause.is_empty());
    }

    #[test]
    fn test_collection_isolation() {
        let policy = test_policy();
        let filter = policy.collection_isolation(&["c1".to_string(), "c2".to_string()], None);
        assert_eq!(filter.clause, "collection_id IN (?, ?)");
        assert_eq!(filter.params, vec!["c1", "c2"]);
    }

    #[test]
    fn test_collection_isolation_empty() {
        let policy = test_policy();
        let filter = policy.collection_isolation(&[], None);
        assert_eq!(filter.clause, "1=0");
    }

    #[test]
    fn test_collection_isolation_with_alias() {
        let policy = test_policy();
        let filter = policy.collection_isolation(&["c1".to_string()], Some("e"));
        assert_eq!(filter.clause, "e.collection_id IN (?)");
    }

    #[test]
    fn test_unknown_role_denied() {
        let policy = test_policy();
        assert!(!policy.check_access("entry", "guest", AccessOp::Read));
    }

    #[test]
    fn test_unknown_model_admin_allowed() {
        let policy = test_policy();
        assert!(policy.check_access("unknown_model", "admin", AccessOp::Read));
    }

    #[test]
    fn test_unknown_model_user_denied() {
        let policy = test_policy();
        assert!(!policy.check_access("unknown_model", "user", AccessOp::Read));
    }

    #[test]
    fn test_from_policy_json_basic() {
        let json = r#"{
            "model_accesses": [
                {
                    "model": "entry",
                    "role": "editor",
                    "read": true,
                    "write": true,
                    "create": true,
                    "delete": false,
                    "import": true,
                    "export": false
                }
            ]
        }"#;
        let policy = SecurityBuilder::from_policy_json(json).unwrap().build();
        assert!(policy.check_access("entry", "editor", AccessOp::Read));
        assert!(policy.check_access("entry", "editor", AccessOp::Write));
        assert!(policy.check_access("entry", "editor", AccessOp::Import));
        assert!(!policy.check_access("entry", "editor", AccessOp::Delete));
        assert!(!policy.check_access("entry", "editor", AccessOp::Export));
    }

    #[test]
    fn test_import_export_access() {
        let policy = test_policy();
        // admin: 全部 true
        assert!(policy.check_access("entry", "admin", AccessOp::Import));
        assert!(policy.check_access("entry", "admin", AccessOp::Export));
        // viewer: read=true, export=true, import=false
        assert!(!policy.check_access("entry", "viewer", AccessOp::Import));
        assert!(policy.check_access("entry", "viewer", AccessOp::Export));
        // user: import=true, export=true
        assert!(policy.check_access("entry", "user", AccessOp::Import));
        assert!(policy.check_access("entry", "user", AccessOp::Export));
    }

    #[test]
    fn test_from_policy_json_with_record_rules() {
        let json = r#"{
            "model_accesses": [
                {
                    "model": "entry",
                    "role": "viewer",
                    "read": true,
                    "write": false,
                    "create": false,
                    "delete": false,
                    "import": false,
                    "export": true
                }
            ],
            "record_rules": [
                {
                    "model": "entry",
                    "role": "viewer",
                    "domain": "[\"status\", \"=\", \"draft\"]",
                    "perm_read": true,
                    "perm_write": false,
                    "perm_create": false,
                    "perm_delete": false
                }
            ]
        }"#;
        let policy = SecurityBuilder::from_policy_json(json).unwrap().build();
        assert!(policy.check_access("entry", "viewer", AccessOp::Read));
        assert!(policy.check_access("entry", "viewer", AccessOp::Export));
        let filter = policy.record_filter("entry", "viewer", "u1", &AccessOp::Read);
        assert_eq!(filter.clause, "status = ?");
        assert_eq!(filter.params, vec!["draft"]);
    }

    #[test]
    fn test_from_policy_json_invalid() {
        let result = SecurityBuilder::from_policy_json("{ not valid json }");
        assert!(result.is_err());
    }
}
