//! Domain DSL — Odoo 风格声明式过滤表达式
//!
//! 语法: `[operator, domain1, domain2, ...]` 或 `[field, op, value]`
//!
//! 示例:
//! ```json
//! ["&", ["status", "=", "published"], ["collection_id", "in", ["id1", "id2"]]]
//! ```
//!
//! 编译为 SQL WHERE 子句:
//! ```sql
//! (status = ? AND collection_id IN (?, ?))
//! ```

use anyhow::{anyhow, Result};
use serde_json::Value;
use std::fmt;

/// Domain AST — 过滤表达式的抽象语法树
#[derive(Clone, Debug, PartialEq)]
pub enum Domain {
    /// 叶节点: field op value
    Leaf {
        field: String,
        op: DomainOp,
        value: DomainValue,
    },
    /// 逻辑与 (默认): domain1 AND domain2 AND ...
    And(Vec<Domain>),
    /// 逻辑或: domain1 OR domain2 OR ...
    Or(Vec<Domain>),
    /// 逻辑非: NOT domain
    Not(Box<Domain>),
}

/// 比较操作符
#[derive(Clone, Debug, PartialEq)]
pub enum DomainOp {
    Equal,        // =
    NotEqual,     // !=
    Less,         // <
    Greater,      // >
    LessEqual,    // <=
    GreaterEqual, // >=
    Like,         // LIKE (区分大小写)
    ILike,        // LIKE (不区分大小写)
    NotLike,
    NotILike,
    In,           // IN
    NotIn,        // NOT IN
    IsNull,       // IS NULL (value=true) / IS NOT NULL (value=false)
    Between,      // BETWEEN (value=[lo, hi])
    ChildOf,      // 父子关系 (暂不支持，预留)
}

/// 域值类型
#[derive(Clone, Debug, PartialEq)]
pub enum DomainValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<DomainValue>),
}

/// SQL 编译结果
#[derive(Clone, Debug)]
pub struct SqlCondition {
    /// SQL WHERE 片段，如 "field1 = ? AND field2 IN (?, ?)"
    pub clause: String,
    /// 按顺序收集的参数值（全部序列化为 String 给 sqlx bind）
    pub params: Vec<String>,
}

impl Domain {
    pub fn parse(value: &Value) -> Result<Self> {
        match value {
            Value::Array(arr) if arr.is_empty() => Err(anyhow!("Domain 表达式不能为空")),
            Value::Array(arr) => {
                if let Some(Value::String(s)) = arr.first() {
                    match s.as_str() {
                        "&" => {
                            if arr.len() < 3 {
                                return Err(anyhow!("AND 操作需要至少 2 个子域"));
                            }
                            let domains: Result<Vec<_>> = arr[1..].iter().map(Self::parse).collect();
                            Ok(Domain::And(domains?))
                        }
                        "|" => {
                            if arr.len() < 3 {
                                return Err(anyhow!("OR 操作需要至少 2 个子域"));
                            }
                            let domains: Result<Vec<_>> = arr[1..].iter().map(Self::parse).collect();
                            Ok(Domain::Or(domains?))
                        }
                        "!" => {
                            if arr.len() != 2 {
                                return Err(anyhow!("NOT 操作需要恰好 1 个子域"));
                            }
                            Ok(Domain::Not(Box::new(Self::parse(&arr[1])?)))
                        }
                    _ => Self::parse_leaf(arr),
                    }
                } else {
                    if arr.len() == 1 {
                        Self::parse(&arr[0])
                    } else {
                        let domains: Result<Vec<_>> = arr.iter().map(Self::parse).collect();
                        Ok(Domain::And(domains?))
                    }
                }
            }
            _ => Err(anyhow!("Domain 表达式必须是数组，收到: {}", value)),
        }
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(json)?;
        Self::parse(&value)
    }

    fn parse_leaf(arr: &[Value]) -> Result<Self> {
        if arr.len() < 3 {
            return Err(anyhow!("叶节点需要 [field, op, value] 格式，收到 {} 个元素", arr.len()));
        }
        let field = match &arr[0] {
            Value::String(s) => s.clone(),
            other => return Err(anyhow!("field 必须是字符串，收到: {}", other)),
        };
        let op = match &arr[1] {
            Value::String(s) => Self::parse_op(s)?,
            other => return Err(anyhow!("op 必须是字符串，收到: {}", other)),
        };
        let value = Self::parse_value(&arr[2])?;
        Ok(Domain::Leaf { field, op, value })
    }

    fn parse_op(s: &str) -> Result<DomainOp> {
        match s {
            "=" => Ok(DomainOp::Equal),
            "!=" => Ok(DomainOp::NotEqual),
            "<" => Ok(DomainOp::Less),
            ">" => Ok(DomainOp::Greater),
            "<=" => Ok(DomainOp::LessEqual),
            ">=" => Ok(DomainOp::GreaterEqual),
            "like" => Ok(DomainOp::Like),
            "ilike" => Ok(DomainOp::ILike),
            "not like" => Ok(DomainOp::NotLike),
            "not ilike" => Ok(DomainOp::NotILike),
            "in" => Ok(DomainOp::In),
            "not in" => Ok(DomainOp::NotIn),
            "is null" => Ok(DomainOp::IsNull),
            "between" => Ok(DomainOp::Between),
            "child_of" => Ok(DomainOp::ChildOf),
            _ => Err(anyhow!("不支持的操作符: '{}'", s)),
        }
    }

    fn parse_value(value: &Value) -> Result<DomainValue> {
        match value {
            Value::Null => Ok(DomainValue::Null),
            Value::Bool(b) => Ok(DomainValue::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(DomainValue::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(DomainValue::Float(f))
                } else {
                    Ok(DomainValue::String(n.to_string()))
                }
            }
            Value::String(s) => Ok(DomainValue::String(s.clone())),
            Value::Array(arr) => {
                let values: Result<Vec<_>> = arr.iter().map(Self::parse_value).collect();
                Ok(DomainValue::List(values?))
            }
            other => Err(anyhow!("不支持的值类型: {}", other)),
        }
    }
}

// ==================== SQL 编译 ====================

impl Domain {
    pub fn to_sql(&self, alias: Option<&str>) -> SqlCondition {
        match self {
            Domain::Leaf { field, op, value } => {
                let col = if let Some(a) = alias {
                    format!("{}.{}", a, field)
                } else {
                    field.clone()
                };
                op_to_sql(&col, op, value)
            }
            Domain::And(domains) => {
                let mut combined = SqlCondition::empty();
                let mut first = true;
                for d in domains {
                    let cond = d.to_sql(alias);
                    if !cond.clause.is_empty() {
                        if !first {
                            combined.clause.push_str(" AND ");
                        }
                        combined.clause.push('(');
                        combined.clause.push_str(&cond.clause);
                        combined.clause.push(')');
                        combined.params.extend(cond.params);
                        first = false;
                    }
                }
                combined
            }
            Domain::Or(domains) => {
                let mut combined = SqlCondition::empty();
                let mut first = true;
                for d in domains {
                    let cond = d.to_sql(alias);
                    if !cond.clause.is_empty() {
                        if !first {
                            combined.clause.push_str(" OR ");
                        }
                        combined.clause.push('(');
                        combined.clause.push_str(&cond.clause);
                        combined.clause.push(')');
                        combined.params.extend(cond.params);
                        first = false;
                    }
                }
                combined
            }
            Domain::Not(inner) => {
                let cond = inner.to_sql(alias);
                if cond.clause.is_empty() {
                    cond
                } else {
                    SqlCondition {
                        clause: format!("NOT ({})", cond.clause),
                        params: cond.params,
                    }
                }
            }
        }
    }

    /// 合并多个 Domain 为 AND
    pub fn all(domains: Vec<Domain>) -> Domain {
        Domain::And(domains)
    }

    /// 合并多个 Domain 为 OR
    pub fn any(domains: Vec<Domain>) -> Domain {
        Domain::Or(domains)
    }
}

fn op_to_sql(col: &str, op: &DomainOp, value: &DomainValue) -> SqlCondition {
    match op {
        DomainOp::Equal => match value {
            DomainValue::Null => SqlCondition::expr(format!("{} IS NULL", col)),
            _ => SqlCondition::bind(format!("{} = ?", col), value),
        },
        DomainOp::NotEqual => match value {
            DomainValue::Null => SqlCondition::expr(format!("{} IS NOT NULL", col)),
            _ => SqlCondition::bind(format!("{} != ?", col), value),
        },
        DomainOp::Less => SqlCondition::bind(format!("{} < ?", col), value),
        DomainOp::Greater => SqlCondition::bind(format!("{} > ?", col), value),
        DomainOp::LessEqual => SqlCondition::bind(format!("{} <= ?", col), value),
        DomainOp::GreaterEqual => SqlCondition::bind(format!("{} >= ?", col), value),
        DomainOp::Like => SqlCondition::bind(format!("{} LIKE ?", col), value),
        DomainOp::ILike => SqlCondition::bind(format!("{} LIKE ? COLLATE NOCASE", col), value),
        DomainOp::NotLike => SqlCondition::bind(format!("{} NOT LIKE ?", col), value),
        DomainOp::NotILike => SqlCondition::bind(format!("{} NOT LIKE ? COLLATE NOCASE", col), value),
        DomainOp::In => match value {
            DomainValue::List(items) if items.is_empty() => SqlCondition::expr("1=0".to_string()),
            DomainValue::List(items) => {
                let placeholders: Vec<&str> = items.iter().map(|_| "?").collect();
                let mut params = Vec::new();
                for item in items {
                    params.push(value_to_string(item));
                }
                SqlCondition {
                    clause: format!("{} IN ({})", col, placeholders.join(", ")),
                    params,
                }
            }
            _ => SqlCondition::bind(format!("{} = ?", col), value),
        },
        DomainOp::NotIn => {
            let inner = op_to_sql(col, &DomainOp::In, value);
            if inner.clause == "1=0" {
                SqlCondition::expr("1=1".to_string())
            } else {
                SqlCondition {
                    clause: format!("NOT ({})", inner.clause),
                    params: inner.params,
                }
            }
        }
        DomainOp::IsNull => match value {
            DomainValue::Bool(false) => SqlCondition::expr(format!("{} IS NULL", col)),
            _ => SqlCondition::expr(format!("{} IS NOT NULL", col)),
        },
        DomainOp::Between => match value {
            DomainValue::List(items) if items.len() == 2 => {
                SqlCondition {
                    clause: format!("{} BETWEEN ? AND ?", col),
                    params: vec![
                        value_to_string(&items[0]),
                        value_to_string(&items[1]),
                    ],
                }
            }
            _ => SqlCondition::empty(),
        },
        DomainOp::ChildOf => SqlCondition::empty(),
    }
}

fn value_to_string(v: &DomainValue) -> String {
    match v {
        DomainValue::Null => "NULL".to_string(),
        DomainValue::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        DomainValue::Integer(i) => i.to_string(),
        DomainValue::Float(f) => f.to_string(),
        DomainValue::String(s) => s.clone(),
        DomainValue::List(items) => items.iter().map(value_to_string).collect::<Vec<_>>().join(","),
    }
}

impl SqlCondition {
    pub fn empty() -> Self {
        Self {
            clause: String::new(),
            params: Vec::new(),
        }
    }

    pub fn expr(clause: String) -> Self {
        Self {
            clause,
            params: Vec::new(),
        }
    }

    fn bind(clause: String, value: &DomainValue) -> Self {
        Self {
            clause,
            params: vec![value_to_string(value)],
        }
    }

    pub fn apply_to_query(&self, sql: &mut String) {
        if self.clause.is_empty() {
            return;
        }
        if sql.contains("WHERE") || sql.contains("where") {
            sql.push_str(" AND (");
            sql.push_str(&self.clause);
            sql.push(')');
        } else {
            sql.push_str(" WHERE ");
            sql.push_str(&self.clause);
        }
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Domain::Leaf { field, op, value } => {
                write!(f, "({} {:?} {:?})", field, op, value)
            }
            Domain::And(ds) => {
                write!(f, "AND(")?;
                for (i, d) in ds.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", d)?;
                }
                write!(f, ")")
            }
            Domain::Or(ds) => {
                write!(f, "OR(")?;
                for (i, d) in ds.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", d)?;
                }
                write!(f, ")")
            }
            Domain::Not(d) => write!(f, "NOT({})", d),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_leaf() {
        let json = r#"["status", "=", "published"]"#;
        let domain = Domain::from_json(json).unwrap();
        assert!(matches!(domain, Domain::Leaf { .. }));
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "status = ?");
        assert_eq!(sql.params, vec!["published"]);
    }

    #[test]
    fn test_parse_and() {
        let json = r#"["&", ["status", "=", "published"], ["category", "=", "concept"]]"#;
        let domain = Domain::from_json(json).unwrap();
        assert!(matches!(domain, Domain::And(_)));
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "(status = ?) AND (category = ?)");
        assert_eq!(sql.params, vec!["published", "concept"]);
    }

    #[test]
    fn test_parse_or() {
        let json = r#"["|", ["status", "=", "draft"], ["status", "=", "published"]]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "(status = ?) OR (status = ?)");
        assert_eq!(sql.params, vec!["draft", "published"]);
    }

    #[test]
    fn test_parse_not() {
        let json = r#"["!", ["status", "=", "deleted"]]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "NOT (status = ?)");
        assert_eq!(sql.params, vec!["deleted"]);
    }

    #[test]
    fn test_parse_null() {
        let json = r#"["parent_id", "=", null]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "parent_id IS NULL");
        assert!(sql.params.is_empty());
    }

    #[test]
    fn test_parse_is_null() {
        let json = r#"["parent_id", "is null", false]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "parent_id IS NULL");
    }

    #[test]
    fn test_parse_in_list() {
        let json = r#"["status", "in", ["draft", "published", "review"]]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "status IN (?, ?, ?)");
        assert_eq!(sql.params, vec!["draft", "published", "review"]);
    }

    #[test]
    fn test_parse_empty_in() {
        let json = r#"["status", "in", []]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "1=0"); // 空列表 → 永假
    }

    #[test]
    fn test_parse_not_in_empty() {
        let json = r#"["status", "not in", []]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "1=1"); // NOT IN 空 → 永真
    }

    #[test]
    fn test_parse_between() {
        let json = r#"["score", "between", [0, 100]]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "score BETWEEN ? AND ?");
        assert_eq!(sql.params, vec!["0", "100"]);
    }

    #[test]
    fn test_parse_ilike() {
        let json = r#"["title", "ilike", "%rust%"]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "title LIKE ? COLLATE NOCASE");
        assert_eq!(sql.params, vec!["%rust%"]);
    }

    #[test]
    fn test_parse_implicit_and() {
        let json = r#"[["status", "=", "published"], ["category", "=", "concept"]]"#;
        let domain = Domain::from_json(json).unwrap();
        assert!(matches!(domain, Domain::And(_)));
    }

    #[test]
    fn test_parse_compound() {
        let json = r#"["|", ["&", ["status", "=", "published"], ["category", "=", "concept"]], ["status", "=", "draft"]]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert!(sql.clause.contains("OR"));
        assert!(sql.clause.contains("AND"));
        assert_eq!(sql.params.len(), 3);
    }

    #[test]
    fn test_alias_prefix() {
        let json = r#"["status", "=", "published"]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(Some("e"));
        assert_eq!(sql.clause, "e.status = ?");
    }

    #[test]
    fn test_apply_to_query_with_where() {
        let domain = Domain::from_json(r#"["status", "=", "published"]"#).unwrap();
        let mut sql = String::from("SELECT * FROM entries WHERE collection_id = ?");
        let cond = domain.to_sql(None);
        cond.apply_to_query(&mut sql);
        assert_eq!(sql, "SELECT * FROM entries WHERE collection_id = ? AND (status = ?)");
    }

    #[test]
    fn test_apply_to_query_without_where() {
        let domain = Domain::from_json(r#"["status", "=", "published"]"#).unwrap();
        let mut sql = String::from("SELECT * FROM entries");
        let cond = domain.to_sql(None);
        cond.apply_to_query(&mut sql);
        assert_eq!(sql, "SELECT * FROM entries WHERE status = ?");
    }

    #[test]
    fn test_parse_errors() {
        assert!(Domain::from_json("[]").is_err());
        assert!(Domain::from_json(r#"["&", ["a", "=", "1"]]"#).is_err());
        assert!(Domain::from_json(r#"["!", ["a", "=", "1"], ["b", "=", "2"]]"#).is_err());
        assert!(Domain::from_json(r#"["a", "~~", "1"]"#).is_err());
        assert!(Domain::from_json(r#""hello""#).is_err());
    }

    #[test]
    fn test_parse_integer_and_float() {
        let json = r#"["score", ">", 42]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.params, vec!["42"]);

        let json2 = r#"["rate", ">=", 3.14]"#;
        let domain2 = Domain::from_json(json2).unwrap();
        let sql2 = domain2.to_sql(None);
        assert_eq!(sql2.params, vec!["3.14"]);
    }

    #[test]
    fn test_parse_not_equal_null() {
        let json = r#"["parent_id", "!=", null]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "parent_id IS NOT NULL");
    }

    #[test]
    fn test_not_in_with_values() {
        let json = r#"["status", "not in", ["deleted", "archived"]]"#;
        let domain = Domain::from_json(json).unwrap();
        let sql = domain.to_sql(None);
        assert_eq!(sql.clause, "NOT (status IN (?, ?))");
        assert_eq!(sql.params, vec!["deleted", "archived"]);
    }
}
