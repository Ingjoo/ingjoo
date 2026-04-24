# 开发指南

> 如何扩展和修改 ingjoo 框架。阅读本文前请先阅读 `ARCHITECTURE.md` 和 `CODE_STYLE.md`。

## 目录

1. [添加新的 Store Trait](#添加新的-store-trait)
2. [添加新的 HTTP Handler + 路由](#添加新的-http-handler--路由)
3. [使用 Domain DSL](#使用-domain-dsl)
4. [配置安全策略](#配置安全策略)
5. [测试模式](#测试模式)
6. [数据库迁移](#数据库迁移)
7. [错误处理模式](#错误处理模式)
8. [添加新的基础设施提供者](#添加新的基础设施提供者)

---

## 添加新的 Store Trait

Store trait 系统遵循固定模式。以添加 `NotificationStore` 为例：

### 第 1 步：在 `ingjoo-core/src/db/traits.rs` 中定义 trait

```rust
#[async_trait]
pub trait NotificationStore {
    async fn create_notification(&self, user_id: &UserId, message: &str) -> Result<()>;
    async fn get_unread(&self, user_id: &UserId) -> Result<Vec<Notification>>;
    async fn mark_read(&self, notification_id: i64) -> Result<()>;
}
```

### 第 2 步：加入 `ScaffStore` 聚合 trait

在同一文件中，将 `NotificationStore + Send + Sync` 加入 `ScaffStore` trait 定义。

### 第 3 步：在 `ingjoo-core/src/db/models.rs` 中定义模型

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Notification {
    pub id: i64,
    pub user_id: UserId,
    pub message: String,
    pub is_read: bool,
    pub created_at: String,
}
```

### 第 4 步：在 `ingjoo-infra/src/db/mod.rs` 中实现

添加 `impl NotificationStore for ScaffDb` 块，编写实际 SQL 查询。使用 `Dialect` 确保跨数据库兼容：

```rust
#[async_trait]
impl NotificationStore for ScaffDb {
    async fn create_notification(&self, user_id: &UserId, message: &str) -> Result<()> {
        let sql = "INSERT INTO notifications (user_id, message, is_read, created_at) VALUES (?, ?, 0, ?)";
        let sql = self.dialect.prepare_sql(sql);
        let now = self.dialect.now();
        sqlx::query(&sql)
            .bind(user_id.as_i64())
            .bind(message)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    // ... 其他方法
}
```

### 第 5 步：在 `ingjoo-infra/src/db/mock.rs` 中实现 Mock

添加 `impl NotificationStore for MockScaffDb`，用 `HashMap` 存储。

### 第 6 步：在 `run_migrations()` 中添加 DDL

在 `ingjoo-infra/src/db/mod.rs` 中添加 `CREATE TABLE IF NOT EXISTS notifications (...)` 语句。

### 第 7 步：编写测试

参见下方[测试模式](#测试模式)。

---

## 添加新的 HTTP Handler + 路由

> **注意**：HTTP 层尚未构建。本节描述基于现有架构的预期模式。

### Handler 模式

```rust
// crates/ingjoo-bin/src/handlers/notification.rs

use axum::{
    extract::{Extension, Path, Query},
    Json,
};
use ingjoo_core::db::traits::ScaffStore;
use ingjoo_core::db::models::*;
use ingjoo_infra::middleware::error::AppError;

pub async fn list_notifications(
    Extension(store): Extension<Arc<dyn ScaffStore>>,
    Extension(current_user): Extension<CurrentUser>,
) -> Result<Json<Vec<Notification>>, AppError> {
    let notifications = store.get_unread(&current_user.id).await?;
    Ok(Json(notifications))
}
```

### 路由注册

```rust
// crates/ingjoo-bin/src/routes.rs

use axum::{routing::get, Router};

pub fn notification_routes() -> Router {
    Router::new()
        .route("/api/notifications", get(list_notifications))
}
```

### 中间件栈

预期的中间件执行顺序（由内到外）：

```
请求 → CORS → Tracing → Auth(JWT提取) → Permission(check_access) → Handler
```

---

## 使用 Domain DSL

Domain DSL 是框架的查询过滤系统。将 Odoo 风格的过滤表达式解析为参数化 SQL。

### 语法参考

| 表达式 | 含义 |
|--------|------|
| `["field", "=", "value"]` | 精确匹配 |
| `["field", "!=", "value"]` | 不等于 |
| `["field", ">", "10"]` | 大于 |
| `["field", ">=", "10"]` | 大于等于 |
| `["field", "<", "10"]` | 小于 |
| `["field", "<=", "10"]` | 小于等于 |
| `["field", "like", "%test%"]` | 区分大小写 LIKE |
| `["field", "ilike", "%test%"]` | 不区分大小写 LIKE |
| `["field", "not like", "%spam%"]` | NOT LIKE |
| `["field", "not ilike", "%spam%"]` | NOT ILIKE |
| `["field", "in", ["a","b"]]` | IN 列表 |
| `["field", "not in", ["a","b"]]` | NOT IN 列表 |
| `["field", "is null", ""]` | IS NULL |
| `["field", "between", ["1","100"]]` | BETWEEN |
| `["field", "child_of", "123"]` | 预留（尚未实现） |

### 组合表达式

| 前缀 | 含义 |
|------|------|
| `"&"` | AND（多条件时的默认行为） |
| `"\|"` | OR |
| `"!"` | NOT |

### 使用示例

```rust
use ingjoo_core::query::domain::Domain;
use ingjoo_core::dialect::Dialect;

// 单条件
let d = Domain::parse(r#"[["status", "=", "active"]]"#)?;
let sql = d.apply_to_query("SELECT * FROM users WHERE 1=1", &Dialect::Sqlite);
// → "SELECT * FROM users WHERE 1=1 AND status = ?" 参数为 ["active"]

// AND 组合
let d = Domain::parse(r#"["&", ["status", "=", "active"], ["role", "in", ["admin","user"]]]"#)?;

// OR
let d = Domain::parse(r#"["|", ["role", "=", "admin"], ["status", "=", "active"]]"#)?;

// 嵌套
let d = Domain::parse(r#"["&", ["!", ["status", "=", "banned"]], ["role", "=", "user"]]"#)?;
```

### 使用表别名

```rust
let d = Domain::parse(r#"[["t.name", "=", "test"]]"#)?;
// 生成：t.name = ?
```

---

## 配置安全策略

安全引擎有 3 层。通过 `SecurityPolicy` 结构体或 JSON 配置。

### 第 1 层：模型级访问控制

控制哪些角色可以对哪些模型执行哪些操作。

```rust
use ingjoo_security::policy::{SecurityPolicy, ModelAccess, Operation};

let policy = SecurityPolicy {
    model_accesses: vec![
        ModelAccess {
            model: "entry".into(),
            role: "admin".into(),
            read: true, write: true, create: true, delete: true, import: true, export: true,
        },
        ModelAccess {
            model: "entry".into(),
            role: "viewer".into(),
            read: true, write: false, create: false, delete: false, import: false, export: false,
        },
    ],
    record_rules: vec![],
};
```

### 第 2 层：记录级过滤规则

基于 Domain 的行级过滤，按角色和操作类型。

```rust
use ingjoo_security::policy::RecordRule;

let rule = RecordRule {
    model: "entry".into(),
    role: "viewer".into(),
    domain: Domain::parse(r#"[["author_id", "=", "${user_id}"]]"#)?,
    perm_read: true,
    perm_write: false,
    perm_create: false,
    perm_delete: false,
};
```

### 第 3 层：集合隔离

自动注入 `collection_id IN (...)` 确保数据隔离。

```rust
let condition = policy.collection_isolation(&["col_1".into(), "col_2".into()], Some("t"));
// → t.collection_id IN (?, ?) 参数为 ["col_1", "col_2"]
```

### 从 JSON 加载

```rust
let policy = SecurityPolicy::from_policy_json(r#"{
    "model_accesses": [
        {"model": "entry", "role": "admin", "read": true, "write": true, "create": true, "delete": true, "import": true, "export": true}
    ],
    "record_rules": []
}"#)?;
```

---

## 测试模式

### 使用 MockScaffDb

```rust
#[cfg(test)]
mod tests {
    use ingjoo_infra::db::mock::MockScaffDb;

    #[tokio::test]
    async fn test_create_user() {
        let db = MockScaffDb::new();
        let req = RegisterRequest {
            email: "test@example.com".into(),
            password: "secret".into(),
            username: Some("testuser".into()),
            phone: None,
        };
        let user = db.register_user(req).await.unwrap();
        assert_eq!(user.email, "test@example.com");
    }
}
```

### Domain DSL 测试

```rust
#[test]
fn test_filter() {
    let domain = Domain::parse(r#"[["status", "=", "active"]]"#).unwrap();
    let condition = domain.apply_to_query(
        "SELECT * FROM users WHERE 1=1",
        &Dialect::Sqlite,
    );
    assert!(condition.sql.contains("status = ?"));
    assert_eq!(condition.params, vec!["active"]);
}
```

### 安全策略测试

```rust
#[test]
fn test_access_control() {
    let policy = SecurityPolicy { /* ... */ };
    assert!(policy.check_access("entry", "admin", "delete"));
    assert!(!policy.check_access("entry", "viewer", "delete"));
}
```

---

## 数据库迁移

### 当前方案（Ad-hoc）

迁移在 `ScaffDb::run_migrations()` 中内联执行：

```rust
pub async fn run_migrations(&self) -> Result<()> {
    sqlx::query("CREATE TABLE IF NOT EXISTS users (...)").execute(&self.pool).await?;
    // 增量：ALTER TABLE ADD COLUMN（错误被静默忽略）
    let _ = sqlx::query("ALTER TABLE users ADD COLUMN phone TEXT").execute(&self.pool).await;
    Ok(())
}
```

**问题**：无版本号、无回滚、ALTER TABLE 静默失败。

### 计划方案（版本化）

参见 `ROADMAP.md` T2.4。目标结构：

```
migrations/
├── 001_init_users.sql
├── 002_add_phone_column.sql
└── 003_create_settings.sql
```

配合 `_migration_versions` 版本追踪表。

### Dialect 感知的 DDL

编写 DDL 时始终使用 `self.dialect`：

```rust
// 时间戳函数
let now = self.dialect.now();  // SQLite: datetime('now'), PG: CURRENT_TIMESTAMP

// 占位符风格
let sql = self.dialect.prepare_sql("SELECT * FROM users WHERE id = ?");  // PG: $1

// 自增主键
let pk = self.dialect.serial_pk("id");  // SQLite: INTEGER PRIMARY KEY AUTOINCREMENT, PG: SERIAL PRIMARY KEY
```

---

## 错误处理模式

### 现状（3 种策略共存）

| 位置 | 策略 | 问题 |
|------|------|------|
| Store trait | `anyhow::Result` | 调用方无法模式匹配错误 |
| ScopeGuard | `thiserror::ScopeError` | 正确 |
| SecurityPolicy | 手写 `impl Display + Error` | 不一致 |
| HTTP 层 | `AppError` 枚举 + `IntoResponse` | 正确 |

### 新代码推荐模式

在 trait 边界使用类型化错误，内部实现可用 anyhow：

```rust
// 在 ingjoo-core/src/db/errors.rs（新文件）
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("记录未找到: {model} {id}")]
    NotFound { model: String, id: String },
    #[error("唯一约束冲突: {field}={value}")]
    UniqueViolation { field: String, value: String },
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),
}

// Trait 使用类型化错误
#[async_trait]
pub trait UserStore {
    async fn get_user(&self, id: &UserId) -> Result<User, StoreError>;
}

// 实现可内部用 anyhow，在边界转换
impl UserStore for ScaffDb {
    async fn get_user(&self, id: &UserId) -> Result<User, StoreError> {
        sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(id.as_i64())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => StoreError::NotFound {
                    model: "user".into(),
                    id: id.to_string(),
                },
                other => StoreError::Database(other),
            })
    }
}
```

---

## 添加新的基础设施提供者

基础设施提供者通过 feature flag 控制。以添加 Redis 队列提供者为例。

### 第 1 步：在 `ingjoo-infra/Cargo.toml` 中添加 feature flag

```toml
[features]
queue-redis = ["redis"]
```

### 第 2 步：创建模块

```rust
// crates/ingjoo-infra/src/queue/redis.rs
#[cfg(feature = "queue-redis")]
pub struct RedisQueue { /* ... */ }
```

### 第 3 步：在 `mod.rs` 中注册

```rust
// crates/ingjoo-infra/src/queue/mod.rs
#[cfg(feature = "queue-redis")]
pub mod redis;
```

### 第 4 步：实现 `ingjoo-core` 中定义的 trait

trait 应定义在 `ingjoo-core` 中，具体实现在 `ingjoo-infra` 中通过 feature flag 控制。
