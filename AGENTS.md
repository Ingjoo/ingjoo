# AGENTS.md — ingjoo_rs 项目

> 本文件为项目级配置，覆盖全局 AGENTS.md 中与本项目不匹配的部分。
> 全局通用原则（第一性原理、Karpathy 准则、语言偏好）仍然生效。

## 项目概述

**ingjoo (莺竹框架)** — Rust 开源后端框架，提供动态模型注册、三层权限引擎、Domain DSL 查询语言和可插拔基础设施，用于构建多租户业务系统。灵感源自 Odoo 的权限与配置体系。

## 目录结构

```
ingjoo/                              # 项目根目录
├── AGENTS.md                        # 本文件 — 项目上下文 + 开发规范
├── opencode.json                    # OpenCode 工具配置
├── build.sh / ci.sh / dev.sh / test.sh  # 开发脚本快捷方式
├── config/                          # 运行时配置文件
├── thoughts/                        # 设计思路记录
├── cankao/                          # 参考资料（PRD、架构、核心函数文档 PDF）
├── docs/
│   ├── ledgers/                     # 会话持续日志（跨会话上下文）
│   ├── AUDIT-REPORT-*.md            # 安全审计报告
│   └── PROGRESS-PLAN-*.md           # 进度计划
└── source/
    ├── ingjoo/                      # Rust 后端框架（独立 git）
    │   ├── Cargo.toml               # Workspace 根配置
    │   ├── Cargo.lock               # 依赖锁定
    │   ├── Justfile                 # 开发任务（build/test/lint/ci）
    │   ├── rustfmt.toml             # 格式化配置
    │   ├── README.md                # 项目说明
    │   ├── ARCHITECTURE.md          # 架构文档
    │   ├── CODE_STYLE.md            # 编码规范
    │   ├── .github/workflows/ci.yml # CI 流水线
    │   ├── crates/
    │   │   ├── ingjoo-core/         # 核心抽象（trait + 类型定义）
    │   │   │   ├── src/
    │   │   │   │   ├── config/      # 模块配置
    │   │   │   │   ├── db/          # DB 抽象（error, pool, dialect）
    │   │   │   │   ├── extension/   # 扩展 trait（SearchEngine, EventBus, StateMachine 等）
    │   │   │   │   ├── module/      # 模块系统
    │   │   │   │   ├── query/       # Domain DSL 查询引擎
    │   │   │   │   └── scope/       # 多租户作用域
    │   │   │   └── benches/         # 基准测试（domain, registry）
    │   │   ├── ingjoo-security/     # 三层权限引擎
    │   │   │   └── src/policy.rs    # ModelAccess + RecordRule + SecurityPolicy
    │   │   ├── ingjoo-infra/        # 基础设施实现
    │   │   │   ├── src/
    │   │   │   │   ├── auth/        # JWT 认证
    │   │   │   │   ├── captcha/     # 验证码
    │   │   │   │   ├── db/          # DB 实现（含 mock.rs 测试辅助）
    │   │   │   │   ├── email/       # 邮件 provider
    │   │   │   │   ├── extractors/  # Axum 提取器
    │   │   │   │   ├── handlers/    # HTTP handler（CRUD、Auth、Settings、Search）
    │   │   │   │   ├── middleware/   # 中间件（认证、安全头、审计日志）
    │   │   │   │   ├── plugin/      # 插件系统
    │   │   │   │   ├── sms/         # 短信 provider
    │   │   │   │   ├── storage/     # 文件存储 provider
    │   │   │   │   ├── extension_impl/  # 扩展 trait DB 实现（14 个）
    │   │   │   │   ├── extension_noop.rs  # Noop 默认实现
    │   │   │   │   ├── state.rs     # AppState（聚合所有依赖）
    │   │   │   │   ├── config.rs    # 基础设施配置
    │   │   │   │   └── router.rs    # 路由注册
    │   │   │   └── tests/           # 集成测试
    │   │   │       ├── db_test.rs / generic_test.rs / derive_test.rs
    │   │   │       ├── handlers_test.rs / router_test.rs
    │   │   │       └── ...
    │   │   ├── ingjoo-cache/        # 独立缓存封装（Moka）
    │   │   │   └── benches/         # 基准测试
    │   │   ├── ingjoo-queue/        # 任务队列（内存 + SQL）
    │   │   │   ├── src/
    │   │   │   │   ├── memory.rs    # InMemoryQueue
    │   │   │   │   ├── sql.rs       # SqlQueue
    │   │   │   │   ├── scheduler.rs # Cron 调度器
    │   │   │   │   ├── worker.rs    # WorkerPool
    │   │   │   │   └── retry.rs     # 重试策略
    │   │   │   └── tests/           # memory_test.rs + sql_test.rs
    │   │   ├── ingjoo-macros/       # 过程宏（#[derive(IngjooModel)]）
    │   │   └── ingjoo-bin/          # 可执行入口
    │   │       ├── src/
    │   │       │   ├── main.rs      # 服务器启动 + AppState 构建
    │   │       │   ├── cli.rs       # CLI 参数解析
    │   │       │   └── lib.rs       # 公共导出
    │   │       └── tests/
    │   │           └── integration_test.rs  # 端到端集成测试
    │   └── dev-docs/                # 开发文档（en/ + zh/）
    │       ├── en/                  # 英文（ROADMAP, DEV-GUIDE, QUICK-START）
    │       └── zh/                  # 中文（同上 + PROGRESS-REPORT）
    ├── ingjoo-js/                   # JS/TS SDK + 前端工具库（独立 git）
    │   └── packages/
    │       └── ingjoo-web/          # Web SDK（API 客户端、类型定义）
    └── web-base/                    # 前端 Web 应用（独立 git，Next.js）
        ├── package.json             # Next.js 16 项目配置
        ├── next.config.ts           # Next.js 配置
        ├── src/
        │   ├── app/                 # App Router 页面
        │   │   ├── login/           # 登录页
        │   │   ├── register/        # 注册页
        │   │   ├── dashboard/       # 仪表盘
        │   │   ├── admin/           # 管理后台
        │   │   ├── profile/         # 用户资料
        │   │   ├── settings/        # 设置
        │   │   ├── search/          # 搜索
        │   │   ├── notifications/   # 通知
        │   │   └── forgot-password/ # 忘记密码
        │   ├── components/          # 共享组件（Header, Footer, DiscussionPanel 等）
        │   ├── lib/                 # 工具库（api.ts, configs.ts, types.ts, hooks）
        │   └── scaff/               # 脚手架模块（组件、国际化、模块模板）
        └── public/                  # 静态资源
```

## 技术栈

| 层面 | 技术 |
|------|------|
| 语言 | Rust Edition 2021 |
| 异步运行时 | tokio 1.x (full features) |
| HTTP 框架 | axum 0.8 |
| 数据库 | sqlx 0.8 (SQLite + PostgreSQL) |
| 认证 | jsonwebtoken + argon2 |
| 缓存 | moka 0.12 |
| 错误处理 | anyhow + thiserror |
| 序列化 | serde + serde_json |

## 常用命令

所有命令在 `source/ingjoo/` 目录下执行。

```bash
# 构建
just build

# 全部测试（当前基线: 411 tests）
just test

# 运行
just dev

# lint
just lint

# 全量检查（CI 本地版: fmt-check + lint + test）
just ci

# 环境变量
DATABASE_URL="sqlite:./data/ingjoo.db?mode=rwc"  # 默认 SQLite
```

根目录脚本（跨仓库）：

```bash
./build.sh                     # 后端 + 前端构建
./build.sh --backend-only      # 仅后端
./build.sh --release           # release 构建

./test.sh                      # 后端 + 前端测试
./test.sh --backend-only       # 仅后端
./test.sh --frontend-only      # 仅前端 (@ingjoo/web)
./test.sh --crate ingjoo-core  # 指定 crate

./dev.sh                       # 仅后端开发服务器
./dev.sh --frontend            # 仅前端 (web-base, port 3001)
./dev.sh --full                # 前后端同时启动

./ci.sh                        # 全量 CI (fmt + clippy + test + 前端)
./ci.sh --skip-frontend        # 跳过前端
./ci.sh --fix                  # 自动修复格式
```

## 核心架构

### Crate 依赖关系

```
ingjoo-macros (独立 proc-macro)
ingjoo-cache  (独立，仅依赖 moka)

ingjoo-core ──→ ingjoo-security (权限引擎)
            └──→ ingjoo-infra (基础设施) ──→ ingjoo-bin (可执行入口)
```

### Store Trait 层次

```
UserStore + TokenStore + SettingsStore + ... → IngjooStore (聚合 trait)
```

- 每个存储能力一个独立 trait
- `IngjooStore` 聚合所有 trait，作为 `Arc<dyn IngjooStore>` 使用
- 返回类型: `StoreResult<T>` (非 `anyhow::Result<T>`)

### 三层权限引擎

| 层级 | 机制 |
|------|------|
| Layer 1 — 模型级 | ModelAccess: 角色 × 实体 × CRUD 矩阵 |
| Layer 2 — 记录级 | RecordRule: Domain 过滤自动注入 WHERE |
| Layer 3 — 集合级 | collection_isolation: 多租户自动隔离 |

### 扩展 Traits (ingjoo-core/src/extension/)

StateMachine, EventBus, IdGenerator, SearchEngine, PaymentProvider, Lock, RelationLoader, TranslationStore

## 编码规范摘要

- **命名**: Struct/Trait/Enum PascalCase, 函数 snake_case, 常量 SCREAMING_SNAKE_CASE
- **Import**: 逐行 use，不聚合
- **SQL**: 手写（无 ORM），通过 `Dialect::prepare()` 跨库适配，占位符统一 `?`
- **错误**: HTTP 层 `AppError`, 领域层 `thiserror`, 通用 `anyhow`
- **错误消息**: 中文
- **测试**: 源文件底部 `#[cfg(test)] mod tests`，异步用 `#[tokio::test]`
- **Feature 门控**: `#[cfg(feature = "...")]` 控制模块导出
- **详见**: `source/ingjoo/CODE_STYLE.md`

## 开发约定

- `ingjoo-core` 只定义 trait 和类型，不含具体实现
- 新存储能力: 定义 trait → 加入 `IngjooStore` 约束 → 在 `Db` 上实现
- 新 ID 类型: `define_id!(EntityId);`
- SQL 必须通过 `self.sql()` 处理（Dialect 适配）
- 迁移用 `CREATE TABLE IF NOT EXISTS` + `ALTER TABLE ADD COLUMN` 渐进式

## 参考文档

- `cankao/产品需求文档 (PRD) - Rustdoo.pdf` — 产品需求
- `cankao/架构设计文档 - Rustdoo.pdf` — 架构设计
- `cankao/核心功能函数文档 - Rustdoo.pdf` — 核心函数
- `source/ingjoo/ARCHITECTURE.md` — 当前架构文档
- `source/ingjoo/CODE_STYLE.md` — 编码规范
- `source/ingjoo/dev-docs/` — 开发文档（ROADMAP、DEV-GUIDE、QUICK-START）

## 当前状态

- **测试基线**: 411 passed (workspace), 0 failures
- **代码规模**: ~25,000 行 Rust，130+ 源文件
- **Phase 1 (HTTP 服务)**: ✅ 完成
- **Phase 2 (修复/加固)**: ✅ 完成
- **Phase 3 (扩展特性)**: ✅ 完成 — 8 个扩展 trait
- **Phase 4 (元数据+可扩展)**: ✅ 完成 — 菜单/视图/动作、插件系统、CLI
- **Phase 5 (扩展激活)**: ✅ 完成 — noop 补全、安全头、缓存集成
- **Phase 6 (生产就绪)**: ✅ 完成 — 14 trait 接线、条件编译、审计日志
- **Phase 7 (扩展实现)**: ✅ 完成 — RelationLoader/TranslationStore/StateMachine/TextSplitter 真实实现
- **Phase 7.5 (输入安全)**: ✅ 完成 — InputSanitizer + DocumentLoader
- **Phase 8 (搜索集成)**: ✅ 完成 — DbSearchEngine + CRUD 自动索引 + 全局搜索 API + 索引重建管理端点
- **Phase 9 (前端集成)**: ✅ 完成 — 登录/注册/仪表盘/通知/QA 验证
- **Phase 10 (模块系统)**: ✅ 完成 — 模块安装/卸载/升级 + 空库体验
- **Phase 11 (通知系统)**: ✅ 完成 — NotificationStore + SSE + 管理后台增强
- **Phase 12 (通知铃铛)**: ✅ 完成 — NotificationBell + fetch-based SSE 认证 + 共享 timeAgo
- **Phase 13 (集成测试修复+Profile)**: ✅ 完成 — 头像上传 + 偏好持久化 + 前端构建修复
- **StoreError 类型化**: ✅ 完成

---

# Rust 开发工作流（项目级覆盖）

> 本节覆盖全局 AGENTS.md 中的 Odoo 开发工作流，适配 Rust 工具链。
> 全局通用原则（第一性原理、Karpathy 准则、铁律精神）保持不变，具体实现适配本项目。

## 铁律（Rust 适配版）

```
⚖️ 铁律 1: 没有失败测试，就不写生产代码
    先写测试 → cargo test 确认失败 → 再写实现。

⚖️ 铁律 2: 没有根因调查，就不修 bug
    先复现 → 追踪数据流 → 确认根因 → 再修。猜测修复 = 失败。

⚖️ 铁律 3: 没有验证证据，就不声称完成
    必须在本次对话中运行验证命令（cargo test / just ci）并确认输出。

⚖️ 铁律 4: 没有规格合规确认，就不做代码质量审查
    先确认"做了对的事"（规格合规），再确认"把事做对了"（代码质量）。
```

---

## 任务评级

| 等级 | 规模 | 典型场景 | 适用流程 |
|------|------|---------|---------|
| **S** | < 30min | 修 bug、加字段、改错误消息、调路由 | 编码 → 测试 → 审查 |
| **M** | 0.5-2 天 | 新功能、新 trait、小模块 | 精简PRD → 技术设计 → TDD → 审查 |
| **L** | 2-5 天 | 新 crate、跨 crate 改动、复杂工作流 | 完整 8 阶段 |
| **XL** | 5+ 天 | 架构重构、多 crate 集成 | 完整 8 阶段 + 额外架构评审 |

### 评级决策树

```
用户提出需求
    ↓
改动范围 ≤ 3 个文件 且 无新数据模型？──是──→ S 级
    ↓ 否
涉及新 crate 或 新数据模型？──否──→ M 级
    ↓ 是
涉及多 crate 集成 或 架构变更？──否──→ L 级
    ↓ 是
    XL 级
```

### 各等级适用阶段

| 阶段 | S | M | L | XL |
|------|---|---|---|----|
| 0. 任务评级 | ✅ | ✅ | ✅ | ✅ |
| 1. 需求定义 | - | ✅ | ✅ | ✅ |
| 2. PRD 文档 | - | ✅(精简) | ✅(完整) | ✅(完整) |
| 3. 流程设计 | - | - | ✅ | ✅ |
| 4. 技术设计 | - | ✅ | ✅ | ✅ |
| 5. 任务分解 | - | - | ✅ | ✅ |
| 6. 编码实现 | ✅ | ✅ | ✅ | ✅ |
| 7. 代码审查 | ✅(简化) | ✅ | ✅ | ✅ |
| 8. 文档归档 | - | ✅(简化) | ✅ | ✅ |

**S 级特殊规则**：
- 无需创建文档目录结构
- 编码后只需 `cargo test` + `just lint`
- 完成后在对话中简要说明改动即可

---

## 编码实现（Rust TDD）

### TDD 三色循环

1. **🔴 红色阶段**：先写测试
   - 在源文件底部 `#[cfg(test)] mod tests` 中添加测试
   - `cargo test -p {crate}` 确认测试失败（编译通过但断言失败）

2. **🟢 绿色阶段**：实现功能
   - 写最少代码使测试通过
   - `cargo test -p {crate}` 确认通过

3. **🔵 蓝色重构阶段**：优化
   - 在测试通过的前提下重构
   - 重构后再跑 `cargo test`

### 测试命令速查

```bash
# 单 crate 测试
just test-crate ingjoo-core
just test-crate ingjoo-infra

# 全 workspace 测试
just test

# 集成测试
just test-integration

# lint（clippy 严格模式）
just lint

# 全量检查（CI 本地版）
just ci
```

### 外科手术式修改检查

每次改动后确认：
- diff 只含必要改动
- 无顺手重构、无格式化漂移
- 新增的 `use` 语句都是必要的
- 无遗留的 `todo!()` 或 `unimplemented!()`

---

## 代码审查（Rust 版）

### 审查工具决策树

```
阶段 7A: 规格合规审查（先做）
  ├─ 对照 PRD/任务清单逐项检查
  ├─ 确认所有 FR/AC 有对应实现
  └─ 确认无多余功能（YAGNI）
       ↓ 通过后
阶段 7B: 代码质量审查（后做）
  ├─ just lint (clippy) → 警告扫描
  ├─ lsp_diagnostics → 类型错误
  ├─ cargo test → 全部通过
  └─ 代码清理
```

### 验证命令映射

| 全局配置 (Odoo) | 本项目 (Rust) | 用途 |
|-----------------|---------------|------|
| `odoo-bin -d test_db --test-enable` | `just test` | 运行全部测试 |
| 特定模块测试 | `just test-crate {crate}` | 单 crate 测试 |
| 集成测试 | `just test-integration` | 集成测试 |
| PEP8 检查 | `cargo fmt --check` | 格式检查 |
| 手动代码审查 | `just lint` (clippy) | 静态分析 |
| 全量验证 | `just ci` | fmt + lint + test |

### 代码审查清单

- [ ] `just test` 通过（0 failures）
- [ ] `just lint` 通过（0 warnings）
- [ ] `cargo fmt --check` 通过
- [ ] 无 `unwrap()` 在非测试代码中（用 `?` 或显式错误处理）
- [ ] 无 `as any`、`#[allow(...)]` 抑制警告
- [ ] 无未使用的 `use` 导入
- [ ] 错误消息为中文
- [ ] SQL 通过 `self.sql()` 处理（Dialect 适配）
- [ ] 新公共 API 有对应测试
- [ ] **Karpathy 反模式检查**:
  - [ ] 无隐含假设
  - [ ] 无投机代码/过度抽象
  - [ ] diff 无非必要改动
  - [ ] 每个修复有对应测试

---

## 验证完成守门

每个阶段完成前：

```
BEFORE 声称完成：

1. IDENTIFY: 什么命令/证据能证明完成？
2. RUN: 执行验证（just test / just lint / just ci）
3. READ: 读完整输出，确认结果
4. VERIFY: 输出是否确认声明？
   - NO → 说明实际状态和差距
   - YES → 附证据声明完成
5. ONLY THEN: 才能标记完成
```

### 各阶段验证方式

| 阶段 | 验证方式 |
|------|---------|
| 阶段 1 需求定义 | vision.md 存在 + 用户已确认范围 |
| 阶段 2 PRD | prd.md 存在 + 每个 FR 有 US 和 AC |
| 阶段 3 流程设计 | user-story-flow.md 存在 + 流程图完整 |
| 阶段 4 技术设计 | tech-design.md 存在 + 字段/trait 设计完整 |
| 阶段 5 任务分解 | task-list.md 存在 + 无占位符 |
| 阶段 6 编码实现 | `just test` 通过 + diff 只含必要改动 |
| 阶段 7 代码审查 | 7A 规格合规 + 7B `just lint` 通过 |
| 阶段 8 文档归档 | 文档已同步 + 测试仍通过 |

---

## 调试规则

### 3 次失败质疑架构

连续 3 次修复尝试失败：

1. **立即停止** — 不再尝试第 4 次修复
2. **评估信号** — 是否架构问题：
   - 每次修复都在不同位置暴露新问题
   - 修复需要大规模重构才能实现
   - 每次修复在新的 trait 依赖/crate 边界处出问题
3. **质疑架构本身**：当前 trait 层次/crate 拆分是否合理？
4. **与用户讨论后再决定**

---

## 变更管理

| 变更等级 | 典型场景 | 处理方式 |
|---------|---------|---------|
| **S 级** | 加字段、改错误消息、调路由 | 直接修改，记录变更 |
| **M 级** | 改 trait 签名、加新 Store 能力 | 更新 PRD → 调整技术设计 |
| **L 级** | 改 crate 依赖关系、新 crate | 回退到需求定义 |
| **取消** | 功能不要了 | 记录原因，标记取消 |

---

## 文档目录约定（L/XL 级）

```
docs/
├── specify/           # 规格文档
│   ├── vision.md      # 愿景
│   ├── prd.md         # 产品需求
│   ├── user-story-flow.md  # 流程设计（L/XL）
│   └── tech-design.md # 技术设计
├── progress/          # 进度文档
│   ├── development-log.md  # 开发日志
│   ├── task-list.md   # 任务清单
│   └── code-review.md # 代码审查报告
```

---

*最后更新: 2026-04-25 (Rust 适配版 v1.0 — 覆盖全局 Odoo 工作流)*
