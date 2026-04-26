---
session: ses_2424
updated: 2026-04-24T20:23:20.494Z
---

# 会话总结

## 目标
实现Ingjoo (莺竹) Rust 框架第二阶段（修复/加固）和第三阶段（扩展特性）——一个具有三层权限引擎、领域 DSL 和可扩展特性架构的动态业务平台。当所有 P2/P3 项目完成且 `cargo test --workspace` 通过（217 个测试，0 个失败）时，即标志着成功。

## 限制与偏好
- Rust 工作区包含 6 个 crate：`ingjoo-core`、`ingjoo-security`、`ingjoo-infra`、`ingjoo-queue`、`ingjoo-macros`、`ingjoo-bin`
- 使用 `thiserror` 处理错误类型，`async_trait` 用于异步特性，`serde`/`serde_json` 用于序列化
- 与方言无关的 SQL（主要支持 SQLite，通过 `Dialect` 枚举支持 PostgreSQL）
- 面向用户的中文错误消息（例如，`"需要管理员权限"`，`"记录不存在或无权删除"`）
- 幂等的初始数据（`INSERT OR IGNORE` / `ON CONFLICT DO NOTHING`）
- 所有公共 API 从 `Scaff*` 重命名为 `Ingjoo*`（本次会话已完成）
- `SecurityPolicy` 字段是私有的——必须使用 `add_model_access()` / `add_record_rule()` 构建器方法
- `GroupId` 是一个需要 `.to_string()` 以适应 `String` 上下文的新类型包装器

## 进展

### 已完成
- [x] **P2-1**：领域 DSL 方言集成 — `Domain::to_sql()` 接受 `&Dialect` 参数，生成与方言匹配的 SQL（布尔值 `1/0` 与 `TRUE/FALSE`，`LIKE` 与 `ILIKE`，日期时间函数）
- [x] **P2-2**：带有第2层（模型访问）+ 第3层（记录规则）权限过滤的 CRUD 处理器 — `crud.rs` 在所有 CRUD 接口上使用 `load_security_policy()`、`check_access()`、`get_record_filter()`；通过 `add_model_access()`/`add_record_rule()` 替代直接推送，修复了私有字段访问问题；修复了 `GroupId→String` 转换；移除了旧的重复处理器中的函数定义（L210-340）
- [x] **P2-3**：动态模型初始数据 — 将 `seed::seed_model_access(pool, dialect, &models)` 函数提取到 `seed.rs` 中，添加了 `AppState::seed_model_access_from_registry()` 方法，该方法读取注册表并在运行时播种 `model_access`
- [x] **P2-4**：完整的 `Scaff*` → `Ingjoo*` 重命名，涉及13个文件，45处引用 — `IngjooStore`、`IngjooTransaction`、`IngjooDb`、`MockIngjooDb`、`IngjooConfig`；没有 `Scaff` 引用残留
- [x] **P2-5**：统一的权限检查 — `CurrentUser::is_admin()` + `require_admin()` 在 `extractors/auth.rs` 中，替换了9处分散的 `groups.iter().any(|g| g == "admin")` 模式
- [x] **P3-1**：`StateMachine` 特性 — `extension/state_machine.rs`，包含 `StateTransition`、`TransitionError`、异步方法：`get_current_state`、`get_available_transitions`、`transition`、`register_machine`
- [x] **P3-2**：`EventBus` 特性 — `extension/event_bus.rs`，包含 `Event` 结构体、`EventHandler` 类型别名、异步方法：`publish`、`subscribe`、`unsubscribe`
- [x] **P3-3**：`IdGenerator` 特性 — `extension/id_generator.rs`，包含同步方法：`generate(prefix)`、`generate_uuid()`、`generate_short_id(length)`
- [x] **P3-4**：`SearchEngine` 特性 — `extension/search.rs`，包含 `SearchQuery`、`SearchResult`、`SearchHighlight`、异步方法：`index_record`、`remove_record`、`search`、`rebuild_index`
- [x] **P3-5**：`PaymentProvider` 特性 — `extension/payment.rs`，包含 `PaymentStatus` 枚举、`PaymentIntent`、`PaymentResult`、`RefundResult`、异步方法：`create_intent`、`confirm`、`cancel`、`refund`、`get_status`
- [x] **P3-6**：`Lock` 特性 — `extension/lock.rs`，包含 `LockGuard`（键/令牌/过期时间 + `is_expired()`）、异步方法：`acquire`、`try_acquire`、`extend`
- [x] **P3-7**：`RelationLoader` 特性 — `extension/relations.rs`，包含 `Many2Many` 辅助结构体、异步方法：`load_one2many`、`load_many2one`
- [x] **P3-8**：`TranslationStore` 特性 — `extension/translation.rs`，包含 `Translation` 结构体、异步方法：`get`、`set`、`get_batch`、`remove`、`list_languages`
- [x] **验证**：`cargo test --workspace` — 217 个测试，0 个失败。`cargo check --workspace` 通过。

### 进行中
（无）

### 已阻塞
（无）

## 关键决策
- **通过 `.to_string()` 将 `GroupId` 转换为 `String`**：`GroupId` 是一个新类型；在从数据库行构建 `SecurityPolicy` 时使用
- **构建器方法优先于直接字段访问**：`SecurityPolicy.model_accesses` 和 `.record_rules` 是私有的；使用公共方法 `add_model_access()` / `add_record_rule()`
- **初始数据拆分**：在迁移期间播种组/隐含组（始终需要），通过 `AppState::seed_model_access_from_registry()` 从注册表动态播种 `model_access`
- **P2-6 `SecurityPolicy` 缓存已取消**：优先级低，后续可通过 `moka`/`ttl-cache` 添加
- **`EventBus` 处理器类型**：使用 `Box<dyn Fn(Event) -> Pin<Box<dyn Future<Output=Result<(), anyhow::Error>> + Send>> + Send + Sync` 以实现异步回调
- **扩展模块结构**：所有 8 个特性都在 `ingjoo-core/src/extension/` 中，并带有 `mod.rs` 重新导出；遵循与 `db/traits.rs` 相同的模式

## 下一步
1. 所有计划工作已**完成**。没有剩余任务。
2. 潜在的未来工作（不在范围内）：
   - 使用 `tokio::broadcast` 实现具体的 `EventBus`
   - 使用数据库支持的存储实现具体的 `StateMachine`
   - 使用 SQLite FTS5 或 Meilisearch 后端实现 `SearchEngine`
   - 添加 `SecurityPolicy` 缓存（P2-6，已取消）
   - 更新 `dev-docs/*/ROADMAP.md` 以反映完成情况

## 关键上下文
- **测试基线**：217 个测试通过，工作区中 0 个失败
- **工作区根目录**：`/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo`
- **`ingjoo-core` 的 `Cargo.toml` 依赖项**：`async-trait`、`serde`、`serde_json`、`thiserror`、`sqlx`、`tokio`、`chrono`、`uuid` — 均可用于扩展特性
- **扩展特性模式**：`#[async_trait] pub trait Foo: Send + Sync { async fn method(&self, ...) -> Result<T, anyhow::Error>; }`
- **`AppState` 结构体**：`store: Arc<dyn IngjooStore>`，`auth: JwtAuthProvider`，`registry: Arc<ModelRegistry>`，`pool: Arc<Pool>`，`dialect: Dialect`，`events: broadcast::Sender<String>`，`start_time: Instant`
- **CRUD 权限流程**：`resolve_model()` → `load_security_policy()` → `check_access()`（第2层）→ `get_record_filter()` → `GenericDb::generic_*_with_filter()`（第3层）
- **初始数据流程**：`IngjooDb::run_migrations()` → `seed_default_groups()`（组+隐含组）+ `seed::seed_model_access()`（硬编码模型）；`AppState::seed_model_access_from_registry()` 用于动态模型

## 文件操作

### 读取
- `/Users/mom988/.plannotator/plans/plan-2026-04-24-approved.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/ARCHITECTURE.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-bin/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-bin/src`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-bin/src/main.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-bin/tests/integration_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-cache/src`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/error.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/ids.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/models.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/traits.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/dialect.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/lib.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/module`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/module/metadata.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/module/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/module/registry.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/pool.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/query`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/query/domain.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/scope`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/auth/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/config.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/generic.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/migration.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/mock.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/seed.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/traits.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/extractors`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/extractors/auth.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/action.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/auth.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/crud.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/groups.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/menu.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/permission.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/settings.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/ws.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/lib.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/error.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/permission.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/security.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/router.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/state.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/db_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/generic_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/handlers_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/router_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-macros/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-macros/src/lib.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/lib.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/memory.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/sql.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/worker.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/tests/memory_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/tests/sql_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-security/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-security/src`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-security/src/lib.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-security/src/policy.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/dev-docs/en/ROADMAP.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/dev-docs/zh/ROADMAP.md`

### 修改
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/.env.example`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/.github/workflows/ci.yml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/AUDIT-REPORT-2026-04-25.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/Justfile`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-bin/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-bin/src/main.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-bin/tests/integration_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-cache/src/moka_cache.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/error.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/ids.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/models.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/traits.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/extension/event_bus.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/extension/id_generator.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/extension/lock.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/extension/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/extension/payment.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/extension/relations.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/extension/search.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/extension/state_machine.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/extension/translation.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/lib.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/module/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/module/registry.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/query/domain.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/auth/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/config.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/generic.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/migration.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/mock.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/seed.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/traits.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/extractors/auth.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/action.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/auth.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/crud.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/groups.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/health.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/menu.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/permission.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/schedule.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/view.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/ws.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/lib.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/error.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/mod.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/permission.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/ratelimit.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/security.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/router.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/state.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/db_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/derive_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/generic_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/handlers_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/router_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-macros/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-macros/src/lib.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/Cargo.toml`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/lib.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/memory.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/retry.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/scheduler.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/sql.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/src/worker.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/tests/memory_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-queue/tests/sql_test.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-security/src/policy.rs`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/dev-docs/en/DEV-GUIDE.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/dev-docs/en/QUICK-START.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/dev-docs/en/ROADMAP.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/dev-docs/zh/DEV-GUIDE.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/dev-docs/zh/QUICK-START.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/dev-docs/zh/ROADMAP.md`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/rustfmt.toml`
