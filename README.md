# ingjoo

# ingjoo - 莺竹框架

<p align="center">
  <strong>莺啼晓竹，万物新生</strong>
</p>

**ingjoo** 是一个用 Rust 编写的**开源后端框架**，为构建可动态扩展、多租户、高安全性的业务系统提供核心基础设施。它是业务平台的骨架，轻量、灵活，并且对开发者友好。

---

## ✨ 核心理念

> 框架不预设任何业务模型，只提供最强的抽象：动态模型注册、运行时权限引擎、领域查询语言和开箱即用的基础设施。
> 用 `ingjoo` 构建的业务模块可以像竹子一样快速生长，彼此独立又共享同一根脉。

---

## 🧩 主要特性

- **动态模型注册表**：运行时定义和组织业务模型，无需重新编译框架。
- **三层权限引擎** (`ingjoo-security`)：
  - 模型级 CRUD + 导入导出控制
  - 记录级过滤规则（类似 Odoo 的 `ir.rule`）
  - 多租户/集合自动隔离
- **Domain DSL**：一种安全、可组合的查询表达式，自动转换为 SQL 条件，支持 13 种操作符。
- **级联配置** (`module_settings`)：系统默认 → 集合覆盖，动态生效，无需重启。
- **内置基础设施** (`ingjoo-infra`)：SQLite 默认存储、JWT 认证、SMS/邮件/存储等 provider 接口。
- **完全异步**：基于 `axum` 和 `sqlx`，享受 Rust 的性能与安全。

---

## 🚀 快速开始

```toml
[dependencies]
ingjoo-core = { git = "https://github.com/ingjoo/ingjoo.git", package = "ingjoo-core" }
ingjoo-security = { git = "https://github.com/ingjoo/ingjoo.git", package = "ingjoo-security" }
ingjoo-infra = { git = "https://github.com/ingjoo/ingjoo.git", package = "ingjoo-infra" }
```
