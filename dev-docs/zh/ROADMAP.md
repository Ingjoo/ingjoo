# 改进路线图

> 最后更新：2026-04-24
> 当前版本：v0.1.0 — 阶段 1-3 已完成

## 现状总览

| Crate | 代码行数 | 测试数 | 成熟度 | 状态 |
|-------|---------|--------|--------|------|
| `ingjoo-core` | 2,137 | 68 | **成熟** | Domain DSL、Store trait、Dialect — 可用于生产 |
| `ingjoo-infra` | 5,539 | 60 | **成熟** | DB 实现、认证、存储、handler、中间件、路由 |
| `ingjoo-security` | 478 | 16 | **成熟** | 三层 RBAC 引擎，已接线到 CRUD handler |
| `ingjoo-cache` | 216 | 5 | **完整** | Moka 缓存可用 |
| `ingjoo-queue` | 1,168 | 25 | **已完成** | 内存/SQL 双后端队列、WorkerPool、重试策略、Cron 调度器 |
| `ingjoo-bin` | 306 | 7 | **可用** | 完整 axum 路由、集成测试通过 |
| `ingjoo-macros` | 180 | 4 | **可用** | `#[derive(IngjooModel)]` 派生宏，自动生成 ModelDescriptor |

**合计**：约 10,024 行代码，185 个测试，64 个源文件。

---

## 阶段 1：让框架能跑 ✅ 已完成

> 目标：实现端到端 HTTP 请求流，串联所有已有组件。

| # | 任务 | 优先级 | 状态 | 交付物 |
|---|------|--------|------|--------|
| T1.1 | 在 `ingjoo-bin` 中搭建 axum 路由骨架 | **P0** | ✅ | `main.rs` 含工作 HTTP 服务、CORS、tracing |
| T1.2 | JWT axum 提取器 + Auth 中间件 | **P0** | ✅ | `middleware/auth.rs` — Bearer token 提取、验证、注入 `CurrentUser` |
| T1.3 | `TokenClaims` 增加 `role` + `groups` 字段 | **P0** | ✅ | JWT token 含 `sub` + `role` + `groups` + `exp` + `iat` |
| T1.4 | 将 `SecurityPolicy` 接线到 CRUD handler | **P0** | ✅ | 从 DB 实时加载权限策略，5 个 CRUD handler 全部接入 |
| T1.5 | User CRUD handler（注册/登录/个人资料） | **P1** | ✅ | `/api/auth/register`、`/api/auth/login`、`/api/auth/profile` |
| T1.6 | Settings CRUD handler | **P1** | ✅ | `/api/settings/*` 端点 |
| T1.7 | 集成测试：HTTP → Auth → Security → DB → Response | **P1** | ✅ | 7 个集成测试（注册/登录/刷新/设置等） |

**退出标准**：✅ `cargo run` 启动服务器，`curl` 可完成注册/登录/获取个人资料，JWT 认证和权限检查正常工作。

---

## 阶段 2：让框架可靠 ✅ 已完成

> 目标：防止回归、修复错误处理、规范化迁移。

| # | 任务 | 优先级 | 状态 | 交付物 |
|---|------|--------|------|--------|
| T2.1 | GitHub Actions CI：`test` + `clippy` + `fmt --check` | **P0** | ✅ | `.github/workflows/ci.yml` |
| T2.2 | `ingjoo-infra` 测试覆盖 | **P0** | ✅ | 17 单元测试 + 23 DB 测试 + 7 路由测试 + 13 GenericDB 测试 |
| T2.3 | 统一 Store trait 边界的类型化错误体系 | **P1** | ✅ | `StoreError` 枚举（Database / Config / Io / NotFound / Conflict） |
| T2.4 | 版本化迁移系统 | **P1** | ✅ | v1-v4 迁移，支持多语句 DDL，自动跳过已存在表 |
| T2.5 | Justfile 开发命令 | **P1** | ✅ | `just test`、`just dev`、`just migrate`、`just lint` |
| T2.6 | `.env.example` 含所有可配置变量 | **P2** | ✅ | `DATABASE_URL`、`CORS_ORIGIN`、JWT 密钥等 |
| T2.7 | `rustfmt.toml` 配置 | **P2** | ✅ | 统一的格式化规则 |

**退出标准**：✅ CI 绿色，`ingjoo-infra` 测试 60 个，错误可模式匹配。

---

## 阶段 3：让框架名副其实 ✅ 已完成

> 目标：实现框架的核心卖点——动态模型注册、队列、定时任务。

| # | 任务 | 优先级 | 状态 | 交付物 |
|---|------|--------|------|--------|
| T3.1 | **动态模型注册系统** | **P0** | ✅ | `ModelRegistry` + `GenericDb` — 运行时 schema 定义、自动 DDL、CRUD |
| T3.2 | **`ingjoo-macros` 派生宏** | **P1** | ✅ | `#[derive(IngjooModel)]` — 从结构体 + `#[ingjoo]` 属性自动生成 `fn descriptor()` |
| T3.3 | **权限持久化** | **P1** | ✅ | `model_accesses` + `record_rules` DB 表 + CRUD API + DB 接线 |
| T3.4 | **队列管理系统** | **P1** | ✅ | `ingjoo-queue` crate — 内存/SQL 双后端、WorkerPool、重试策略 |
| T3.5 | **定时任务（Cron）** | **P1** | ✅ | `Scheduler` + `ScheduleStore` + `scheduled_jobs` 表 + 管理 CRUD API |
| T3.6 | **关系字段抽象** | **P2** | ✅ | `Many2one` 字段类型、`RelationConfig`、DDL FK 约束生成 |
| T3.7 | **审计日志字段** | **P2** | ✅ | `create_uid`/`write_uid`/`create_date`/`write_date` 自动填充 |
| T3.8 | **WebSocket 支持** | **P2** | ✅ | 广播通道 + WS handler，`/ws` 公共端点 |

**退出标准**：✅ 模型可在运行时定义，后台任务入队执行，权限通过 API 管理，定时任务，关系字段，WebSocket 通知，派生宏。

### T3.4 队列管理 — 已完成

```
ingjoo-queue
├── src/
│   ├── lib.rs           # Queue trait、JobStatus、QueuedJob
│   ├── memory.rs        # InMemoryQueue — VecDeque 后端（测试用）
│   ├── sql.rs           # SqlQueue — 原子 dequeue（UPDATE WHERE status=pending）
│   ├── worker.rs        # WorkerPool + JobHandler trait，Semaphore 并发控制
│   └── retry.rs         # RetryPolicy — 指数退避 + ±20% 抖动
├── tests/
│   ├── memory_test.rs   # 14 个测试
│   └── sql_test.rs      # 9 个测试
└── migration v4         # queue_jobs 表 + 2 个索引
```

设计决策（实际实现）：
- **双后端**：`InMemoryQueue`（测试）+ `SqlQueue`（生产），共享 `Queue` trait
- **持久化**：`queue_jobs` 表，重启不丢失，原子 dequeue
- **优先级**：数值越小越高，同优先级 FIFO
- **重试**：指数退避 + 最大重试次数 + 死信队列
- **并发**：Tokio Semaphore 工作池，广播式优雅关闭

### T3.5 定时任务 — 已完成

```
ingjoo-queue/src/
├── scheduler.rs         # ScheduledJob、ScheduleStore trait、SqlScheduleStore、Scheduler<Q>
└── (已有队列文件)

ingjoo-infra/src/
├── db/migration.rs      # v5: scheduled_jobs 表 + 索引
└── handlers/schedule.rs # CRUD 端点: /api/schedules、/api/schedules/{id}
```

设计决策：
- **`cron` crate** 解析 cron 表达式（7 字段格式：秒 分 时 日 月 周 年）
- **`ScheduleStore` trait** + `SqlScheduleStore` 实现（与 SqlQueue 相同的 Pool+Dialect 模式）
- **自适应休眠**：调度器根据下次触发时间计算休眠时长
- **队列集成**：`Scheduler<Q: Queue>` 触发任务入队到队列系统
- **管理 CRUD API**：5 个端点，管理员权限

### T3.2 派生宏 — 已完成

```
ingjoo-macros/src/
└── lib.rs              # #[derive(IngjooModel)] 过程宏

ingjoo-infra/tests/
└── derive_test.rs      # 4 个测试：基础字段、audit+many2one、json+timestamp、手动比较
```

属性语法：
- 结构体：`#[ingjoo(table = "name", audit)]`
- 字段：`#[ingjoo(type = "text|integer|float|boolean|timestamp|json|many2one", required, unique, related = "model")]`

### T3.6 关系字段 — 已完成

- `FieldType::Many2one` + `RelationConfig { related_model, foreign_key, through }`
- DDL 生成：通过 `Dialect::reference()` 生成 FK 约束（`REFERENCES table(id) ON DELETE SET NULL`）

### T3.8 WebSocket — 已完成

- `AppState.events: broadcast::Sender<String>` 广播通道（容量 256）
- `handlers/ws.rs`：WS 升级 + split sink/stream + 广播接收
- 路由：`/ws` 公共端点

---

## 阶段 4：让框架可扩展（持续）

> 目标：支撑生态增长——插件、文档、性能。

| # | 任务 | 优先级 | 依赖 | 状态 |
|---|------|--------|------|------|
| T4.0 | **菜单 + 视图 + 动作元数据系统** | **P0** | T3.1 | ✅ |
| T4.1 | 插件/模块热加载系统 | P1 | T3.1 | ✅ |
| T4.2 | 公共 API rustdoc（所有公开项加 `///`） | P1 | 阶段 2 | ✅ |
| T4.3 | 性能基准测试（criterion） | P2 | 阶段 2 | ✅ |
| T4.4 | 完整集成测试套件 | P2 | 阶段 2 | 🔄 |
| T4.5 | `ingjoo-cli` 管理工具 | P2 | 阶段 2 | ✅ |
| T4.6 | 多租户集合隔离测试 | P2 | T3.3 | ✅ |
| T4.7 | 数据库连接池可观测性 | P3 | — | ✅ |
| T4.8 | 限流中间件 | P3 | 阶段 1 | ✅ |

### T4.0 元数据系统 — 已完成

```
ingjoo-core/src/module/
└── metadata.rs          # ViewType、ActionType、MenuDescriptor、ViewDescriptor、ActionDescriptor

ingjoo-infra/src/
├── db/
│   ├── migration.rs     # v6: ir_menu + ir_view + ir_action 三表 + 索引
│   └── seed.rs          # 幂等种子数据（demo action + views + menu）
└── handlers/
    ├── menu.rs          # GET/POST/PUT/DELETE /api/menus — 树构建 + 分组可见性过滤
    ├── view.rs          # GET/POST/PUT/DELETE /api/views — 按 model/type 查询
    └── action.rs        # GET/POST/PUT/DELETE /api/actions — 复合响应（action+views+model）
```

设计决策：
- **JSON arch 格式**（非 Odoo 的 XML）— 前端直接消费 JSON API
- **单表动作**，用 `type` 字段区分（非 PostgreSQL INHERITS 多表继承）
- **命名槽位** 视图继承（v2 预留，v1 不继承）
- **复合 `GET /api/actions/{id}`** — 一次请求返回 action + views + model schema（前端只需 2 次往返）
- **菜单可见性** 通过 JWT `groups` 集合交集过滤（零额外 DB 查询）
- **`page_limit` 列名** 避免 SQLite 保留字 `limit`

---

## 额外已完成项（不在原 ROADMAP 中）

| 任务 | 描述 |
|------|------|
| Postgres 兼容 | `Dialect` 支持 SQLite + Postgres 双后端（placeholder、时间函数、自增主键、DDL 分割） |
| Group 权限系统 | `groups` + `group_implied` + `user_groups` 表，完整的分组管理 CRUD API |
| 记录级权限过滤 | CRUD handler 已接入 `SecurityPolicy`，从 DB 实时加载 model_access + record_rule |

---

## 优先级矩阵（下一步）

```
阶段 4 进行中
├── T4.0 菜单+视图+动作元数据 — ✅ ir_menu/ir_view/ir_action + handler + 种子数据

高影响（阶段 4）
├── T4.1 插件热加载 — ✅ 线程安全 Registry + PluginManifest + PluginManager + 管理 API
├── T4.2 rustdoc — ✅ 全 crate 公共 API 文档注释
├── T4.3 性能基准 — ✅ criterion: Domain DSL / Registry / Cache 三组基准
├── T4.4 完整集成测试 — 🔄 扩展中
├── T4.5 CLI 管理工具 — ✅ clap 5 参数 + env var 支持
├── T4.6 多租户隔离测试 — ✅ 6 单元 + 5 集成测试

已完成（低优先级）
├── T4.7 连接池可观测 — ✅ PoolOptions + PoolStats + health 端点
└── T4.8 限流中间件 — ✅ 可配置分层限流 (public/protected/admin)
```

---

## 架构现状图

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

图例：
- ✅ 完成：代码完成且有测试
- ✅ 未使用：代码完成但未被业务流程调用
