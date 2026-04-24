# CODE_STYLE.md — ingjoo (莺竹框架)

## 命名约定

### Crate 命名

| 规则 | 示例 |
|------|------|
| `ingjoo-{module}` (kebab-case) | `ingjoo-core`, `ingjoo-security`, `ingjoo-infra` |

### 类型命名

| 类别 | 约定 | 示例 |
|------|------|------|
| Struct | PascalCase | `User`, `Db`, `JwtAuthProvider` |
| Enum | PascalCase + PascalCase 变体 | `AppError::NotFound`, `Dialect::Sqlite` |
| Trait | PascalCase，能力描述 | `UserStore`, `AuthProvider`, `ScopeGuard` |
| 类型别名 | PascalCase | `type Pool = sqlx::pool::Pool<sqlx::Any>` |
| Config struct | `{Name}Config` | `AuthConfig`, `SmsConfig`, `ScaffConfig` |
| Provider struct | `{Tech}{Provider}` | `JwtAuthProvider`, `TencentSmsProvider` |
| ID 类型 | `{Entity}Id` (via `define_id!`) | `UserId` |
| 请求 DTO | `{Verb}{Entity}` | `RegisterRequest`, `CreateAttachment`, `SetModuleSetting` |
| 错误 enum | `{Name}Error` | `AppError`, `ScopeError`, `SecurityError` |

### 函数/方法命名

| 规则 | 示例 |
|------|------|
| snake_case | `create_user`, `hash_password`, `run_migrations` |
| 查询前缀: `get_`, `list_`, `search_` | `get_user_by_email`, `list_users`, `search_users` |
| 变更前缀: `create_`, `update_`, `delete_`, `set_` | `update_user_role`, `delete_attachment` |
| 状态变更: `mark_` | `mark_captcha_used`, `mark_password_reset_used` |

### 常量

| 规则 | 示例 |
|------|------|
| SCREAMING_SNAKE_CASE | `DEFAULT_TTL_SECS`, `SCOPE_HIERARCHY_4` |

### 布尔字段

| 规则 | 示例 |
|------|------|
| 直接动词，无 `is_` 前缀 | `read`, `write`, `create`, `delete`, `import`, `export` |

## 文件组织

### 模块结构

- 每个**存储能力**一个文件：`ids.rs` (ID), `models.rs` (模型), `traits.rs` (trait)
- 子模块用 `mod.rs` 做入口 + 功能文件：`scope/mod.rs` + `scope/guard.rs`
- `lib.rs` 负责 barrel re-export — 消费者只需 `use ingjoo_core::User`

### 文件职责边界

| 文件 | 内容 |
|------|------|
| `ids.rs` | 仅 ID 类型定义（`define_id!` 调用） |
| `models.rs` | 数据模型 + DTO + `impl From` 转换 |
| `traits.rs` | 仅 trait 定义 + blanket impl |
| `mod.rs` (如 `db/mod.rs`) | 具体 sqlx 实现 + trait impl |
| `lib.rs` | `pub mod` 声明 + `pub use` re-export |

## Import 风格

### 1. 逐行 use（不聚合）

```rust
// ✅ 正确
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

// ❌ 不用
use axum::{http::StatusCode, response::{IntoResponse, Response}};
```

### 2. crate 内用 `super::` / `crate::`

```rust
use super::ids::UserId;          // 父模块
use crate::PaginatedResult;      // crate 根 re-export
use crate::db::traits::ModuleSettingStore;
```

### 3. 局部 use（函数内按需导入）

```rust
fn refresh_token_hash(&self, token: &str) -> String {
    use sha2::{Sha256, Digest};  // 仅在需要时导入
    // ...
}
```

### 4. 类型别名简化长路径

```rust
pub type Pool = sqlx::pool::Pool<sqlx::Any>;
pub type AuthUtil = JwtAuthProvider;
type HmacSha256 = Hmac<Sha256>;
```

## 代码模式

### Store Trait 模式

```rust
// 1. 定义原子 trait（core 层）
#[async_trait]
pub trait UserStore: Send + Sync {
    async fn create_user(&self, user: &User) -> Result<User>;
    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>>;
}

// 2. 聚合 trait + blanket impl
#[async_trait]
pub trait ScaffStore: UserStore + TokenStore + ... + Send + Sync {}
impl<T> ScaffStore for T where T: UserStore + TokenStore + ... + Send + Sync {}
```

### Provider / 策略模式

```rust
// 1. 定义 trait
pub trait AuthProvider: Send + Sync {
    fn hash_password(&self, password: &str) -> Result<String>;
}

// 2. 具体实现
pub struct JwtAuthProvider { encoding_key: EncodingKey, ... }
impl AuthProvider for JwtAuthProvider { /* ... */ }

// 3. 默认别名
pub type AuthUtil = JwtAuthProvider;
```

### DB 查询模式

```rust
// 统一模式: self.sql() → query_as → bind → fetch
sqlx::query_as::<_, User>(
    &self.sql("SELECT * FROM users WHERE email = ?")
)
.bind(email)
.fetch_optional(&self.pool)
.await
.map_err(Into::into)

// 分页查询
let total: i64 = sqlx::query_scalar(&self.sql("SELECT COUNT(*) FROM users"))
    .fetch_one(&self.pool).await?;
let items = sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users LIMIT ? OFFSET ?"))
    .bind(limit).bind(offset).fetch_all(&self.pool).await?;
Ok(PaginatedResult::new(items, total, limit, offset))
```

### Feature-Gated 模块导出

```rust
// lib.rs
#[cfg(feature = "auth")]
pub mod auth;

#[cfg(feature = "db")]
pub use db::Db as ScaffDb;

#[cfg(all(feature = "db", feature = "mock"))]
pub use db::mock::MockScaffDb;
```

### 类型安全 ID 宏

```rust
// 新 ID 类型只需一行
define_id!(UserId);
// 自动获得: Display, From<String>, From<&str>, Deref<Target=str>, sqlx::Type(transparent)
```

## 数据模型约定

### 三类 derive 组合

| 模型类型 | derive | 用途 |
|---------|--------|------|
| DB 模型 | `Serialize + Deserialize + sqlx::FromRow` | 完整数据库行映射 |
| 响应 DTO | `Serialize`（不 Deserialize） | 排除敏感字段 |
| 请求 DTO | 仅 `Deserialize` | 输入验证 |

### DTO 分离

```rust
// DB 模型 — 包含敏感字段
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub password_hash: Option<String>,  // 敏感
    // ...
}

// 响应 DTO — 排除敏感字段
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserPublic {
    // 没有 password_hash
}

// 转换 impl
impl From<&User> for UserPublic {
    fn from(u: &User) -> Self { Self { id: u.id.clone(), ... } }
}
```

### 可选字段

- 一律 `Option<T>`，DB 模型和 DTO 都是
- DB 查询用 `COALESCE(?, column)` 处理部分更新

## 错误处理

### 三轨制

| 场景 | 工具 | 返回类型 |
|------|------|---------|
| HTTP 层 | `AppError` enum + `IntoResponse` | `AppError` |
| 领域层 | `thiserror` enum | `Result<T, SpecificError>` |
| 通用/IO | `anyhow` | `anyhow::Result<T>` |

### AppError → HTTP 状态码映射

```rust
pub enum AppError {
    Internal(anyhow::Error),  // → 500
    NotFound(String),         // → 404
    Unauthorized(String),     // → 401
    Forbidden(String),        // → 403
    BadRequest(String),       // → 400
}

// Blanket From — 任何错误都能 ? 转成 AppError::Internal
impl<E: Into<anyhow::Error>> From<E> for AppError { ... }
```

### 错误消息语言

- 错误消息用**中文**：`anyhow!("密码哈希失败: {}", e)`
- 用户可见消息混合：`"Not found: {}"`, `"角色 '{}' 无权对 '{}' 执行 {} 操作"`

### Trait 方法返回类型

- **通用 trait 方法**统一返回 `anyhow::Result<T>`
- **领域特定 trait** 返回 `Result<T, SpecificError>`

## 异步模式

| 场景 | 方式 |
|------|------|
| DB/IO trait | `#[async_trait]` + `: Send + Sync` |
| 纯计算 trait | 同步，不用 `#[async_trait]` |
| 异步测试 | `#[tokio::test]` |

## 测试

### 结构

- 所有测试在源文件底部 `#[cfg(test)] mod tests { ... }`
- 无独立 `tests/` 目录，无集成测试
- Mock 通过手写 struct + `#[async_trait]` impl（不用 mockall）

### 命名

- 描述性 `snake_case`：`test_admin_full_access`, `test_viewer_entry_filter`

### 模式

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple() { /* 同步 */ }

    #[tokio::test]
    async fn test_async() { /* 异步 */ }
}
```

## SQL 约定

- **手写 SQL**，不使用 ORM
- SQL 以 SQLite 语法编写，通过 `self.sql()` (即 `Dialect::prepare()`) 转换为 PostgreSQL
- 占位符统一用 `?`（Dialect 自动替换为 `$1, $2...`）
- 时间函数统一写 `datetime('now')`（Dialect 自动转换）
- 错误转换统一用 `.map_err(Into::into)` 或 `?`
- 分页返回 `PaginatedResult<T>`

## 注释和文档

- 模块级文档用 `//!`（如 `query/domain.rs` 的 Domain DSL 说明）
- 公共 API 无需逐项 doc comment（框架处于 0.1.0 早期阶段）
- 代码中偶有行内注释解释关键逻辑

## Do's and Don'ts

### ✅ Do

- 新存储能力：定义 trait → 加入 `ScaffStore` 约束 → 在 `Db` 上实现
- 新 Provider：定义 trait → 实现具体 struct → 用 `pub type` 提供默认别名
- 新 ID 类型：`define_id!(EntityId);`
- 新数据模型：`Serialize + Deserialize + sqlx::FromRow`
- 请求 DTO 仅 derive `Deserialize`
- 错误处理链：`anyhow` → blanket `From` → `AppError`
- Feature 门控新模块：`#[cfg(feature = "...")]` 在 `mod` 声明和 `pub use` 上

### ❌ Don't

- 不要用 ORM — 全部手写 SQL
- 不要在 core 层写具体实现 — core 只定义 trait 和类型
- 不要混用错误类型 — HTTP 层 `AppError`，领域层 `thiserror`，通用 `anyhow`
- 不要忽略 `Dialect` — 所有 SQL 必须通过 `self.sql()` 处理
- 不要在 DTO 中暴露敏感字段（如 `password_hash`）
- 不要聚合 use 语句 — 逐行写
- 不要用 mockall — 手写 mock struct
