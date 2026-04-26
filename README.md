# ingjoo — 莺竹框架

<p align="center">
  <strong>莺啼晓竹，万物新生</strong>
</p>

**ingjoo** 是一个用 Rust 编写的**开源后端框架**，为构建可动态扩展、多租户、高安全性的业务系统提供核心基础设施。

框架不预设任何业务模型，只提供最强的抽象：动态模型注册、运行时权限引擎、领域查询语言和开箱即用的基础设施。用 `ingjoo` 构建的业务模块可以像竹子一样快速生长，彼此独立又共享同一根脉。

---

## 主要特性

- **动态模型注册表** — 运行时定义和组织业务模型，无需重新编译框架
- **三层权限引擎** — 模型级 CRUD 控制 + 记录级 Domain 过滤 + 多租户自动隔离
- **Domain DSL** — 安全、可组合的查询表达式，13 种操作符，自动编译为 SQL
- **14 个扩展 Trait** — 搜索引擎、事件总线、状态机、翻译、ID 生成、文档处理等
- **任务队列** — 内存/SQL 双后端，WorkerPool + Cron 调度器 + 重试策略
- **级联配置** — 系统默认 → 集合覆盖，动态生效，无需重启
- **多数据库** — SQLite（零配置开发）+ PostgreSQL（生产），通过 Dialect 抽象透明切换
- **完全异步** — 基于 `axum` 0.8 + `sqlx` 0.8 + `tokio`

---

## Crate 架构

```
ingjoo-macros (独立 proc-macro)
ingjoo-cache  (独立缓存，仅依赖 moka)
ingjoo-queue  (独立队列，内存/SQL 双后端)

ingjoo-core ──→ ingjoo-security (三层权限引擎)
            └──→ ingjoo-infra (基础设施实现) ──→ ingjoo-bin (可执行入口)
```

| Crate | 职责 |
|-------|------|
| `ingjoo-core` | 核心抽象：Store trait、Domain DSL、Dialect、动态模型、作用域 |
| `ingjoo-security` | 三层 RBAC 权限引擎 |
| `ingjoo-infra` | 基础设施：DB 实现、JWT 认证、Handler、中间件、14 个扩展实现 |
| `ingjoo-cache` | Moka 缓存封装 |
| `ingjoo-queue` | 任务队列 + Cron 调度器 + WorkerPool |
| `ingjoo-macros` | `#[derive(IngjooModel)]` 过程宏 |
| `ingjoo-bin` | 可执行入口，组装 AppState + 启动 axum 服务 |

---

## 技术栈

| 层面 | 技术 |
|------|------|
| 语言 | Rust Edition 2021 |
| 异步运行时 | tokio 1.x |
| HTTP 框架 | axum 0.8 |
| 数据库 | sqlx 0.8 (SQLite + PostgreSQL) |
| 认证 | jsonwebtoken + argon2 |
| 缓存 | moka 0.12 |
| 序列化 | serde + serde_json |
| 错误处理 | anyhow + thiserror |

---

## 快速开始

### 环境要求

- Rust 1.80+（推荐 1.95+）
- SQLite 3.x（开发默认）
- 可选：PostgreSQL 14+

### 构建与运行

```bash
# 构建
cargo build

# 运行（默认 SQLite，零配置）
DATABASE_URL="sqlite:./data/ingjoo.db?mode=rwc" cargo run

# 运行（PostgreSQL）
DATABASE_URL="postgres://user:pass@localhost/ingjoo" cargo run
```

服务启动后监听 `0.0.0.0:3000`。

### 作为依赖使用

```toml
[dependencies]
ingjoo-core = { git = "https://github.com/ingjoo/ingjoo.git", package = "ingjoo-core" }
ingjoo-security = { git = "https://github.com/ingjoo/ingjoo.git", package = "ingjoo-security" }
ingjoo-infra = { git = "https://github.com/ingjoo/ingjoo.git", package = "ingjoo-infra" }
```

---

## 开发命令

项目使用 [Just](https://github.com/casey/just) 管理开发命令：

```bash
just build          # 构建
just test           # 运行全部测试
just test-crate X   # 运行指定 crate 测试
just lint           # clippy 静态分析
just ci             # 全量检查 (fmt + lint + test)
just dev            # 启动开发服务器
```

根目录也提供跨仓库脚本：

```bash
./build.sh                     # 后端 + 前端构建
./test.sh --backend-only       # 仅后端测试
./test.sh --frontend-only      # 仅前端测试
./dev.sh                       # 仅后端开发服务器
./dev.sh --frontend            # 仅前端开发服务器
./dev.sh --full                # 前后端同时启动
./ci.sh                        # 全量 CI（fmt + clippy + 后端测试 + 前端构建/测试）
```

---

## 测试

约 **415 个测试**，覆盖所有 crate：

```bash
cargo test --workspace              # 全 workspace
cargo test -p ingjoo-core           # 单 crate
cargo test -p ingjoo-bin --test integration_test  # 集成测试
```

---

## 项目结构

```
ingjoo/
├── crates/
│   ├── ingjoo-core/       # 核心 trait + 类型定义
│   ├── ingjoo-security/   # 三层权限引擎
│   ├── ingjoo-infra/      # 基础设施实现
│   ├── ingjoo-cache/      # 缓存封装
│   ├── ingjoo-queue/      # 任务队列
│   ├── ingjoo-macros/     # 过程宏
│   └── ingjoo-bin/        # 可执行入口
├── dev-docs/              # 开发文档 (en/ + zh/)
├── ARCHITECTURE.md        # 架构文档
├── CODE_STYLE.md          # 编码规范
└── Justfile               # 开发任务
```

---

## 相关仓库

| 仓库 | 说明 |
|------|------|
| [ingjoo-js](../ingjoo-js) | JS/TS SDK — `@ingjoo/web` 组件库 |
| [web-base](../web-base) | 管理前端 — Next.js 16 应用 |

---

## 许可证

MIT
