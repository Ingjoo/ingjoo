# 改进路线图

> 最后更新：2026-04-26
> 当前版本：v0.1.0 — 阶段 1-12 已完成

## 现状总览

| Crate | 代码行数 | 测试数 | 成熟度 | 状态 |
|-------|---------|--------|--------|------|
| `ingjoo-core` | 3,920 | 82 | **成熟** | Domain DSL、Store trait、Dialect、NotificationStore |
| `ingjoo-infra` | 15,800 | 197 | **成熟** | DB 实现、认证、存储、handler、中间件、路由、15 扩展实现 |
| `ingjoo-security` | 622 | 22 | **成熟** | 三层 RBAC 引擎，已接线到 CRUD handler |
| `ingjoo-cache` | 342 | 5 | **完整** | Moka 缓存可用 |
| `ingjoo-queue` | 1,585 | 25 | **已完成** | 内存/SQL 双后端队列、WorkerPool、重试策略、Cron 调度器 |
| `ingjoo-bin` | 2,650 | 80 | **可用** | 完整 axum 路由、80 集成测试通过 |
| `ingjoo-macros` | 210 | — | **可用** | `#[derive(IngjooModel)]` 派生宏 |

**合计**：约 24,300 行代码，411 个单元测试 + 集成测试，130+ 个源文件。

### 前端仓库

| 仓库 | 技术 | 页面 | 状态 |
|------|------|------|------|
| `ingjoo-js` (`@ingjoo/web`) | React + TypeScript + Rollup | — | ✅ 共享组件库，auth/i18n/fetch/Domain DSL |
| `web-base` | Next.js 16 + Tailwind CSS 4 | 8 个 | ✅ 完整前后端对接 |

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
| T4.4 | 完整集成测试套件 | P2 | 阶段 2 | ✅ |
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

## 阶段 5：扩展激活 ✅ 已完成

> 目标：激活已有扩展组件、接入安全头、补全 noop 覆盖、集成缓存。

| # | 任务 | 优先级 | 状态 | 交付物 |
|---|------|--------|------|--------|
| T5.1 | 安全响应头中间件激活 | **P0** | ✅ | `security_headers_middleware` 接入路由 — X-Content-Type-Options、X-Frame-Options、CSP 等 |
| T5.2 | 5 个缺失 trait 的 noop 实现 | **P0** | ✅ | NoopStateMachine、NoopSearchEngine、NoopPaymentProvider、NoopRelationLoader、NoopTranslationStore |
| T5.3 | 扩展实现的 feature flag | **P0** | ✅ | Cargo.toml 中 `content-filter`、`data-mask`、`vector-pg`、`signature` feature + aes-gcm 依赖 |
| T5.4 | FrameworkCache 集成到 AppState | **P1** | ✅ | `cache: Arc<FrameworkCache<SecurityPolicy, ()>>` 加入 AppState，load_security_policy() 缓存策略，权限变更时失效缓存 |
| T5.5 | ROADMAP 更新 | **P2** | ✅ | 中英文 ROADMAP 均已更新至阶段 5 |

**退出标准**：✅ 所有扩展 trait 有 noop 降级，安全头在每次响应生效，SecurityPolicy 加载走缓存，feature-gated 实现可编译。

---

## 阶段 6：生产就绪 ✅ 已完成

> 目标：将 14 个扩展 trait 接入 AppState、条件编译 main.rs、审计日志接入 CRUD、缓存测试、全特性编译验证。

| # | 任务 | 优先级 | 状态 | 交付物 |
|---|------|--------|------|--------|
| T6.1 | AppState 接入 14 个扩展 trait 字段 | **P0** | ✅ | `state.rs` 新增 14 个 `Arc<dyn Trait>` 字段 + noop 默认 + 14 个 `with_xxx()` builder 方法 |
| T6.2 | main.rs 条件编译（feature-gated 实现选择） | **P0** | ✅ | `ingjoo-bin/Cargo.toml` feature 转发 + `build_audit()`/`build_content_filter()`/`build_data_mask()`/`build_signature()` 函数 |
| T6.3 | CRUD handler 审计日志 | **P0** | ✅ | create/update/delete 成功后写审计日志，`state.audit.create_audit_log()` fire-and-forget |
| T6.4 | 缓存集成测试 | **P1** | ✅ | 3 个测试：put+get 命中、不同 key 隔离、invalidate 清除 |
| T6.5 | `--all-features` 编译修复 | **P0** | ✅ | 补全 `ids.rs`/`models.rs`/`traits.rs` 的 re-export（GroupId、Group、GroupImplied、ModelAccessRow、RecordRuleRow、GroupStore、AccessStore） |
| T6.6 | ROADMAP + 进度报告更新 | **P2** | ✅ | 中英文 ROADMAP + 进度报告 |

**退出标准**：✅ 14 个扩展 trait 全部接入 AppState，main.rs 按 feature 选择实现，CRUD 审计日志生效，缓存测试通过，`cargo check --workspace --all-features` 零错误。

---

## 阶段 7：扩展实现 ✅ 已完成

> 目标：实现真实的 DB 后端扩展 trait，替换 noop 桩 — EventBus、IdGenerator、Lock、RelationLoader、TranslationStore、StateMachine、CharTextSplitter。接入 state.rs 和 main.rs。

| # | 任务 | 优先级 | 状态 | 交付物 |
|---|------|--------|------|--------|
| T7.1 | EventBus（进程内广播） | **P0** | ✅ | `event_bus_impl.rs` — tokio broadcast channel，异步订阅者分发 |
| T7.2 | IdGenerator（UUID v4） | **P0** | ✅ | `id_generator_impl.rs` — uuid::Uuid::new_v4() |
| T7.3 | Lock（DB 顾问锁） | **P0** | ✅ | `lock_impl.rs` — 基于 SQLite 的互斥锁，带过期时间 |
| T7.4 | DbRelationLoader | **P1** | ✅ | `relation_loader_impl.rs` — 批量加载 Many2one 关系及 display_name |
| T7.5 | DbTranslationStore | **P1** | ✅ | `translation.rs` — ir_translation 表，set/get/batch/remove/list_languages |
| T7.6 | DbStateMachine | **P1** | ✅ | `state_machine_impl.rs` — ir_state_machine/transition/record 表，注册/转换/查询 |
| T7.7 | CharTextSplitter | **P1** | ✅ | `text_splitter_impl.rs` — 字符计数分块，带重叠 |
| T7.8 | 迁移 v8/v9 | **P0** | ✅ | ir_translation、ir_state_machine、ir_state_transition、ir_state_record DDL |
| T7.9 | state.rs 接线 + main.rs | **P0** | ✅ | 所有已实现 trait 使用真实默认值，main.rs 中 feature-gated |

**退出标准**：✅ 6 个扩展 trait 有 DB 后端实现，CharTextSplitter 已实现，370 个测试通过，0 个 clippy 警告。

### 阶段 7.5：额外扩展实现 ✅ 已完成

| # | 任务 | 优先级 | 状态 | 交付物 |
|---|------|--------|------|--------|
| T7.5.1 | HtmlInputSanitizer | **P1** | ✅ | `sanitizer.rs` — 标签白名单、被阻标签内容抑制、属性剥离 |
| T7.5.2 | FsDocumentLoader | **P1** | ✅ | `document_loader.rs` — 递归文件扫描，10MB 限制，元数据提取 |
| T7.5.3 | state.rs 真实默认值 | **P0** | ✅ | sanitizer/document_loader/text_splitter 使用真实实现替代 noop |
| T7.5.4 | 集成测试 | **P0** | ✅ | 7 个 translation + 7 个 state_machine 测试（基于 tempfile SQLite） |

---

## 阶段 8：DbSearchEngine + 集成测试 + QA ✅ 已完成

> 目标：实现 DB 后端全文搜索引擎，接入 HTTP 搜索端点，QA 验证，修复发现的问题。

| # | 任务 | 优先级 | 状态 | 交付物 |
|---|------|--------|------|--------|
| T8.1 | DbSearchEngine 实现 | **P0** | ✅ | `search_engine.rs` — SQL LIKE 全文搜索，ir_search_index 表，v10 迁移 |
| T8.2 | 搜索接线 | **P0** | ✅ | `build_search()` + `with_search()` 在 main.rs 中 feature-gated |
| T8.3 | UTF-8 修复 | **P0** | ✅ | `extract_highlights` char 边界对齐（`floor_char_boundary`/`ceil_char_boundary`） |
| T8.4 | Clippy 清零 | **P0** | ✅ | 0 警告（derive_test dead_code + integration_test is_empty） |
| T8.5 | 搜索集成测试 | **P0** | ✅ | 3 个测试：匹配/无匹配/空查询（380 个测试总计） |
| T8.6 | 文档同步 | **P1** | ✅ | ROADMAP + AGENTS.md + QA 报告 |
| T8.7 | QA 系统化测试 | **P0** | ✅ | 18 个端点/页面测试，健康评分 58/100 |
| T8.8 | QA 修复：logout 路由注册 | **P0** | ✅ | `POST /api/auth/logout` 路由注册 + 失效 token 校验 |
| T8.9 | QA 修复：/api/auth/me 端点 | **P0** | ✅ | 新增 `get_me` handler，返回当前用户 `UserPublic` |
| T8.10 | 种子数据激活 | **P1** | ✅ | `seed_metadata()` 调用接入 `run_migrations()` |

---

## 额外已完成项（不在原 ROADMAP 中）

| 任务 | 描述 |
|------|------|
| Postgres 兼容 | `Dialect` 支持 SQLite + Postgres 双后端（placeholder、时间函数、自增主键、DDL 分割） |
| Group 权限系统 | `groups` + `group_implied` + `user_groups` 表，完整的分组管理 CRUD API |
| 记录级权限过滤 | CRUD handler 已接入 `SecurityPolicy`，从 DB 实时加载 model_access + record_rule |
| PG boolean 适配 | `Dialect::bool_true()`/`bool_false()` 在 menu/view/action handler 中（7 处 SQL） |
| Captcha 验证码 | `handlers/captcha.rs` — 生成/验证 captcha 图片（feature-gated `captcha`） |
| 用户偏好 API | `GET/PUT /api/auth/preferences` — theme/language/notification_channels 持久化 |
| 修改密码 API | `POST /api/auth/change-password` — 验证当前密码 + 更新 |
| 当前用户 API | `GET /api/auth/me` — 返回当前用户 `UserPublic`（前端刷新验证用） |
| 安全 logout | `POST /api/auth/logout` — 删除 refresh token + 失效 token 二次调用校验 |
| 种子数据激活 | `seed_metadata()` 接入 `run_migrations()` — 菜单/视图/动作自动填充 |

---

## 阶段 9：前端对接 ✅ 已完成

> 目标：修复前端与后端 API 的对接问题，实现完整登录→Dashboard 流程。

| # | 任务 | 优先级 | 状态 | 交付物 |
|---|------|--------|------|--------|
| T9.1 | /register 页面修复 | **P0** | ✅ | `login/page.tsx` 读取 `?register=1` query param 切换注册模式 |
| T9.2 | Auth token 持久化 | **P0** | ✅ | `ingjoo-web` auth.tsx/fetch.ts 改为 localStorage + Authorization header |
| T9.3 | /api/auth/me 前端对接 | **P0** | ✅ | 前端 `getMe()` 已对接 `/auth/profile`，页面加载恢复用户状态 |
| T9.4 | 通知 API 前端 stub | **P1** | ✅ | 4 个 Next.js Route Handler stub（防 404 连锁） |
| T9.5 | /admin, /search 骨架页面 | **P2** | ✅ | admin 用户管理+审计日志 tabs，search 全局搜索 |
| T9.6 | 新增后端 API 端点 | **P0** | ✅ | dashboard stats / users search / audit-log 三个端点 |
| T9.7 | Preferences 路径对齐 | **P1** | ✅ | `/users/me/preferences` → `/auth/preferences` |
| T9.8 | 死代码清理 | **P2** | ✅ | 删除 `providers.tsx` |

**退出标准**：✅ 登录→Dashboard 完整流程通过浏览器 QA 验证，页面刷新保持登录状态。

---

## 优先级矩阵（下一步）

```
阶段 7.5 已完成（370 个测试，0 失败，0 个 clippy 警告）
├── T7.5.1 HtmlInputSanitizer — ✅ 标签白名单 + 内容抑制
├── T7.5.2 FsDocumentLoader — ✅ 递归文件扫描，10MB 限制
├── T7.5.3 state.rs 真实默认值 — ✅ sanitizer/document_loader/text_splitter
└── T7.5.4 集成测试 — ✅ 7 个 translation + 7 个 state_machine 测试

阶段 9 已完成（272 个测试，0 clippy 警告，登录→Dashboard QA 通过）
├── T9.1 /register 页面 — ✅ ?register=1 query param
├── T9.2 Auth 持久化 — ✅ localStorage + Authorization header
├── T9.3 /me 前端对接 — ✅ /auth/profile 恢复用户状态
├── T9.4 通知 stub — ✅ 4 个 Next.js Route Handler
├── T9.5 admin/search — ✅ 骨架页面
├── T9.6 新 API 端点 — ✅ dashboard stats / users search / audit-log
├── T9.7 preferences 路径 — ✅ /auth/preferences
└── T9.8 死代码清理 — ✅ 删除 providers.tsx

阶段 12 已完成（411 测试，前端 SSE 认证已修复）
├── T12.1 NotificationBell — ✅ badge + 下拉 + mark-read
├── T12.2 SSE 认证修复 — ✅ fetch-based SSE 携带 Bearer token
├── T12.3 通知中心页面 — ✅ 分页 + 筛选 + SSE 实时刷新
├── T12.4 共享 timeAgo — ✅ 提取到 utils.ts
└── T12.5 Header 接线 — ✅ 通知铃铛已接入 Header

阶段 8 已完成
├── T8.1 DbSearchEngine — ✅ SQL LIKE 全文搜索
├── T8.2 搜索接线 — ✅ build_search() + with_search()
├── T8.3 UTF-8 修复 — ✅ char 边界对齐
├── T8.4 Clippy 清零 — ✅ 0 警告
├── T8.5 搜索集成测试 — ✅ 3 个测试
├── T8.6 文档同步 — ✅ ROADMAP + AGENTS.md
├── T8.7 QA 测试 — ✅ 18 端点/页面，健康评分 58/100
├── T8.8 Logout 路由 — ✅ 注册 + 失效校验
├── T8.9 /api/auth/me — ✅ 当前用户信息
└── T8.10 种子数据 — ✅ seed_metadata() 接入 run_migrations()

阶段 7 已完成
├── T7.1 EventBus — ✅ tokio broadcast
├── T7.2 IdGenerator — ✅ UUID v4
├── T7.3 Lock — ✅ SQLite 顾问锁
├── T7.4 DbRelationLoader — ✅ 批量 Many2one
├── T7.5 DbTranslationStore — ✅ ir_translation CRUD
├── T7.6 DbStateMachine — ✅ 注册/转换/查询
├── T7.7 CharTextSplitter — ✅ 字符计数分块
├── T7.8 迁移 v8/v9 — ✅ DDL
└── T7.9 接线 — ✅ state.rs + main.rs

阶段 6 已完成
├── T6.1 AppState 扩展 trait — ✅ 14 个字段 + noop 默认 + builder 方法
├── T6.2 条件编译 — ✅ feature-gated build_xxx() + ingjoo-bin feature 转发
├── T6.3 审计日志 — ✅ CRUD 成功后 fire-and-forget 审计
├── T6.4 缓存测试 — ✅ 3 个集成测试 (put/get/invalidate)
├── T6.5 --all-features — ✅ 补全 re-export，零错误
└── T6.6 ROADMAP — ✅ 已更新

已完成的阶段
├── 阶段 1：让框架能跑 — ✅
├── 阶段 2：让框架可靠 — ✅
├── 阶段 3：让框架名副其实 — ✅
├── 阶段 4：让框架可扩展 — ✅
├── 阶段 5：扩展激活 — ✅
└── 阶段 6：生产就绪 — ✅
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
│ ✅     │ │ ✅ 已接线 │ │ ✅      │ │ ✅ 完成  │
│已接线  │ │缓存策略   │ │          │ │ (25测试) │
└────┬───┘ └────┬──────┘ └────┬─────┘ └───┬──────┘
     │          │              │           │
     └──────────┴──────┬───────┴───────────┘
                       │
                ┌──────▼──────┐
                 │ ingjoo-infra│  ✅ 完整 (134 测试)
                │(DB/认证/存储)│
                └─────────────┘
```

图例：
- ✅ 完成：代码完成且有测试
- ✅ 未使用：代码完成但未被业务流程调用
