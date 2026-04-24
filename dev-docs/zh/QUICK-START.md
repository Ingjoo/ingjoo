# 快速上手指南

## 环境要求

| 依赖 | 版本 | 说明 |
|------|------|------|
| Rust | 1.75+ | `rustup update stable` |
| SQLite | 3.x | 默认开发数据库 |
| PostgreSQL | 15+ | 可选，用于类生产环境测试 |

## 构建

```bash
# 在 workspace 根目录（source/ingjoo/）
cargo build
```

## 测试

```bash
# 运行全部测试（85 个，覆盖 3 个 crate）
cargo test

# 测试指定 crate
cargo test -p ingjoo-core
cargo test -p ingjoo-security
cargo test -p ingjoo-cache

# 注意：ingjoo-infra 当前零测试
```

## 数据库配置

### SQLite（默认，零配置）

```bash
export DATABASE_URL="sqlite:./dev.db"
```

框架启动时通过 `run_migrations()` 自动建表，无需手动 schema。

### PostgreSQL

```bash
export DATABASE_URL="postgres://user:pass@localhost:5432/ingjoo_dev"
```

Dialect 系统自动从 URL scheme（`sqlite:` vs `postgres:`）检测数据库类型。

## 运行

```bash
cargo run -p ingjoo-bin
```

**当前行为**：连接数据库、运行迁移、绑定 3000 端口，但**不启动 HTTP 服务**。该二进制仅为骨架——参见 `ROADMAP.md` 阶段 1 的构建计划。

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `DATABASE_URL` | 必填 | 数据库连接字符串 |
| `PORT` | `3000` | 服务绑定端口 |
| `RUST_LOG` | `info` | 日志级别（`debug`、`trace`、`warn`、`error`） |

## Crate Feature Flags

### ingjoo-infra

```toml
# 默认：DB + Auth
ingjoo-infra = { path = "crates/ingjoo-infra" }

# 全部功能：DB + Auth + Email + SMS + Captcha + S3 Storage
ingjoo-infra = { path = "crates/ingjoo-infra", features = ["full"] }

# Mock 内存数据库（测试用）
ingjoo-infra = { path = "crates/ingjoo-infra", features = ["mock"] }
```

| Feature | 启用内容 |
|---------|---------|
| `db`（默认） | `ScaffDb` — SQLite/Postgres 实现 |
| `auth`（默认） | `JwtAuthProvider` — JWT + Argon2 |
| `email` | SMTP 邮件（`lettre`） |
| `sms` | 腾讯云短信 |
| `captcha` | 图片验证码生成 |
| `s3` | AWS S3 文件存储 |
| `mock` | `MockScaffDb` — 内存测试替身 |

## 项目结构

```
ingjoo/
├── Cargo.toml              # Workspace 根
├── ARCHITECTURE.md          # 完整架构文档
├── CODE_STYLE.md            # 编码规范
├── dev-docs/                # 开发文档
│   ├── en/                  # 英文（AI 优先）
│   └── zh/                  # 中文
└── crates/
    ├── ingjoo-core/         # Trait、模型、DSL、Dialect
    ├── ingjoo-infra/        # 具体实现（DB、认证、存储）
    ├── ingjoo-security/     # 三层 RBAC 引擎
    ├── ingjoo-cache/        # Moka 缓存
    ├── ingjoo-bin/          # HTTP 服务入口（骨架）
    └── ingjoo-macros/       # 派生宏（空壳）
```

## 当前可用功能

| 组件 | 状态 | 使用方式 |
|------|------|---------|
| Domain DSL | ✅ 可用 | `Domain::parse(r#"[["name", "=", "test"]]"#)` → SQL 条件 |
| Dialect 抽象 | ✅ 可用 | `Dialect::Sqlite` / `Dialect::Postgres` 处理 DDL + 占位符转换 |
| Store trait | ✅ 可用 | `Arc<dyn ScaffStore>` — 8 个异步数据访问 trait |
| 安全策略引擎 | ✅ 可用 | `SecurityPolicy::check_access(model, role, op)` |
| 缓存 | ✅ 可用 | `FrameworkCache::new()` — 作用域、用户、设置缓存 |
| JWT 认证 | ✅ 可用 | `JwtAuthProvider::new(config)` — 哈希/验证/创建 token |
| Mock DB | ✅ 可用 | `MockScaffDb::new()` — 完整内存实现 |
| HTTP 服务 | ❌ 未实现 | `ingjoo-bin` 无 axum 路由 |
| 动态模型注册 | ❌ 未实现 | `module/mod.rs` 仅 12 行 |
| 派生宏 | ❌ 未实现 | `ingjoo-macros` 为空 |

## 开发工作流

```bash
# 1. 修改某个 crate
vim crates/ingjoo-core/src/query/domain.rs

# 2. 运行该 crate 的测试
cargo test -p ingjoo-core

# 3. 检查警告
cargo clippy -p ingjoo-core -- -D warnings

# 4. 检查格式
cargo fmt --check

# 5. 全量构建
cargo build
```

## 常见任务

### 添加新的 Store trait 方法

1. 在 `ingjoo-core/src/db/traits.rs` 的对应 trait 中添加方法签名
2. 在 `ingjoo-infra/src/db/mod.rs` 中实现（真实 DB）
3. 在 `ingjoo-infra/src/db/mock.rs` 中实现（Mock）
4. 添加测试

### 使用 Domain DSL

```rust
use ingjoo_core::query::domain::Domain;

// 简单过滤
let domain = Domain::parse(r#"[["status", "=", "active"]]"#)?;
let condition = domain.apply_to_query("SELECT * FROM users", &Dialect::Sqlite);

// 组合表达式
let domain = Domain::parse(r#"["&", ["status", "=", "active"], ["role", "in", ["admin", "manager"]]]"#)?;
```

### 使用安全策略

```rust
use ingjoo_security::policy::{SecurityPolicy, ModelAccess, RecordRule};

let policy = SecurityPolicy {
    model_accesses: vec![ModelAccess {
        model: "entry".into(),
        role: "viewer".into(),
        read: true, write: false, create: false, delete: false, import: false, export: false,
    }],
    record_rules: vec![],
};

assert!(policy.check_access("entry", "viewer", "read"));
assert!(!policy.check_access("entry", "viewer", "write"));
```
