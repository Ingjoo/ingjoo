//! 三层安全模型 (Odoo ir.model.access + ir.rule 简化版)
//!
//! Layer 1: 模型级访问 — 角色 × 实体 × 操作 (CRUD)
//! Layer 2: 记录级规则 — Domain 过滤 (行级隔离)
//! Layer 3: 集合隔离 — 自动注入 collection_id 过滤

use anyhow::Result;
use ingjoo_core::query::domain::{Domain, SqlCondition};
use ingjoo_core::Dialect;
use serde::{Deserialize, Serialize};

/// CRUD 及导入导出操作类型
#[derive(Clone, Debug, PartialEq)]
pub enum AccessOp {
    Read,
    Write,
    Create,
    Delete,
    Import,
    Export,
}

/// Layer 1: 模型级访问规则 — 单条"角色 × 模型"的 CRUD 权限矩阵
///
/// 每条规则声明一个角色对某个模型拥有哪些操作权限，
/// 由 [`SecurityPolicy::check_access_groups()`] 在运行时遍历匹配。
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

/// Layer 2: 记录级过滤规则 — Domain 自动注入 WHERE
///
/// 每条规则绑定一个角色和一个 Domain 表达式，查询时自动拼接到 SQL 条件中，
/// 实现行级数据隔离。参考 Odoo `ir.rule`。
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

/// 三层权限引擎 — 模型级、记录级、集合级的统一安全策略
///
/// 通过 [`SecurityPolicy::new()`] 创建空策略，再用 `add_model_access()` 和
/// `add_record_rule()` 注入规则。CRUD 处理器调用各层检查方法决定访问权限。
///
/// - Layer 1: [`check_access_groups()`](SecurityPolicy::check_access_groups)
/// - Layer 2: [`record_filter_groups_with_dialect()`](SecurityPolicy::record_filter_groups_with_dialect)
/// - Layer 3: [`collection_isolation()`](SecurityPolicy::collection_isolation)
#[derive(Clone, Debug, Default)]
pub struct SecurityPolicy {
    model_accesses: Vec<ModelAccess>,
    record_rules: Vec<RecordRule>,
}

impl SecurityPolicy {
    /// 创建空策略（等价于 `default()`）
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一条 Layer 1 模型级访问规则
    pub fn add_model_access(&mut self, access: ModelAccess) {
        self.model_accesses.push(access);
    }

    /// 注册一条 Layer 2 记录级过滤规则
    pub fn add_record_rule(&mut self, rule: RecordRule) {
        self.record_rules.push(rule);
    }

    /// Layer 1 便捷接口 — 单角色模型级权限检查
    ///
    /// 将 role 包装为单元素 groups 列表后委托给 [`check_access_groups()`](Self::check_access_groups)。
    pub fn check_access(&self, model: &str, role: &str, op: AccessOp) -> bool {
        let groups = vec![role.to_string()];
        self.check_access_groups(model, &groups, op)
    }

    /// Layer 1: 模型级权限检查 — 判断指定角色组是否对某模型有指定操作权限
    ///
    /// 遍历已注册的 [`ModelAccess`] 规则，匹配 model + groups + op。
    /// 未定义规则的模型默认 admin 放行、其他角色拒绝。
    pub fn check_access_groups(&self, model: &str, groups: &[String], op: AccessOp) -> bool {
        if groups.is_empty() {
            return false;
        }
        for access in &self.model_accesses {
            if access.model == model && groups.iter().any(|g| g == &access.role) {
                let allowed = match op {
                    AccessOp::Read => access.read,
                    AccessOp::Write => access.write,
                    AccessOp::Create => access.create,
                    AccessOp::Delete => access.delete,
                    AccessOp::Import => access.import,
                    AccessOp::Export => access.export,
                };
                if allowed {
                    return true;
                }
            }
        }
        // 未定义规则 → admin 全部放行，其他角色拒绝
        groups.iter().any(|g| g == "admin")
    }

    /// Layer 2 便捷接口 — 单角色记录级过滤
    ///
    /// 将 role 包装为单元素 groups 列表后委托给 [`record_filter_groups()`](Self::record_filter_groups)。
    pub fn record_filter(&self, model: &str, role: &str, _user_id: &str, op: &AccessOp) -> SqlCondition {
        let groups = vec![role.to_string()];
        self.record_filter_groups(model, &groups, _user_id, op)
    }

    /// Layer 2 便捷接口 — 多角色记录级过滤（使用默认方言）
    ///
    /// 委托给 [`record_filter_groups_with_dialect()`](Self::record_filter_groups_with_dialect)，方言传 `None`。
    pub fn record_filter_groups(&self, model: &str, groups: &[String], _user_id: &str, op: &AccessOp) -> SqlCondition {
        self.record_filter_groups_with_dialect(model, groups, _user_id, op, None)
    }

    /// Layer 2: 记录级过滤规则 — 将匹配的 Domain 转换为 SQL 条件
    ///
    /// 遍历所有 [`RecordRule`]，收集匹配 model + groups + op 的 Domain，
    /// 多条规则之间以 OR 组合。支持通过 `dialect` 参数做跨库 SQL 适配。
    /// 无匹配规则时返回空条件（不追加 WHERE）。
    pub fn record_filter_groups_with_dialect(&self, model: &str, groups: &[String], _user_id: &str, op: &AccessOp, dialect: Option<&Dialect>) -> SqlCondition {
        let mut domains: Vec<Domain> = Vec::new();

        for rule in &self.record_rules {
            if rule.model != model || !groups.iter().any(|g| g == &rule.role) {
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
            domains.remove(0).to_sql_with_dialect(None, dialect)
        } else {
            Domain::Or(domains).to_sql_with_dialect(None, dialect)
        }
    }

    /// Layer 3: 集合隔离 — 生成 `collection_id IN (...)` 条件
    ///
    /// 多租户场景下，将查询范围限制在指定的集合列表内。
    /// `alias` 用于表连接时指定表别名，如 `e.collection_id IN (...)`。
    /// 空列表返回 `1=0`（拒绝所有）。
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

/// 策略文档 — 用于从 `module_settings` 的 JSON 值反序列化
///
/// 包含模型级访问定义和记录级规则定义两部分，可被
/// [`SecurityBuilder::from_policy_json()`] 解析并构建为 [`SecurityPolicy`]。
#[derive(Serialize, Deserialize, Debug)]
pub struct PolicyDocument {
    #[serde(default)]
    pub model_accesses: Vec<ModelAccessDef>,
    #[serde(default)]
    pub record_rules: Vec<RecordRuleDef>,
}

/// 模型级访问定义 — JSON 反序列化用的中间结构
///
/// 字段与 [`ModelAccess`] 一一对应，供 [`PolicyDocument`] 使用。
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

/// 记录级规则定义 — JSON 反序列化用的中间结构
///
/// `domain` 字段为 Domain DSL JSON 字符串，如 `["status", "=", "published"]`，
/// 解析时由 [`SecurityBuilder::from_policy_json()`] 转换为 [`Domain`]。
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

/// 从数据库构建 [`SecurityPolicy`] 的建造器
///
/// 支持编程式逐步添加规则，或通过 [`from_policy_json()`](Self::from_policy_json)
/// 从 JSON 一次性构建。
pub struct SecurityBuilder {
    policy: SecurityPolicy,
}

impl Default for SecurityBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityBuilder {
    /// 创建空建造器
    pub fn new() -> Self {
        Self {
            policy: SecurityPolicy::new(),
        }
    }

    /// 获取内部策略的可变引用，用于直接操作
    pub fn policy(&mut self) -> &mut SecurityPolicy {
        &mut self.policy
    }

    /// 消耗建造器，返回已组装的策略
    pub fn build(self) -> SecurityPolicy {
        self.policy
    }

    /// 从 JSON 字符串构建策略
    ///
    /// JSON 格式参见 [`PolicyDocument`]。解析 `model_accesses` 和 `record_rules`，
    /// 其中 `record_rules[].domain` 会被解析为 [`Domain`]。
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
    /// 角色对模型执行操作被拒绝
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

    // ── Layer 3: collection_isolation 扩展测试 ──────────────────

    #[test]
    fn test_collection_isolation_multiple_collections() {
        let policy = test_policy();
        let ids: Vec<String> = (1..=5).map(|i| format!("col_{}", i)).collect();
        let result = policy.collection_isolation(&ids, None);
        assert!(result.clause.contains("collection_id IN"));
        assert_eq!(result.params.len(), 5);
        assert_eq!(result.params[0], "col_1");
        assert_eq!(result.params[4], "col_5");
        assert_eq!(result.clause, "collection_id IN (?, ?, ?, ?, ?)");
    }

    #[test]
    fn test_collection_isolation_with_table_alias() {
        let policy = test_policy();
        let ids = vec!["col_a".to_string()];
        let result = policy.collection_isolation(&ids, Some("t0"));
        assert!(result.clause.contains("t0.collection_id"));
        assert_eq!(result.clause, "t0.collection_id IN (?)");
        assert_eq!(result.params, vec!["col_a"]);
    }

    #[test]
    fn test_collection_isolation_sql_injection_prevention() {
        let policy = test_policy();
        let ids = vec!["'; DROP TABLE users; --".to_string()];
        let result = policy.collection_isolation(&ids, None);
        assert_eq!(result.params[0], "'; DROP TABLE users; --");
        assert!(result.clause.contains("?"));
        assert_eq!(result.clause, "collection_id IN (?)");
    }

    #[test]
    fn test_collection_isolation_special_characters() {
        let policy = test_policy();
        let ids = vec![
            "col%20with%20spaces".to_string(),
            "集合甲".to_string(),
            "col\"quoted".to_string(),
        ];
        let result = policy.collection_isolation(&ids, None);
        assert_eq!(result.params.len(), 3);
        assert_eq!(result.params[0], "col%20with%20spaces");
        assert_eq!(result.params[1], "集合甲");
        assert_eq!(result.params[2], "col\"quoted");
    }

    #[test]
    fn test_collection_isolation_single_id() {
        let policy = test_policy();
        let result = policy.collection_isolation(&["only_one".to_string()], None);
        assert_eq!(result.clause, "collection_id IN (?)");
        assert_eq!(result.params, vec!["only_one"]);
    }

    #[test]
    fn test_collection_isolation_alias_with_multiple_ids() {
        let policy = test_policy();
        let ids = vec!["c1".to_string(), "c2".to_string(), "c3".to_string()];
        let result = policy.collection_isolation(&ids, Some("rec"));
        assert_eq!(result.clause, "rec.collection_id IN (?, ?, ?)");
        assert_eq!(result.params, vec!["c1", "c2", "c3"]);
    }
}
