# Ingjoo 框架 Phase 2/3 审计报告

> 日期: 2026-04-25
> 基线: 217 tests, 0 failures
> 状态: ✅ 全部完成

---

## 一、变更总览

| 阶段 | 任务 | 优先级 | 状态 | 涉及文件数 |
|------|------|--------|------|-----------|
| P2-1 | Domain DSL Dialect 集成 | HIGH | ✅ | 3 |
| P2-2 | CRUD Handler 接入 Layer 2/3 权限过滤 | HIGH | ✅ | 3 |
| P2-3 | 种子数据改为动态模型注册 | MEDIUM | ✅ | 3 |
| P2-4 | Scaff → Ingjoo 公共 API 重命名 | MEDIUM | ✅ | 13 |
| P2-5 | 统一权限检查入口 | MEDIUM | ✅ | 9 |
| P2-6 | SecurityPolicy 缓存 | LOW | ⏭️ 跳过 | — |
| P3-1 | StateMachine trait | MEDIUM | ✅ | 1 |
| P3-2 | EventBus trait | MEDIUM | ✅ | 1 |
| P3-3 | IdGenerator trait | MEDIUM | ✅ | 1 |
| P3-4 | SearchEngine trait | MEDIUM | ✅ | 1 |
| P3-5 | PaymentProvider trait | MEDIUM | ✅ | 1 |
| P3-6 | Lock trait | MEDIUM | ✅ | 1 |
| P3-7 | Relations (One2many/Many2many) | MEDIUM | ✅ | 1 |
| P3-8 | TranslationStore trait | MEDIUM | ✅ | 1 |

---

## 二、Phase 2 详细变更

### P2-1: Domain DSL Dialect 集成

**问题**: Domain DSL 生成 SQL 时未区分 SQLite/PostgreSQL 方言，导致 `ILIKE`、布尔值、日期时间函数等在不同数据库上行为不一致。

**变更**:

| 文件 | 变更内容 |
|------|---------|
| `ingjoo-core/src/query/domain.rs` | 新增 `use crate::Dialect`；`op_to_sql()` 接受 `Option<&Dialect>` 参数；`ILIKE` 在 Postgres 使用 `ILIKE`，SQLite 使用 `LIKE ? COLLATE NOCASE`；布尔值 Postgres 使用 `TRUE/FALSE`，SQLite 使用 `1/0` |
| `ingjoo-core/src/query/domain.rs` | 新增 `to_sql_with_dialect(alias, dialect)` 方法，原 `to_sql(alias)` 委托给新方法（传 `None`） |
| `ingjoo-infra/src/db/generic.rs` | `generic_list()` 和 `generic_count()` 调用 `d.to_sql_with_dialect(None, Some(self.dialect))` |
| `ingjoo-security/src/policy.rs` | 新增 `record_filter_groups_with_dialect()` 方法接受 `Option<&Dialect>`，原 `record_filter_groups()` 委托给新方法 |
| `ingjoo-core/src/query/domain.rs` | 新增 3 个测试: `test_ilike_postgres`, `test_not_ilike_postgres`, `test_ilike_sqlite_explicit` |

**影响范围**: 所有 Domain 查询 + SecurityPolicy 记录规则过滤

---

### P2-2: CRUD Handler 接入 Layer 2/3 权限过滤

**问题**: CRUD handler 仅检查模型级权限（Layer 2），未实现记录级过滤（Layer 3），导致用户可能读取/修改无权访问的记录。

**变更**:

| 文件 | 变更内容 |
|------|---------|
| `ingjoo-infra/src/db/generic.rs` | 新增 `generic_list_with_filter(model, domain, extra_filter, limit, offset)` — 合并 Domain WHERE + SqlCondition WHERE |
| `ingjoo-infra/src/db/generic.rs` | 新增 `generic_read_with_filter(model, id, extra_filter)` — 在 SELECT WHERE 中添加额外过滤条件 |
| `ingjoo-infra/src/handlers/crud.rs` | **完全重写**: 新增 `SqlCondition` 导入、`get_record_filter()` 辅助函数 |
| `ingjoo-infra/src/handlers/crud.rs` | `crud_list`: 使用 `generic_list_with_filter` + record_filter |
| `ingjoo-infra/src/handlers/crud.rs` | `crud_read`: 使用 `generic_read_with_filter` + record_filter |
| `ingjoo-infra/src/handlers/crud.rs` | `crud_create`: 仅 Layer 1 check_access（创建无记录过滤） |
| `ingjoo-infra/src/handlers/crud.rs` | `crud_update`: 先通过 `generic_read_with_filter` 验证记录可见性，再更新 |
| `ingjoo-infra/src/handlers/crud.rs` | `crud_delete`: 先通过 `generic_read_with_filter` 验证记录可见性，再删除 |

**修复的编译错误**:
- `GroupId` 是 newtype 包装器，需 `.to_string()` 转为 `String`
- `SecurityPolicy.model_accesses` / `.record_rules` 是私有字段，改用 `add_model_access()` / `add_record_rule()` 公共方法
- 删除了未使用的 `HashMap` 导入
- 删除了因文件替换残留的重复函数定义（`resolve_model`, `check_access`, `load_security_policy`, 全部 CRUD handler）

**权限执行流程**:
```
resolve_model()           → 验证模型已注册
load_security_policy()    → 从 DB 加载 model_access + record_rules
check_access()            → Layer 2: 模型级 CRUD 权限检查
get_record_filter()       → Layer 3: 记录级 Domain → SqlCondition 转换
generic_*_with_filter()   → 在 SQL 查询中应用过滤条件
```

---

### P2-3: 种子数据改为动态模型注册

**问题**: `seed_default_groups()` 硬编码 `["collection", "entry", "source", "project", "user"]` 模型列表，与动态模型注册表设计矛盾。

**变更**:

| 文件 | 变更内容 |
|------|---------|
| `ingjoo-infra/src/db/seed.rs` | 新增 `seed_model_access(pool, dialect, models: &[&str])` 函数 — 接受动态模型列表，为 admin/user/viewer 三组生成 model_access 行 |
| `ingjoo-infra/src/db/seed.rs` | 新增 `upsert_sql()` 辅助函数 — 生成方言正确的 INSERT OR IGNORE / ON CONFLICT DO NOTHING 语句 |
| `ingjoo-infra/src/db/mod.rs` | `seed_default_groups()` 中的硬编码循环替换为 `seed::seed_model_access(pool, dialect, &models).await?` |
| `ingjoo-infra/src/state.rs` | 新增 `AppState::seed_model_access_from_registry()` 方法 — 从 `self.registry` 读取已注册模型，调用 `seed::seed_model_access()` |

**时序问题解决方案**:
```
Db::run_migrations()      → 创建表 + 种子 groups/implications + 硬编码模型 access
AppState::new()           → 创建 registry (此时可能为空)
registry.register(...)    → 业务模块注册模型
state.seed_model_access_from_registry()  → 为新注册的模型补种 access
```

---

### P2-4: Scaff → Ingjoo 公共 API 重命名

**问题**: 公共 API 使用旧项目名 `Scaff`，与品牌名 `Ingjoo` 不一致。

**重命名映射**:

| 旧名称 | 新名称 | 类型 | 定义位置 |
|--------|--------|------|---------|
| `ScaffStore` | `IngjooStore` | trait | `ingjoo-core/src/db/traits.rs` |
| `ScaffTransaction` | `IngjooTransaction` | trait | `ingjoo-core/src/db/traits.rs` |
| `ScaffDb` | `IngjooDb` | type alias | `ingjoo-infra/src/lib.rs` |
| `MockScaffDb` | `MockIngjooDb` | struct | `ingjoo-infra/src/db/mock.rs` |
| `ScaffConfig` | `IngjooConfig` | struct | `ingjoo-infra/src/config.rs` |

**涉及文件** (45 处引用):

| 文件 | 替换数 |
|------|--------|
| `ingjoo-core/src/db/traits.rs` | 4 |
| `ingjoo-core/src/lib.rs` | 2 |
| `ingjoo-infra/src/lib.rs` | 5 |
| `ingjoo-infra/src/config.rs` | 2 |
| `ingjoo-infra/src/state.rs` | 4 |
| `ingjoo-infra/src/db/traits.rs` | 2 |
| `ingjoo-infra/src/db/mod.rs` | 1 |
| `ingjoo-infra/src/db/mock.rs` | 12 |
| `ingjoo-bin/src/main.rs` | 3 |
| `ingjoo-bin/tests/integration_test.rs` | 5 |
| `ingjoo-infra/tests/db_test.rs` | 3 |
| `ingjoo-infra/tests/router_test.rs` | 3 |
| `ingjoo-infra/tests/handlers_test.rs` | 2 |

**验证**: `grep -r "Scaff[A-Z]" crates/ --include="*.rs"` 返回零结果。

---

### P2-5: 统一权限检查入口

**问题**: admin 权限检查散落在 9 处，使用内联的 `groups.iter().any(|g| g == "admin")` 模式，不易维护。

**变更**:

| 文件 | 变更内容 |
|------|---------|
| `ingjoo-infra/src/extractors/auth.rs` | 新增 `CurrentUser::is_admin() -> bool` 和 `CurrentUser::require_admin() -> Result<(), AppError>` |
| `ingjoo-infra/src/handlers/view.rs` | 替换为 `user.is_admin()` |
| `ingjoo-infra/src/handlers/groups.rs` | 替换为 `user.is_admin()` |
| `ingjoo-infra/src/handlers/schedule.rs` | 替换为 `user.is_admin()` |
| `ingjoo-infra/src/handlers/action.rs` | 替换为 `user.is_admin()` |
| `ingjoo-infra/src/handlers/menu.rs` | 替换为 `user.is_admin()` |
| `ingjoo-infra/src/handlers/permission.rs` | 替换为 `user.is_admin()` |
| `ingjoo-infra/src/handlers/crud.rs` | 2 处替换为 `current_user.is_admin()` |
| `ingjoo-infra/src/middleware/permission.rs` | 替换为 `current_user.is_admin()` |

**新增 API**:
```rust
impl CurrentUser {
    pub fn is_admin(&self) -> bool { ... }
    pub fn require_admin(&self) -> Result<(), AppError> { ... }
}
```

---

### P2-6: SecurityPolicy 缓存 (跳过)

**原因**: 低优先级，当前每次请求从 DB 加载 SecurityPolicy 的性能可接受。后续可通过 `moka` / `ttl-cache` 按需添加。

---

## 三、Phase 3 扩展 Traits

### 新增模块: `ingjoo-core/src/extension/`

所有 8 个 trait 位于此模块，遵循统一模式:
- `#[async_trait]` 宏
- `Send + Sync` 约束
- `thiserror` 错误类型
- `serde` 序列化支持

---

### P3-1: StateMachine (状态机)

**文件**: `extension/state_machine.rs`

```rust
pub struct StateTransition { from, to, label }
pub enum TransitionError { InvalidTransition, MachineNotFound, ConditionNotMet, Internal }

pub trait StateMachine: Send + Sync {
    async fn get_current_state(&self, model, record_id) -> Result<String>;
    async fn get_available_transitions(&self, model, record_id) -> Result<Vec<StateTransition>>;
    async fn transition(&self, model, record_id, target_state, context: HashMap<String, Value>) -> Result<String>;
    async fn register_machine(&self, model, states, transitions, initial_state) -> Result<()>;
}
```

**设计参考**: Odoo `ir.transition` + 工作流引擎模式

---

### P3-2: EventBus (事件总线)

**文件**: `extension/event_bus.rs`

```rust
pub struct Event { topic, payload: Value, source }
pub type EventHandler = Box<dyn Fn(Event) -> Pin<Box<dyn Future<Output=Result<()>> + Send>> + Send + Sync>;

pub trait EventBus: Send + Sync {
    async fn publish(&self, event: Event) -> Result<()>;
    async fn subscribe(&self, topic, handler: EventHandler) -> Result<String>;  // 返回 subscription_id
    async fn unsubscribe(&self, subscription_id) -> Result<()>;
}
```

**设计参考**: `tokio::broadcast` + Odoo `ir.queue.job` 模式

---

### P3-3: IdGenerator (ID 生成器)

**文件**: `extension/id_generator.rs`

```rust
pub trait IdGenerator: Send + Sync {
    fn generate(&self, prefix: &str) -> String;       // 带前缀的有序 ID
    fn generate_uuid(&self) -> String;                  // UUID v4
    fn generate_short_id(&self, length: usize) -> String;  // 短随机 ID
}
```

**注意**: 同步 trait（非 async），适用于热点路径。

---

### P3-4: SearchEngine (搜索引擎)

**文件**: `extension/search.rs`

```rust
pub struct SearchQuery { text, models, limit, offset, filters: Option<Value> }
pub struct SearchResult { model, record_id, score, data: Value, highlights }
pub struct SearchHighlight { field, snippet }

pub trait SearchEngine: Send + Sync {
    async fn index_record(&self, model, record_id, data: &Value) -> Result<()>;
    async fn remove_record(&self, model, record_id) -> Result<()>;
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>>;
    async fn rebuild_index(&self, model) -> Result<()>;
}
```

**预期实现**: SQLite FTS5 / Meilisearch / Elasticsearch

---

### P3-5: PaymentProvider (支付)

**文件**: `extension/payment.rs`

```rust
pub enum PaymentStatus { Pending, Processing, Succeeded, Failed, Cancelled, Refunded }
pub struct PaymentIntent { id, amount: i64, currency, status, metadata: Value }
pub struct PaymentResult { intent_id, status, transaction_id: Option, client_secret: Option }
pub struct RefundResult { refund_id, amount, status }

pub trait PaymentProvider: Send + Sync {
    async fn create_intent(&self, amount, currency, metadata) -> Result<PaymentResult>;
    async fn confirm(&self, intent_id) -> Result<PaymentResult>;
    async fn cancel(&self, intent_id) -> Result<PaymentResult>;
    async fn refund(&self, intent_id, amount: Option<i64>) -> Result<RefundResult>;
    async fn get_status(&self, intent_id) -> Result<PaymentStatus>;
}
```

**设计参考**: Stripe PaymentIntent API

---

### P3-6: Lock (分布式锁)

**文件**: `extension/lock.rs`

```rust
pub struct LockGuard { key, token, expires_at: Instant }
impl LockGuard { fn is_expired(&self) -> bool }

pub trait Lock: Send + Sync {
    async fn acquire(&self, key, ttl_ms: u64) -> Result<LockGuard>;
    async fn try_acquire(&self, key, ttl_ms: u64) -> Result<Option<LockGuard>>;
    async fn extend(&self, guard: &LockGuard, ttl_ms: u64) -> Result<()>;
}
```

**预期实现**: Redis SETNX / PostgreSQL advisory locks / SQLite 文件锁

---

### P3-7: Relations (关联加载)

**文件**: `extension/relations.rs`

```rust
pub trait RelationLoader: Send + Sync {
    async fn load_one2many(&self, model, field, ids) -> Result<HashMap<String, Vec<Value>>>;
    async fn load_many2one(&self, model, field, ids) -> Result<HashMap<String, Option<Value>>>;
}

pub struct Many2Many { table, column_a, column_b, model_a, model_b }
impl Many2Many { fn new(table, model_a, model_b) -> Self }
```

**设计参考**: Odoo `One2many` / `Many2many` 字段类型

---

### P3-8: TranslationStore (多语言)

**文件**: `extension/translation.rs`

```rust
pub struct Translation { lang, model, field, record_id, value }

pub trait TranslationStore: Send + Sync {
    async fn get(&self, lang, model, field, record_id) -> Result<Option<String>>;
    async fn set(&self, translation: Translation) -> Result<()>;
    async fn get_batch(&self, lang, model, field, record_ids) -> Result<HashMap<String, String>>;
    async fn remove(&self, lang, model, field, record_id) -> Result<()>;
    async fn list_languages(&self, model) -> Result<Vec<String>>;
}
```

**设计参考**: Odoo `ir.translation` 表结构

---

## 四、文件变更清单

### 新增文件 (9)

```
crates/ingjoo-core/src/extension/mod.rs
crates/ingjoo-core/src/extension/state_machine.rs
crates/ingjoo-core/src/extension/event_bus.rs
crates/ingjoo-core/src/extension/id_generator.rs
crates/ingjoo-core/src/extension/search.rs
crates/ingjoo-core/src/extension/payment.rs
crates/ingjoo-core/src/extension/lock.rs
crates/ingjoo-core/src/extension/relations.rs
crates/ingjoo-core/src/extension/translation.rs
```

### 修改文件 (22)

```
crates/ingjoo-core/src/lib.rs                           # 新增 pub mod extension
crates/ingjoo-core/src/query/domain.rs                  # Dialect 集成 + 3 新测试
crates/ingjoo-core/src/db/traits.rs                     # ScaffStore → IngjooStore 重命名

crates/ingjoo-security/src/policy.rs                    # record_filter_groups_with_dialect()

crates/ingjoo-infra/src/lib.rs                          # 重命名导出
crates/ingjoo-infra/src/state.rs                        # IngjooStore + seed_model_access_from_registry()
crates/ingjoo-infra/src/config.rs                       # ScaffConfig → IngjooConfig
crates/ingjoo-infra/src/extractors/auth.rs              # is_admin() + require_admin()
crates/ingjoo-infra/src/db/mod.rs                       # seed_default_groups 调用新函数
crates/ingjoo-infra/src/db/traits.rs                    # 重命名导入
crates/ingjoo-infra/src/db/mock.rs                      # MockScaffDb → MockIngjooDb
crates/ingjoo-infra/src/db/seed.rs                      # 新增 seed_model_access()
crates/ingjoo-infra/src/db/generic.rs                   # 新增 generic_list/read_with_filter()
crates/ingjoo-infra/src/handlers/crud.rs                # 全部 handler 接入 Layer 2/3
crates/ingjoo-infra/src/handlers/view.rs                # is_admin()
crates/ingjoo-infra/src/handlers/groups.rs              # is_admin()
crates/ingjoo-infra/src/handlers/schedule.rs            # is_admin()
crates/ingjoo-infra/src/handlers/action.rs              # is_admin()
crates/ingjoo-infra/src/handlers/menu.rs                # is_admin()
crates/ingjoo-infra/src/handlers/permission.rs          # is_admin()
crates/ingjoo-infra/src/middleware/permission.rs        # is_admin()

crates/ingjoo-bin/src/main.rs                           # IngjooDb/IngjooStore
crates/ingjoo-bin/tests/integration_test.rs             # IngjooDb/IngjooStore
crates/ingjoo-infra/tests/db_test.rs                    # IngjooDb/IngjooStore
crates/ingjoo-infra/tests/router_test.rs                # IngjooDb/IngjooStore
crates/ingjoo-infra/tests/handlers_test.rs              # MockIngjooDb/IngjooStore
```

---

## 五、测试结果

```
cargo test --workspace
```

| Crate | 测试数 | 通过 | 失败 | 忽略 |
|-------|--------|------|------|------|
| ingjoo-bin (integration) | 37 | 37 | 0 | 0 |
| ingjoo-cache (moka) | 5 | 5 | 0 | 0 |
| ingjoo-core (unit) | 71 | 71 | 0 | 0 |
| ingjoo-infra (unit) | 17 | 17 | 0 | 0 |
| ingjoo-infra (db_test) | 23 | 23 | 0 | 0 |
| ingjoo-infra (derive_test) | 4 | 4 | 0 | 0 |
| ingjoo-infra (generic_test) | 13 | 13 | 0 | 0 |
| ingjoo-infra (router_test) | 7 | 7 | 0 | 0 |
| ingjoo-queue (unit) | 2 | 2 | 0 | 0 |
| ingjoo-queue (memory_test) | 14 | 14 | 0 | 0 |
| ingjoo-queue (sql_test) | 9 | 9 | 0 | 0 |
| ingjoo-security (unit) | 16 | 16 | 0 | 0 |
| **总计** | **217** | **217** | **0** | **0** |

---

## 六、已知遗留项

| 项目 | 状态 | 说明 |
|------|------|------|
| P2-6 SecurityPolicy 缓存 | 跳过 | 低优先级，可后续用 moka/ttl-cache 实现 |
| Extension trait 具体实现 | 待定 | 当前仅定义 trait 接口，未提供默认实现 |
| StateMachine DB 后端 | 待定 | 需新建 `ir_state_machine` / `ir_state_transition` 表 |
| EventBus 广播实现 | 待定 | 可基于 `tokio::broadcast` 或 Redis Pub/Sub |
| SearchEngine FTS5 实现 | 待定 | 可在 `ingjoo-infra` 中 feature-gate 实现 |
| PaymentProvider Stripe 实现 | 待定 | 应独立为 `ingjoo-payment` crate |
| Lock Redis 实现 | 待定 | 应独立为 `ingjoo-lock` crate |

---

*报告生成时间: 2026-04-25*
*基线提交: 无 (非 git 仓库)*
