# Development Guide

> How to extend and modify the ingjoo framework. This guide assumes you've read `ARCHITECTURE.md` and `CODE_STYLE.md`.

## Table of Contents

1. [Adding a New Store Trait](#adding-a-new-store-trait)
2. [Adding a New HTTP Handler + Route](#adding-a-new-http-handler--route)
3. [Using the Domain DSL](#using-the-domain-dsl)
4. [Configuring Security Policies](#configuring-security-policies)
5. [Testing Patterns](#testing-patterns)
6. [Database Migrations](#database-migrations)
7. [Error Handling Patterns](#error-handling-patterns)
8. [Adding a New Infrastructure Provider](#adding-a-new-infrastructure-provider)

---

## Adding a New Store Trait

The Store trait system follows a specific pattern. Here's how to add a new one (e.g., `NotificationStore`):

### Step 1: Define the trait in `ingjoo-core/src/db/traits.rs`

```rust
#[async_trait]
pub trait NotificationStore {
    async fn create_notification(&self, user_id: &UserId, message: &str) -> Result<()>;
    async fn get_unread(&self, user_id: &UserId) -> Result<Vec<Notification>>;
    async fn mark_read(&self, notification_id: i64) -> Result<()>;
}
```

### Step 2: Add to the `ScaffStore` composite trait

In the same file, add `NotificationStore + Send + Sync` to the `ScaffStore` trait definition.

### Step 3: Define models in `ingjoo-core/src/db/models.rs`

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

### Step 4: Implement in `ingjoo-infra/src/db/mod.rs`

Add the `impl NotificationStore for ScaffDb` block with actual SQL queries. Use `Dialect` for cross-database compatibility:

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
    // ... other methods
}
```

### Step 5: Implement in `ingjoo-infra/src/db/mock.rs`

Add `impl NotificationStore for MockScaffDb` with `HashMap`-backed storage.

### Step 6: Add DDL in `run_migrations()`

In `ingjoo-infra/src/db/mod.rs`, add the `CREATE TABLE IF NOT EXISTS notifications (...)` statement.

### Step 7: Write tests

See [Testing Patterns](#testing-patterns) below.

---

## Adding a New HTTP Handler + Route

> **Note**: The HTTP layer is not yet built. This section describes the intended pattern based on the existing architecture.

### Handler Pattern

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

### Route Registration

```rust
// crates/ingjoo-bin/src/routes.rs

use axum::{routing::get, Router};

pub fn notification_routes() -> Router {
    Router::new()
        .route("/api/notifications", get(list_notifications))
}
```

### Middleware Stack

The intended middleware order (innermost to outermost):

```
Request → CORS → Tracing → Auth(JWT extraction) → Permission(check_access) → Handler
```

---

## Using the Domain DSL

The Domain DSL is the framework's query filter system. It parses Odoo-style filter expressions into parameterized SQL.

### Syntax Reference

| Expression | Meaning |
|------------|---------|
| `["field", "=", "value"]` | Exact match |
| `["field", "!=", "value"]` | Not equal |
| `["field", ">", "10"]` | Greater than |
| `["field", ">=", "10"]` | Greater than or equal |
| `["field", "<", "10"]` | Less than |
| `["field", "<=", "10"]` | Less than or equal |
| `["field", "like", "%test%"]` | Case-sensitive LIKE |
| `["field", "ilike", "%test%"]` | Case-insensitive LIKE |
| `["field", "not like", "%spam%"]` | NOT LIKE |
| `["field", "not ilike", "%spam%"]` | NOT ILIKE |
| `["field", "in", ["a","b"]]` | IN list |
| `["field", "not in", ["a","b"]]` | NOT IN list |
| `["field", "is null", ""]` | IS NULL |
| `["field", "between", ["1","100"]]` | BETWEEN |
| `["field", "child_of", "123"]` | Reserved (not yet implemented) |

### Compound Expressions

| Prefix | Meaning |
|--------|---------|
| `"&"` | AND (default when multiple filters) |
| `"\|"` | OR |
| `"!"` | NOT |

### Usage Example

```rust
use ingjoo_core::query::domain::Domain;
use ingjoo_core::dialect::Dialect;

// Single condition
let d = Domain::parse(r#"[["status", "=", "active"]]"#)?;
let sql = d.apply_to_query("SELECT * FROM users WHERE 1=1", &Dialect::Sqlite);
// → "SELECT * FROM users WHERE 1=1 AND status = ?" with params ["active"]

// AND of two conditions
let d = Domain::parse(r#"["&", ["status", "=", "active"], ["role", "in", ["admin","user"]]]"#)?;

// OR
let d = Domain::parse(r#"["|", ["role", "=", "admin"], ["status", "=", "active"]]"#)?;

// Nested
let d = Domain::parse(r#"["&", ["!", ["status", "=", "banned"]], ["role", "=", "user"]]"#)?;
```

### With Table Alias

```rust
let d = Domain::parse(r#"[["t.name", "=", "test"]]"#)?;
// Generates: t.name = ?
```

---

## Configuring Security Policies

The security engine has 3 layers. Configure them via `SecurityPolicy` struct or JSON.

### Layer 1: Model-Level Access

Controls which roles can perform which operations on which models.

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

### Layer 2: Record-Level Rules

Domain-based row filtering per role and operation.

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

### Layer 3: Collection Isolation

Auto-injects `collection_id IN (...)` to ensure data isolation.

```rust
let condition = policy.collection_isolation(&["col_1".into(), "col_2".into()], Some("t"));
// → t.collection_id IN (?, ?) with params ["col_1", "col_2"]
```

### From JSON

```rust
let policy = SecurityPolicy::from_policy_json(r#"{
    "model_accesses": [
        {"model": "entry", "role": "admin", "read": true, "write": true, "create": true, "delete": true, "import": true, "export": true}
    ],
    "record_rules": []
}"#)?;
```

---

## Testing Patterns

### Using MockScaffDb

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

### Domain DSL Testing

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

### Security Policy Testing

```rust
#[test]
fn test_access_control() {
    let policy = SecurityPolicy { /* ... */ };
    assert!(policy.check_access("entry", "admin", "delete"));
    assert!(!policy.check_access("entry", "viewer", "delete"));
}
```

---

## Database Migrations

### Current Approach (Ad-hoc)

Migrations run inline in `ScaffDb::run_migrations()`:

```rust
pub async fn run_migrations(&self) -> Result<()> {
    sqlx::query("CREATE TABLE IF NOT EXISTS users (...)").execute(&self.pool).await?;
    // Incremental: ALTER TABLE ADD COLUMN (errors silently ignored)
    let _ = sqlx::query("ALTER TABLE users ADD COLUMN phone TEXT").execute(&self.pool).await;
    Ok(())
}
```

**Problems**: No versioning, no rollback, silent failures on ALTER TABLE.

### Planned Approach (Versioned)

See ROADMAP.md T2.4. The target is:

```
migrations/
├── 001_init_users.sql
├── 002_add_phone_column.sql
└── 003_create_settings.sql
```

With a `_migration_versions` tracking table.

### Dialect-Aware DDL

Always use `self.dialect` when writing DDL:

```rust
// Timestamp function
let now = self.dialect.now();  // SQLite: datetime('now'), PG: CURRENT_TIMESTAMP

// Placeholder style
let sql = self.dialect.prepare_sql("SELECT * FROM users WHERE id = ?");  // PG: $1

// Serial primary key
let pk = self.dialect.serial_pk("id");  // SQLite: INTEGER PRIMARY KEY AUTOINCREMENT, PG: SERIAL PRIMARY KEY
```

---

## Error Handling Patterns

### Current State (3 strategies coexist)

| Location | Strategy | Problem |
|----------|----------|---------|
| Store traits | `anyhow::Result` | Callers can't pattern-match errors |
| ScopeGuard | `thiserror::ScopeError` | Correct |
| SecurityPolicy | Manual `impl Display + Error` | Inconsistent |
| HTTP layer | `AppError` enum + `IntoResponse` | Correct for HTTP |

### Recommended Pattern for New Code

Use typed errors at trait boundaries, `anyhow` for internal implementation:

```rust
// In ingjoo-core/src/db/errors.rs (new file)
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("记录未找到: {model} {id}")]
    NotFound { model: String, id: String },
    #[error("唯一约束冲突: {field}={value}")]
    UniqueViolation { field: String, value: String },
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),
}

// Trait uses typed error
#[async_trait]
pub trait UserStore {
    async fn get_user(&self, id: &UserId) -> Result<User, StoreError>;
}

// Implementation can use anyhow internally, convert at boundary
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

## Adding a New Infrastructure Provider

Infrastructure providers are feature-gated in `ingjoo-infra`. Example: adding a Redis queue provider.

### Step 1: Add feature flag in `ingjoo-infra/Cargo.toml`

```toml
[features]
queue-redis = ["redis"]
```

### Step 2: Create the module

```rust
// crates/ingjoo-infra/src/queue/redis.rs
#[cfg(feature = "queue-redis")]
pub struct RedisQueue { /* ... */ }
```

### Step 3: Register in `mod.rs`

```rust
// crates/ingjoo-infra/src/queue/mod.rs
#[cfg(feature = "queue-redis")]
pub mod redis;
```

### Step 4: Implement the trait from `ingjoo-core`

The trait should be defined in `ingjoo-core`, with the concrete impl in `ingjoo-infra` behind the feature flag.
