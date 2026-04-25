# ARCHITECTURE.md — ingjoo (莺竹框架)

## 概述

**ingjoo** 是一个用 Rust 编写的开源后端框架，提供动态模型注册、三层权限引擎、Domain DSL 查询语言和可插拔基础设施，用于构建多租户业务系统。灵感源自 Odoo 的权限与配置体系。

## 技术栈

| 层面 | 技术 | 版本 |
|------|------|------|
| 语言 | Rust (Edition 2021) | - |
| 异步运行时 | tokio | 1.x (full features) |
| HTTP 框架 | axum | 0.8 (multipart + websocket) |
| HTTP 中间件 | tower + tower-http | 0.5 / 0.6 |
| 数据库 | sqlx (SQLite + PostgreSQL) | 0.8 |
| 序列化 | serde + serde_json | 1.x |
| 认证 | jsonwebtoken + argon2 | 9 / 0.5 |
| 缓存 | moka | 0.12 |
| 邮件 | lettre | 0.11 |
| 对象存储 | aws-sdk-s3 | 1.x |
| 日志 | tracing + tracing-subscriber | 0.1 / 0.3 |
| 错误处理 | anyhow + thiserror | 1 / 2 |
| ID 生成 | uuid (v4) | 1.x |

## 目录结构

```
ingjoo/
├── Cargo.toml                  # Workspace 根配置
├── Cargo.lock
├── README.md
└── crates/
    ├── ingjoo-core/            # 核心抽象库 (trait + 类型定义)
    │   └── src/
    │       ├── lib.rs          #   公共 API barrel re-export
    │       ├── dialect.rs      #   SQL 方言抽象 (SQLite/PostgreSQL)
    │       ├── pool.rs         #   连接池抽象
    │       ├── config/
    │       │   └── cascade.rs  #   级联配置 (系统默认 → 集合覆盖)
    │       ├── db/
    │       │   ├── ids.rs      #   类型安全 ID (define_id! 宏)
    │       │   ├── models.rs   #   数据模型 + DTO
    │       │   └── traits.rs   #   Store trait 层次结构
    │       ├── module/
    │       │   └── mod.rs      #   动态模型注册表
    │       ├── query/
    │       │   └── domain.rs   #   Domain DSL → SQL 编译器
    │       └── scope/
    │           ├── mod.rs      #   作用域管理
    │           └── guard.rs    #   作用域守卫 (多租户/集合隔离)
    │
    ├── ingjoo-security/        # 三层权限引擎
    │   └── src/
    │       ├── lib.rs
    │       └── policy.rs       #   模型级/记录级/集合级权限
    │
    ├── ingjoo-cache/           # 缓存封装 (独立，无 ingjoo 依赖)
    │   └── src/
    │       ├── lib.rs
    │       └── moka_cache.rs   #   Moka 同步缓存适配
    │
    ├── ingjoo-infra/           # 基础设施参考实现
    │   └── src/
    │       ├── lib.rs          #   Feature-gated 模块导出
    │       ├── config.rs       #   基础设施配置
    │       ├── state.rs        #   AppState（含 14 个扩展 trait + cache）
    │       ├── router.rs       #   axum 路由 (CORS + Trace + 安全头)
    │       ├── auth/mod.rs     #   JWT 认证 + Argon2 密码
    │       ├── db/
    │       │   ├── mod.rs      #   Store trait 的 sqlx 实现
    │       │   ├── database_manager.rs  #   多数据库管理（DatabaseManager）
    │       │   ├── mock.rs     #   Mock 数据库 (测试用)
    │       │   └── models.rs   #   Re-export core 模型
    │       ├── email/mod.rs    #   SMTP 邮件发送
    │       ├── sms/mod.rs      #   短信发送 (腾讯云)
    │       ├── storage/mod.rs  #   文件存储 (本地/S3)
    │       ├── captcha/mod.rs  #   图片验证码
    │       ├── handlers/
    │       │   ├── auth.rs     #   认证 handler (注册/登录/刷新/资料)
    │       │   ├── crud.rs     #   通用 CRUD handler (动态模型)
    │       │   ├── database.rs #   数据库管理 handler
    │       │   ├── attachment.rs # 附件上传 handler
    │       │   ├── menu.rs     #   菜单管理 handler
    │       │   ├── view.rs     #   视图管理 handler
    │       │   ├── action.rs   #   动作管理 handler
    │       │   ├── schedule.rs #   定时任务 handler
    │       │   └── permission.rs # 权限管理 handler
    │       └── middleware/
    │           ├── mod.rs
    │           ├── database_selector.rs  #   多数据库选择中间件
    │           ├── security.rs  #   安全策略中间件
    │           └── error.rs   #   AppError → HTTP 状态码映射
    │
    ├── ingjoo-macros/          # 过程宏 (预留，当前为空壳)
    │   └── src/lib.rs
    │
    └── ingjoo-bin/             # 可执行二进制入口
        └── src/
            └── main.rs         #   启动: 连接 DB → 迁移 → 监听 :3000
```

## 依赖关系图

```
ingjoo-macros (独立 proc-macro，零依赖)
ingjoo-cache  (独立，仅依赖 moka)

ingjoo-core ──────────────────────────────────────┐
    │                                              │
    ├──→ ingjoo-security (三层权限引擎)             │
    │                                              │
    └──→ ingjoo-infra (基础设施实现)                │
              │                                    │
              └──→ ingjoo-bin (可执行入口)          │
                                                   │
              ingjoo-infra 也依赖 ingjoo-core ─────┘
```

**关键规则**：`ingjoo-core` 是整个依赖图的根——除 `ingjoo-cache`（独立缓存封装）和 `ingjoo-macros`（独立过程宏）外，所有 crate 都直接或间接依赖它。

## 核心组件

### 1. ingjoo-core — 核心抽象

定义框架的核心类型和 trait，**不含具体实现**，是整个依赖图的根节点。

| 模块 | 职责 |
|------|------|
| `db/traits.rs` | Store trait 层次结构：`UserStore`, `TokenStore`, `IngjooStore` 等 8 个原子 trait + 1 个聚合 trait + `GroupStore`/`AccessStore` |
| `db/models.rs` | 数据模型 (`User`, `Attachment`, `ModuleSetting`) + DTO (`UserPublic`, `RegisterRequest`) |
| `db/ids.rs` | `define_id!` 宏 — 类型安全 ID（`UserId` 等） |
| `query/domain.rs` | Domain DSL — 13 种操作符的声明式查询表达式 → SQL WHERE 编译器 |
| `scope/guard.rs` | 作用域守卫 — 角色层级比较 (`role_gte`) + 多租户隔离 |
| `config/cascade.rs` | 级联配置 — `系统默认 → 集合覆盖` 的设置优先级 |
| `dialect.rs` | SQL 方言抽象 — SQLite 与 PostgreSQL 差异透明化 |
| `module/mod.rs` | 动态模型注册表 — 运行时定义业务模型 |
| `pool.rs` | 连接池 — 自动检测数据库类型 |

### 2. ingjoo-security — 三层权限引擎

| 层级 | 机制 | 类比 |
|------|------|------|
| **Layer 1** — 模型级 | `ModelAccess` — 角色 × 实体 × CRUD 操作矩阵 | Odoo `ir.model.access` |
| **Layer 2** — 记录级 | `RecordRule` — Domain 过滤条件自动注入 WHERE | Odoo `ir.rule` |
| **Layer 3** — 集合级 | `collection_isolation` — 自动注入 `collection_id IN (...)` | 多租户隔离 |

策略通过 `SecurityBuilder` 从 JSON 构建，存储在 `module_settings` 表中。

### 3. ingjoo-infra — 基础设施实现

**Feature 门控**：

| Feature | 默认 | 功能 |
|---------|------|------|
| `db` | ✅ | SQLite/PostgreSQL 数据库 |
| `auth` | ✅ | JWT + Argon2 密码认证 |
| `email` | ❌ | SMTP 邮件 (lettre) |
| `sms` | ❌ | 短信发送 (reqwest → 腾讯云) |
| `captcha` | ❌ | 图片验证码 (image) |
| `s3` | ❌ | S3 对象存储 (aws-sdk-s3) |
| `mock` | ❌ | Mock 实现 (测试用) |
| `full` | ❌ | 启用所有功能 |

### 4. ingjoo-cache — 独立缓存封装

基于 Moka 的泛型同步缓存，不依赖任何其他 ingjoo crate。

### 5. ingjoo-bin — 可执行入口

启动流程：`读取 DATABASE_URL` → `安装驱动` → `连接池` → `运行迁移` → `创建 ScaffStore` → `监听 :3000`

## 数据流

```
HTTP 请求
    │
    ▼
axum Router (CORS + Trace 中间件)
    │
    ▼
Handler 函数
    │
    ├── 认证: AuthProvider (JWT 验证)
    ├── 权限: SecurityPolicy.check_access() (模型级)
    │         SecurityPolicy.record_filter() (记录级 Domain → SQL)
    │         SecurityPolicy.collection_isolation() (集合级)
    │
    ▼
Arc<dyn IngjooStore> (trait object)
    │
    ▼
Db (sqlx 实现)
    │  ┌─ SQL 通过 Dialect::prepare() 跨库适配
    │  └─ 手写 SQL + sqlx::query_as().bind().fetch_xxx()
    ▼
SQLite / PostgreSQL
```

## Store Trait 层次结构

```
UserStore ───────┐
TokenStore ──────┤
CaptchaStore ────┤
SmsCodeStore ────┼──→ IngjooStore (聚合 trait, blanket impl)
SettingsStore ───┤        │
PreferenceStore ─┤        └──→ IngjooTransaction (+ 事务方法)
AttachmentStore ─┤
ModuleSettingStore┘
```

- 每个存储能力一个独立 trait
- `IngjooStore` 聚合所有 trait，作为 `Arc<dyn IngjooStore>` 使用
- 添加新能力：定义新 trait → 加入聚合约束列表 → 在 `Db` 上实现

## 数据库模式

**10 张核心表**（DDL 在 `ingjoo-infra/src/db/mod.rs::run_migrations`）：

| 表 | 职责 |
|----|------|
| `users` | 用户账户 |
| `refresh_tokens` | JWT 刷新令牌 |
| `password_reset_tokens` | 密码重置令牌 |
| `captcha_codes` | 验证码 |
| `sms_codes` | 短信验证码 |
| `settings` | 系统键值设置 |
| `user_preferences` | 用户偏好 |
| `attachments` | 文件附件 |
| `module_settings` | 级联配置（scope × module × key） |

**迁移策略**：DDL 用 `CREATE TABLE IF NOT EXISTS` + `ALTER TABLE ADD COLUMN`（忽略错误）实现渐进式迁移。

## Domain DSL

Odoo 风格声明式过滤表达式，JSON 格式输入：

```json
["&", ["status", "=", "published"], ["collection_id", "in", ["id1", "id2"]]]
```

编译为 SQL：

```sql
(status = ? AND collection_id IN (?, ?))
```

支持 13 种操作符：`=`, `!=`, `<`, `>`, `<=`, `>=`, `like`, `ilike`, `not like`, `not ilike`, `in`, `not in`, `is null`, `between`

## 配置管理

| 来源 | 方式 |
|------|------|
| 数据库 URL | 环境变量 `DATABASE_URL`（默认 `sqlite:./data/ingjoo.db?mode=rwc`） |
| CORS | 环境变量 `CORS_ORIGIN` |
| JWT 密钥 | `ScaffConfig.jwt_secret` |
| 存储路径 | `ScaffConfig.storage_path` |
| 业务配置 | `module_settings` 表（级联：`system` → `collection`） |
| SMS/Email | `ScaffConfig.sms` / `ScaffConfig.email`（从 settings 表构建） |

## 构建 & 运行

```bash
# 构建
cargo build

# 运行（默认 SQLite）
DATABASE_URL="sqlite:./data/ingjoo.db?mode=rwc" cargo run

# 运行（PostgreSQL）
DATABASE_URL="postgres://user:pass@localhost/ingjoo" cargo run

# 运行测试
cargo test

# 启用全部功能
cargo build --features ingjoo-infra/full
```

## 测试

- **345 个单元/集成测试**（`#[cfg(test)] mod tests` + 集成测试文件）
- 同步测试用 `#[test]`，异步用 `#[tokio::test]`
- 集成测试在 `ingjoo-bin/tests/integration_test.rs`
- Mock 通过 `MockIngjooDb` (Default impl) 实现
