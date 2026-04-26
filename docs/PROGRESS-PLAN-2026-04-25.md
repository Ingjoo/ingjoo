# ingjoo (莺竹框架) — 进度计划报告

> 生成日期: 2026-04-25
> 版本: v0.1.0
> 基线: 411 tests, 0 failures

---

## 一、项目概况

| 指标 | 数值 |
|------|------|
| 代码量 | ~26,000 行 |
| 测试数 | 418 个（全部通过） |
| 源文件 | 140 个 |
| Crate 数 | 7 个 |
| 迁移版本 | v1-v6 |
| 开发周期 | Phase 1 ~ Phase 17 (全部完成) |

---

## 二、Crate 健康度

| Crate | 代码行数 | 测试数 | 成熟度 | 说明 |
|-------|---------|--------|--------|------|
| `ingjoo-core` | 2,137 | 71 | 成熟 | Domain DSL、Store trait、Dialect、8 个扩展 trait |
| `ingjoo-infra` | 5,539 | 60 | 成熟 | DB 实现、认证、存储、handler、中间件、路由 |
| `ingjoo-security` | 478 | 16 | 成熟 | 三层 RBAC 引擎，已接线到 CRUD handler |
| `ingjoo-cache` | 216 | 5 | 完整 | Moka 缓存封装 |
| `ingjoo-queue` | 1,168 | 25 | 已完成 | 内存/SQL 双后端队列、WorkerPool、重试策略、Cron 调度器 |
| `ingjoo-bin` | 306 | 7 | 可用 | 完整 axum 路由、集成测试通过 |
| `ingjoo-macros` | 180 | 4 | 可用 | `#[derive(IngjooModel)]` 派生宏 |

---

## 三、已完成阶段

### Phase 1: 让框架能跑 ✅

> 目标: 实现端到端 HTTP 请求流，串联所有组件

| 任务 | 优先级 | 交付物 |
|------|--------|--------|
| T1.1 axum 路由骨架 | P0 | `main.rs` — CORS、tracing、HTTP 服务 |
| T1.2 JWT 提取器 + Auth 中间件 | P0 | `middleware/auth.rs` — Bearer token 提取验证 |
| T1.3 TokenClaims 增加 role + groups | P0 | JWT 含 sub + role + groups + exp + iat |
| T1.4 SecurityPolicy 接线 CRUD handler | P0 | 从 DB 实时加载权限策略，5 个 CRUD handler 接入 |
| T1.5 User CRUD handler | P1 | `/api/auth/register`、`/login`、`/profile` |
| T1.6 Settings CRUD handler | P1 | `/api/settings/*` 端点 |
| T1.7 集成测试 | P1 | 7 个集成测试（注册/登录/刷新/设置等） |

**退出标准**: ✅ `cargo run` 启动，curl 完成注册/登录/获取资料，JWT 认证和权限正常

### Phase 2: 让框架可靠 ✅

> 目标: 防止回归、规范化迁移、统一错误体系

| 任务 | 优先级 | 交付物 |
|------|--------|--------|
| T2.1 GitHub Actions CI | P0 | `.github/workflows/ci.yml` |
| T2.2 ingjoo-infra 测试覆盖 | P0 | 17 单元 + 23 DB + 7 路由 + 13 GenericDB 测试 |
| T2.3 StoreError 类型化错误 | P1 | `StoreError` 枚举 (Database / Config / Io / NotFound / Conflict) |
| T2.4 版本化迁移系统 | P1 | v1-v4 迁移，多语句 DDL，自动跳过已存在表 |
| T2.5 Justfile 开发命令 | P1 | `just test/dev/migrate/lint` |
| T2.6 .env.example | P2 | DATABASE_URL、CORS_ORIGIN、JWT 密钥等 |
| T2.7 rustfmt.toml | P2 | 统一格式化规则 |
| P2-1 Domain DSL Dialect 集成 | HIGH | SQLite/PostgreSQL 方言适配 (ILIKE、布尔值、日期函数) |
| P2-2 CRUD 接入 Layer 2/3 权限 | HIGH | `generic_list/read_with_filter`、记录级 Domain 过滤 |
| P2-3 动态模型种子数据 | MEDIUM | `seed_model_access()` 从注册表动态播种 |
| P2-4 Scaff → Ingjoo 重命名 | MEDIUM | 13 文件 45 处引用全部替换 |
| P2-5 统一权限检查入口 | MEDIUM | `is_admin()` + `require_admin()`，9 处内联替换 |

**退出标准**: ✅ CI 绿色，测试 334 个通过，错误可模式匹配

### Phase 3: 让框架名副其实 ✅

> 目标: 动态模型注册、队列、定时任务、扩展 trait

| 任务 | 优先级 | 交付物 |
|------|--------|--------|
| T3.1 动态模型注册系统 | P0 | `ModelRegistry` + `GenericDb` — 运行时 schema、自动 DDL、CRUD |
| T3.2 ingjoo-macros 派生宏 | P1 | `#[derive(IngjooModel)]` 自动生成 descriptor |
| T3.3 权限持久化 | P1 | `model_accesses` + `record_rules` 表 + CRUD API |
| T3.4 队列管理系统 | P1 | 内存/SQL 双后端、WorkerPool、重试策略 |
| T3.5 定时任务 (Cron) | P1 | Scheduler + ScheduleStore + 管理 CRUD API |
| T3.6 关系字段抽象 | P2 | Many2one 字段、RelationConfig、DDL FK 约束 |
| T3.7 审计日志字段 | P2 | create_uid/write_uid/create_date/write_date 自动填充 |
| T3.8 WebSocket 支持 | P2 | 广播通道 + WS handler，`/ws` 公共端点 |
| P3-1 ~ P3-8 扩展 Traits | MEDIUM | StateMachine / EventBus / IdGenerator / SearchEngine / PaymentProvider / Lock / RelationLoader / TranslationStore |

**退出标准**: ✅ 运行时模型定义、后台任务入队、权限 API 管理、定时任务、关系字段、WebSocket、派生宏

### Phase 4 (部分): 元数据系统 ✅ T4.0

| 任务 | 优先级 | 交付物 |
|------|--------|--------|
| T4.0 菜单+视图+动作元数据 | P0 | ir_menu/ir_view/ir_action 三表 + handler + 种子数据 |

**设计亮点**:
- JSON arch 格式（非 XML），前端直接消费
- 复合 `GET /api/actions/{id}` — 一次返回 action + views + model schema
- 菜单可见性通过 JWT groups 交集过滤（零额外 DB 查询）

---

## 四、架构现状

```
                          ┌─────────────┐
                          │  ingjoo-bin │  ✅ 完整 HTTP 服务
                          │  (306 行)   │
                          └──────┬──────┘
                                 │
               ┌─────────────────┼─────────────────┐
               │                 │                  │
       ┌───────▼──────┐  ┌──────▼──────┐  ┌───────▼──────┐
       │  middleware   │  │  handlers   │  │   router     │
       │  ✅ 完成     │  │  ✅ 完成    │  │  ✅ 完成     │
       └───────┬──────┘  └──────┬──────┘  └──────────────┘
               │                │
      ┌────────┼────────────────┼────────────┐
      │        │                │            │
┌────▼───┐ ┌──▼────────┐ ┌────▼─────┐ ┌───▼──────┐
│security│ │   cache   │ │   core   │ │  queue   │
│ ✅     │ │ ✅ 未使用 │ │ ✅      │ │ ✅ 完成  │
│已接线  │ │           │ │          │ │ (25测试) │
└────┬───┘ └────┬──────┘ └────┬─────┘ └───┬──────┘
     │          │              │           │
     └──────────┴──────┬───────┴───────────┘
                       │
                ┌──────▼──────┐
                │ ingjoo-infra│  ✅ 完整 (60 测试)
                │(DB/认证/存储)│
                └─────────────┘
```

---

## 五、下一步计划 — Phase 4（进行中）

### 高优先级

| 任务ID | 任务名称 | 优先级 | 依赖 | 预估工时 | 描述 |
|--------|---------|--------|------|---------|------|
| T4.1 | 插件/模块热加载系统 | P1 | T3.1 | 5d | 运行时动态加载/卸载业务模块，支持模块依赖声明和生命周期管理 |
| T4.2 | 公共 API rustdoc | P1 | Phase 2 | 2d | 所有公开项加 `///` 文档注释，生成 HTML 文档 |

### 中优先级

| 任务ID | 任务名称 | 优先级 | 依赖 | 预估工时 | 描述 |
|--------|---------|--------|------|---------|------|
| T4.3 | 性能基准测试 | P2 | Phase 2 | 2d | criterion 基准，覆盖 CRUD、Domain 解析、权限检查热路径 |
| T4.4 | 完整集成测试套件 | P2 | Phase 2 | 3d | 覆盖所有 API 端点的端到端测试，含权限边界场景 |
| T4.5 | ingjoo-cli 管理工具 | P2 | Phase 2 | 3d | CLI 子命令: migrate / seed / serve / model list / user create |
| T4.6 | 多租户集合隔离测试 | P2 | T3.3 | 1d | collection_isolation 端到端验证，确保租户数据完全隔离 |

### 低优先级

| 任务ID | 任务名称 | 优先级 | 依赖 | 预估工时 | 描述 |
|--------|---------|--------|------|---------|------|
| T4.7 | 数据库连接池可观测性 | P3 | — | 1d | 连接池使用率、等待时间、超时监控指标 |
| T4.8 | 限流中间件 | P3 | Phase 1 | 1d | 基于 IP/用户的请求限流，可配置窗口和阈值 |

---

## 六、扩展 Trait 实现计划

> 当前仅定义接口，以下为建议实现顺序和方案

| 顺序 | Trait | 建议实现方案 | 预估工时 | 优先级理由 |
|------|-------|-------------|---------|-----------|
| 1 | IdGenerator | UUID v4 + 雪花 ID（同步实现） | 0.5d | 最简单，无外部依赖，热点路径使用 |
| 2 | EventBus | `tokio::broadcast` 广播实现 | 1d | 已有 broadcast 基础设施（AppState.events） |
| 3 | Lock | SQLite 文件锁 / PostgreSQL advisory locks | 1d | 队列去重和并发控制需要 |
| 4 | TranslationStore | `ir_translation` DB 表 + 缓存 | 1.5d | 多语言支持，国际化基础 |
| 5 | RelationLoader | GenericDb 扩展查询 | 1.5d | 补全 Many2one 的反向关联 |
| 6 | StateMachine | `ir_state_machine` + `ir_state_transition` 表 | 2d | 工作流基础，需要 DB schema 设计 |
| 7 | SearchEngine | SQLite FTS5 (feature-gate) | 2.5d | 需要迁移和索引策略设计 |
| 8 | PaymentProvider | 独立 `ingjoo-payment` crate + Stripe | 3d | 外部依赖最多，应独立 crate |

---

## 七、已完成 API 端点清单

| 端点 | 方法 | 功能 |
|------|------|------|
| `/api/auth/register` | POST | 用户注册 |
| `/api/auth/login` | POST | 用户登录 |
| `/api/auth/profile` | GET | 获取当前用户资料 |
| `/api/auth/refresh` | POST | 刷新 JWT token |
| `/api/settings` | GET/POST/PUT/DELETE | 系统设置 CRUD |
| `/api/models/{model}` | GET/POST/PUT/DELETE | 动态模型 CRUD |
| `/api/models/{model}/schema` | GET | 获取模型 schema |
| `/api/permissions/model-access` | GET/POST/PUT/DELETE | 模型级权限管理 |
| `/api/permissions/record-rules` | GET/POST/PUT/DELETE | 记录级规则管理 |
| `/api/groups` | GET/POST/PUT/DELETE | 分组管理 |
| `/api/menus` | GET/POST/PUT/DELETE | 菜单管理（树结构） |
| `/api/views` | GET/POST/PUT/DELETE | 视图管理 |
| `/api/actions` | GET/POST/PUT/DELETE | 动作管理 |
| `/api/actions/{id}` | GET | 复合响应（action + views + model） |
| `/api/schedules` | GET/POST/PUT/DELETE | 定时任务管理 |
| `/ws` | WebSocket | 实时通知广播 |
| `/api/search` | POST | 全局搜索（跨模型，需 JWT 认证） |
| `/api/admin/search/rebuild/{model}` | POST | 重建指定模型的搜索索引（需 admin） |
| `/api/databases` | GET | 列出所有数据库（JWT admin）— 已有 |
| `/api/databases` | POST | 创建数据库（JWT admin）— 已有 |
| `/api/databases/{name}/status` | GET | 数据库状态查询 — 已有 |
| `/api/databases/{name}` | DELETE | 删除数据库连接池 — 已有 |
| `/api/database/list` | GET | 公共列出数据库（Basic Auth / list_db 控制）— Phase 18 |
| `/api/database/create` | POST | 创建+迁移+播种数据库（Basic Auth）— Phase 18 |
| `/api/database/{name}` | DELETE | 删除数据库含物理文件（Basic Auth）— Phase 18 |
| `/api/database/{name}/backup` | POST | 备份数据库（Basic Auth）— Phase 18 |
| `/api/database/{name}/restore` | POST | 恢复数据库（Basic Auth）— Phase 18 |
| `/api/database/{name}/info` | GET | 数据库详细信息（Basic Auth）— Phase 18 |
| `/api/database/backup/upload` | POST | 上传备份文件（Basic Auth）— Phase 18 |
| `/api/database/backup/list` | GET | 列出备份文件（Basic Auth）— Phase 18 |

---

## 八、关键设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| SQL 方案 | 手写 SQL（无 ORM） | 灵活控制、Dialect 适配、避免 ORM 抽象泄漏 |
| 权限架构 | 三层 RBAC | 对标 Odoo ir.model.access + ir.rule + multi-company |
| 动态模型 | 运行时注册 + 自动 DDL | 无需重编译，支持热插拔 |
| 队列后端 | 内存 + SQL 双后端 | 开发用内存、生产用 SQL，共享 trait |
| 视图格式 | JSON arch（非 XML） | 前端直接消费，无需 XML 解析 |
| 菜单动作 | 单表 + type 字段 | 比 PostgreSQL INHERITS 多表继承更简单 |
| 缓存 | Moka（未集成） | 已封装但未接入业务流程，待需要时启用 |
| 错误体系 | StoreError 枚举 | 模式匹配友好，区分 Database/Config/Io/NotFound/Conflict |

---

## 九、技术债务

| 项目 | 严重度 | 说明 |
|------|--------|------|
| ingjoo-cache 未集成 | 低 | Moka 缓存已封装但未接入业务流程 |
| SecurityPolicy 缓存 | 低 | 每次请求从 DB 加载，可后续用 moka/ttl-cache |
| GenericDb 仅支持平铺字段 | 中 | 嵌套 JSON 字段需手动序列化，无自动 ORM 映射 |
| 迁移不可回滚 | 低 | 仅支持 `CREATE TABLE IF NOT EXISTS` + `ALTER TABLE ADD COLUMN` |
| 无 OpenAPI/Swagger 文档 | 中 | API 文档需手动维护 |
| 非 git 仓库 | 高 | 无版本控制历史，无 CI/CD 触发 |

---

## 十、里程碑时间线

```
2026-04
├── Phase 1  ✅ ──── HTTP 服务基础
├── Phase 2  ✅ ──── 可靠性加固
├── Phase 3  ✅ ──── 核心特性实现
├── Phase 4  ✅ ──── 元数据系统 + 可扩展性
├── Phase 5  ✅ ──── 扩展激活（noop、安全头、缓存、feature flags）
├── Phase 6  ✅ ──── 生产就绪（trait 接线、条件编译、审计、缓存测试）
├── Phase 7  ✅ ──── 扩展实现（RelationLoader/TranslationStore/StateMachine/TextSplitter）
├── Phase 8  ✅ ──── 搜索集成（DbSearchEngine + CRUD 自动索引 + 全局搜索 API + 索引重建）
├── Phase 9  ✅ ──── 前端集成（登录/注册/仪表盘/通知/QA）
├── Phase 10 ✅ ──── 模块系统（安装/卸载/升级 + 空库体验）
├── Phase 11 ✅ ──── 通知系统 + 前端增强
├── Phase 12 ✅ ──── 通知铃铛（NotificationBell + fetch-based SSE）
├── Phase 13 ✅ ──── 集成测试修复 + Profile
├── Phase 14 ✅ ──── 基础设施接线 + 多租户测试 + 性能基准
├── Phase 15 ✅ ──── QA 修复 + 文档同步（健康评分 49→77.5）
├── Phase 16 ✅ ──── QA 验证 + 控制台修复（0 console errors）
├── Phase 17 ✅ ──── 视图管线增强（ViewArch + 看板 + 搜索收藏）
│
│  ── 以下为计划 ──
│
├── Phase 18 多数据库管理     (预估 8d) ← 原 Phase 12，重编号避免冲突
│   ├── T18-C1~C4 配置层
│   ├── T18-B3   feature gate
│   ├── T18-A1~A8 API 端点（8个）
│   ├── T18-M1~M2 迁移/播种复用
│   ├── T18-L1~L4 认证调整
│   ├── T18-P1~P2 物理管理
│   ├── T18-J1~J3 前端工具库
│   └── T18-F1~F7 Admin 面板
│
├── T4.1 插件热加载        (预估 5d)
├── T4.4 集成测试套件      (预估 3d)
├── T4.5 CLI 管理工具      (预估 3d)
├── T4.2 rustdoc           (预估 2d)
├── T4.3 性能基准          (预估 2d)
├── T4.6 多租户隔离测试    (预估 1d)
├── T4.7 连接池可观测      (预估 1d)
└── T4.8 限流中间件        (预估 1d)

扩展 Trait 实现路线:
├── IdGenerator            (预估 0.5d)
├── EventBus               (预估 1d)
├── Lock                   (预估 1d)
├── TranslationStore       (预估 1.5d)
├── RelationLoader         (预估 1.5d)
├── StateMachine           (预估 2d)
├── SearchEngine           (预估 2.5d)
└── PaymentProvider        (预估 3d)
```

---

## 十一、Phase 18 — 多数据库管理（Odoo 对齐）

> **任务等级**: L（跨三仓库，预估 5-8 天）
> **目标**: 补充完整的多数据库管理功能，行为对齐 Odoo 的数据库管理器机制
> **涉及仓库**: source/ingjoo（后端）、source/ingjoo-js（前端工具库）、source/web-base（Admin 面板）
> **状态**: 📋 待开始

### 核心原则

1. **不会在启动时自动创建数据库**，除非显式配置了 `DEFAULT_DB`。
2. **数据库管理界面无需登录**，仅通过配置中的 `ADMIN_PASSWD` 密码保护（HTTP Basic Auth）。
3. **可通过配置项控制是否允许列出数据库**（`LIST_DB`）。

### 现有实现基础

以下功能已存在，本次计划在此之上扩展：

| 已有功能 | 文件 | 说明 |
|---------|------|------|
| `DatabaseManager` 连接池管理 | `db/database_manager.rs` (263行) | RwLock<HashMap> + 动态创建/移除 |
| `database_selector` 中间件 | `middleware/database_selector.rs` (176行) | X-Ingjoo-Database header + ?db= query |
| `DATABASE_BASE_URL` CLI 参数 | `cli.rs` / `main.rs` | `--database-base-url` |
| `GET/POST /api/databases` | `handlers/database.rs` (258行) | 列出/创建数据库（需 JWT admin） |
| `GET /api/databases/{name}/status` | `handlers/database.rs` | 状态查询 |
| `DELETE /api/databases/{name}` | `handlers/database.rs` | 删除连接池（不删物理文件） |
| `CurrentUser.database` 字段 | `extractors/current_user.rs` | JWT 中可携带 database |

### 缺失功能清单

#### 1. 后端 — 配置层

| 任务ID | 任务 | 说明 |
|--------|------|------|
| T18-C1 | `ADMIN_PASSWD` 配置 | 环境变量 `INGJOO_ADMIN_PASSWD`，用于数据库管理 API 的 Basic Auth。不存入 DB |
| T18-C2 | `LIST_DB` 配置 | 布尔值，默认 `true`。若 `false`，`/api/database/list` 返回 403 |
| T18-C3 | `DEFAULT_DB` 配置 | 可选字符串。若设置，启动时自动创建该数据库（若不存在）并执行迁移+播种 |
| T18-C4 | `DBFILTER` 配置 | 正则表达式，默认空（不过滤）。用于限制 `/api/database/list` 返回的范围 |

#### 2. 后端 — 启动行为改造

| 任务ID | 任务 | 说明 |
|--------|------|------|
| T18-B1 | 无 DEFAULT_DB 启动 | 启动后仅初始化连接池管理器，不连接任何业务数据库，等待管理 API 创建 |
| T18-B2 | 有 DEFAULT_DB 启动 | 检查数据库是否存在 → 不存在则创建 → 执行全部迁移 + 播种核心种子数据 → 连接 |
| T18-B3 | `multi-db` Cargo feature | 整个多数据库功能通过 feature gate 控制，默认不开启，保持向后兼容 |

#### 3. 后端 — 数据库管理 API（公共，HTTP Basic Auth）

> 路径统一为 `/api/database/*`（单数），区别于现有 `/api/databases`（复数，JWT admin）
> 认证方式：HTTP Basic Auth，用户名固定 `"admin"`，密码为 `ADMIN_PASSWD`

| 任务ID | 端点 | 方法 | 说明 |
|--------|------|------|------|
| T18-A1 | `/api/database/list` | GET | 返回数据库名称列表（含 name, created_at, active, size）。受 `LIST_DB` 控制。`list_db=false` 时返回 403。应用 `DBFILTER` 正则过滤。**当 `list_db=true` 时无需认证即可返回** |
| T18-A2 | `/api/database/create` | POST | 请求体 `{ "name": "xxx" }`。创建数据库 → 自动迁移 → 播种核心数据 → 返回成功 |
| T18-A3 | `/api/database/{name}` | DELETE | 删除指定数据库（不能删默认库）。关闭连接池 + 移除物理文件/ DROP DATABASE |
| T18-A4 | `/api/database/{name}/backup` | POST | 执行备份，返回备份文件路径和文件名 |
| T18-A5 | `/api/database/{name}/restore` | POST | 请求体含 `backup_path`，从该文件恢复数据库 |
| T18-A6 | `/api/database/{name}/info` | GET | 返回数据库大小、创建时间、是否为当前默认库等信息 |
| T18-A7 | `/api/database/backup/upload` | POST | 上传备份文件到服务器，返回路径。受 Basic Auth 保护 |
| T18-A8 | `/api/database/backup/list` | GET | 列出所有备份文件及元数据。受 Basic Auth 保护 |

#### 4. 后端 — 创建时迁移+播种

| 任务ID | 任务 | 说明 |
|--------|------|------|
| T18-M1 | 迁移函数复用 | 提取 `run_migrations(pool)` 为独立函数，创建新数据库时调用 |
| T18-M2 | 播种函数复用 | 提取 `run_seed(pool)` 为独立函数，创建新数据库时调用（admin 账户、默认菜单、设置定义等） |

#### 5. 后端 — 认证流程调整

| 任务ID | 任务 | 说明 |
|--------|------|------|
| T18-L1 | 登录接口增加 `database` 字段 | `POST /api/auth/login` 新增可选字段 `"database"`，未提供则使用 `DEFAULT_DB` |
| T18-L2 | JWT 载荷含 `db_name` | TokenClaims 增加 `db_name` 字段，后续请求用此定位连接池 |
| T18-L3 | 中间件提取 `db_name` | 认证中间件从 JWT 提取 `db_name` 并注入请求上下文 |
| T18-L4 | 数据库删除时 JWT 失效 | 数据库被删除后，对应连接池不存在，该库的 JWT 自然失效 |

#### 6. 后端 — 连接池管理增强

| 任务ID | 任务 | 说明 |
|--------|------|------|
| T18-P1 | SQLite 物理文件管理 | 每个数据库对应 `./data/{name}.db`，创建/删除时管理物理文件 |
| T18-P2 | PostgreSQL 动态建库 | 在同一 PostgreSQL 实例上动态 `CREATE DATABASE` / `DROP DATABASE` |

#### 7. 前端工具库（@ingjoo/web）

| 任务ID | 任务 | 说明 |
|--------|------|------|
| T18-J1 | 类型定义 | `DatabaseInfo`、`BackupInfo` 类型导出 |
| T18-J2 | API 函数封装 | `fetchDatabaseList`、`createDatabase`、`deleteDatabase`、`backupDatabase`、`restoreDatabase`、`fetchBackupList`、`uploadBackup` — 自动附加 Basic Auth |
| T18-J3 | index.ts 导出 | 在包入口导出上述类型和函数 |

#### 8. 前端 Admin 面板（web-base）

| 任务ID | 任务 | 说明 |
|--------|------|------|
| T18-F1 | 数据库管理页面 | `app/database-manager/page.tsx` — 无需登录，先输入 Admin Password，后显示管理界面 |
| T18-F2 | 数据库列表区域 | 显示名称/大小/创建时间，操作按钮：Backup、Restore、Delete |
| T18-F3 | 创建新数据库区域 | 输入名称 + Create 按钮 |
| T18-F4 | 备份管理区域 | 列出备份文件 + 上传按钮 + 恢复按钮（选择目标数据库） |
| T18-F5 | 登录页增加数据库选择器 | 登录表单增加 Database 输入框（文本/下拉）。`list_db=true` 时可从 `/api/database/list` 获取列表 |
| T18-F6 | 导航菜单调整 | 数据库管理页面出现在公共区域（无需登录）；登录后侧边栏不含此链接 |
| T18-F7 | 前端不保存 admin 密码 | 每次刷新需重新输入（安全考虑） |

### 任务依赖图

```
T18-C1~C4 (配置层)
    ↓
T18-B3 (feature gate) ──→ T18-A1~A8 (API 端点)
    ↓                        ↓
T18-M1~M2 (迁移/播种)   T18-L1~L4 (认证调整)
    ↓                        ↓
T18-P1~P2 (物理管理)    T18-B1~B2 (启动行为)
                             ↓
                    T18-J1~J3 (前端工具库)
                             ↓
                    T18-F1~F7 (Admin 面板)
```

### 验证标准

| 验证项 | 标准 |
|--------|------|
| 现有测试不受影响 | `multi-db` feature 默认关闭时，387 个测试全部通过 |
| 新增测试 | 覆盖 SQLite 创建/删除/备份/恢复 |
| 前端构建 | `next build` 无错误，新页面可渲染 |
| 端到端流程 | 空启动 → 创建数据库 → 登录 → 管理操作 → 备份/恢复 |

### 配置示例

```bash
# 数据库管理密码（必填，仅环境变量）
INGJOO_ADMIN_PASSWD=your_secure_password

# 是否允许列出数据库（默认 true）
LIST_DB=true

# 默认数据库（可选，启动时自动创建）
DEFAULT_DB=my_company

# 数据库名称过滤正则（可选）
DBFILTER=^company_.*$

# 多数据库基础 URL（启用多数据库模式）
DATABASE_BASE_URL=sqlite:./data/
```

### 后端认证行为说明

| 端点 | 认证方式 | 说明 |
|------|---------|------|
| `GET /api/database/list` | 无认证（`list_db=true`时）/ 403（`list_db=false`时） | 前端登录页需要此端点获取数据库列表 |
| `POST /api/database/create` | HTTP Basic Auth (admin:ADMIN_PASSWD) | 管理操作需密码 |
| `DELETE /api/database/{name}` | HTTP Basic Auth | 管理操作需密码 |
| `POST /api/database/{name}/backup` | HTTP Basic Auth | 管理操作需密码 |
| `POST /api/database/{name}/restore` | HTTP Basic Auth | 管理操作需密码 |
| `GET /api/database/{name}/info` | HTTP Basic Auth | 管理操作需密码 |
| `POST /api/database/backup/upload` | HTTP Basic Auth | 管理操作需密码 |
| `GET /api/database/backup/list` | HTTP Basic Auth | 管理操作需密码 |
| `POST /api/auth/login` | 无认证（增加可选 database 字段） | 登录时指定数据库 |

### 工时估算

| 模块 | 预估工时 |
|------|---------|
| 后端配置层 + feature gate | 1d |
| 后端 API 端点（8个） | 2d |
| 后端启动行为 + 迁移/播种复用 | 1d |
| 后端认证调整 | 0.5d |
| 前端工具库 | 0.5d |
| 前端 Admin 页面 | 2d |
| 测试 + 验证 | 1d |
| **合计** | **~8d** |

---

*报告生成: 2026-04-25 | 数据来源: ROADMAP.md / AUDIT-REPORT-2026-04-25.md / CONTINUITY_ses_2424.md*
*最后更新: 2026-04-27 | 同步 Phase 10-17 完成状态 + 多数据库管理重编号为 Phase 18*
