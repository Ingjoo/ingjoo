# 阶段 4 进度报告

**日期**: 2026-04-25  
**状态**: ✅ 阶段 4 全部完成  
**测试基线**: 218 → 293 (+75 新增)  
**构建**: 0 errors, 0 failures

---

## 总览

| 任务 | 优先级 | 状态 | 交付物摘要 |
|------|--------|------|-----------|
| T4.0 元数据系统 | P0 | ✅ 已完成（之前） | ir_menu / ir_view / ir_action + handlers + 种子数据 |
| T4.1 插件热加载 | P1 | ✅ 已完成 | 线程安全 Registry + PluginManager + 4 API 端点 |
| T4.2 Rustdoc | P1 | ✅ 已完成 | 全 crate 公共 API `///` 文档注释 |
| T4.3 性能基准 | P2 | ✅ 已完成 | criterion 3 组 11 基准 |
| T4.4 集成测试 | P2 | ✅ 已完成 | 55 个集成测试（8 个新增） |
| T4.5 CLI 工具 | P2 | ✅ 已完成 | clap 5 参数 + 环境变量 |
| T4.6 多租户测试 | P2 | ✅ 已完成 | 6 单元 + 5 集成测试 |
| T4.7 连接池可观测 | P3 | ✅ 已完成 | PoolOptions + PoolStats + health 端点 |
| T4.8 限流中间件 | P3 | ✅ 已完成 | 分层可配置限流 |

---

## T4.1 插件/模块热加载系统

### 架构

```
JSON 清单文件 → PluginManifest → PluginManager → ModelRegistry
                                    ↓
                              GenericDb.ensure_table()  →  自动建表
```

### 改动文件

| 文件 | 变更 |
|------|------|
| `ingjoo-core/src/module/registry.rs` | `HashMap` → `RwLock<HashMap>`，所有方法 `&self` |
| `ingjoo-core/src/module/plugin.rs` | **新增**: PluginManifest / PluginInfo / PluginState 类型 |
| `ingjoo-core/src/module/mod.rs` | 新增 `pub mod plugin` + re-exports |
| `ingjoo-infra/src/plugin/manager.rs` | **新增**: PluginManager（load/unload/reload/list） |
| `ingjoo-infra/src/handlers/plugin.rs` | **新增**: 4 个管理 API handler |
| `ingjoo-infra/src/router.rs` | 注册 4 条 admin 路由 |
| `ingjoo-infra/src/state.rs` | 新增 `plugin_manager: Option<Arc<PluginManager>>` |
| `ingjoo-bin/src/main.rs` | PluginManager 初始化 + PLUGINS_DIR 自动加载 |

### API 端点

| 方法 | 路径 | 权限 | 功能 |
|------|------|------|------|
| GET | `/api/admin/plugins` | admin | 列出已加载插件 |
| POST | `/api/admin/plugins/load` | admin | 从目录加载插件 |
| POST | `/api/admin/plugins/{name}/unload` | admin | 卸载插件 |
| POST | `/api/admin/plugins/{name}/reload` | admin | 重载插件 |

### 测试（5 个）
- `test_plugin_list_empty` / `test_plugin_load_from_dir` / `test_plugin_unload`
- `test_plugin_requires_admin` / `test_plugin_load_invalid_dir`

---

## T4.2 公共 API Rustdoc

全 crate 所有公开 trait、struct、enum、fn 添加 `///` 文档注释。
`cargo doc --workspace` 零 missing-doc 警告。

---

## T4.3 性能基准测试（criterion）

### 基准组

| 组 | 文件 | 场景数 | 样例延迟 |
|----|------|--------|---------|
| Domain DSL 解析 | `ingjoo-core/benches/domain_bench.rs` | 4 | simple_leaf ~1.1µs |
| Domain DSL → SQL | 同上 | 3 | and_conditions ~1.9µs |
| Domain 端到端 | 同上 | 1 | parse+compile ~1.8µs |
| Registry 注册 | `ingjoo-core/benches/registry_bench.rs` | 4 | register ~700ns |
| Cache 读写 | `ingjoo-cache/benches/cache_bench.rs` | 4 | put_get ~1.4µs |

### 运行命令

```bash
cargo bench --workspace
```

---

## T4.4 完整集成测试套件

从 42 个扩展到 55 个集成测试。

### 新增测试（8 个）

| 测试 | 覆盖场景 |
|------|---------|
| `test_health_has_pool_stats` | health 端点返回 pool 统计 |
| `test_profile_update` | PUT profile 更新姓名 → GET 验证 |
| `test_crud_update_record` | 动态模型记录更新 |
| `test_crud_delete_record` | 动态模型记录删除 → 404 |
| `test_view_nonadmin_forbidden` | 普通用户创建视图 → 403 |
| `test_action_nonadmin_forbidden` | 普通用户创建动作 → 403 |
| `test_rate_limit_protected_route` | 45 次快速请求 → 触发 429 |
| `test_schedule_crud_full_flow` | 调度任务完整 CRUD |

### 关键发现

- cron 表达式需用 **6 字段格式**（`sec min hour dom month dow`），非传统 5 字段

---

## T4.5 CLI 管理工具

### 新增文件
- `crates/ingjoo-bin/src/cli.rs` — clap derive Cli struct

### 参数映射

| CLI flag | 环境变量 | 默认值 |
|----------|---------|--------|
| `--bind` | `INGJOO_BIND` | `0.0.0.0:3000` |
| `--database-url` | `DATABASE_URL` | `sqlite:./data/ingjoo.db?mode=rwc` |
| `--jwt-secret` | `INGJOO_JWT_SECRET` | `ingjoo-default-secret-change-me` |
| `--log-level` | `INGJOO_LOG` | `ingjoo_bin=info` |
| `--plugins-dir` | `PLUGINS_DIR` | `./plugins` |

### 使用示例

```bash
# 环境变量方式
DATABASE_URL=sqlite:./data/prod.db INGJOO_JWT_SECRET=my-secret ./ingjoo-bin

# CLI 参数方式
./ingjoo-bin --bind 0.0.0.0:8080 --jwt-secret my-secret --log-level debug

# 查看帮助
./ingjoo-bin --help
```

---

## T4.6 多租户集合隔离测试

### 单元测试（6 个，在 `policy.rs`）

| 测试 | 场景 |
|------|------|
| `test_collection_isolation_multiple_collections` | 5 个集合 ID → `IN (?, ?, ?, ?, ?)` |
| `test_collection_isolation_with_table_alias` | 别名 `t0.collection_id IN (...)` |
| `test_collection_isolation_sql_injection_prevention` | 恶意输入作为参数化查询处理 |
| `test_collection_isolation_special_characters` | Unicode / 特殊字符 |
| `test_collection_isolation_single_id` | 单集合 |
| `test_collection_isolation_alias_with_multiple_ids` | 别名 + 多 ID |

### 集成测试（5 个，在 `integration_test.rs`）

| 测试 | 场景 |
|------|------|
| `test_collection_isolation_crud_filter` | Domain 过滤隔离不同集合记录 |
| `test_collection_isolation_cross_collection_denied` | 跨集合记录被过滤 |
| `test_collection_isolation_with_domain_dsl` | 隐式 AND 复合条件 |
| `test_collection_isolation_security_policy_layer3` | SecurityPolicy 层 3 集成验证 |
| `test_collection_isolation_generic_db_filter` | GenericDb 直接 SqlCondition 过滤 |

---

## T4.7 连接池可观测性

### 改动

| 文件 | 变更 |
|------|------|
| `ingjoo-core/src/pool.rs` | 新增 `connect_pool_with_options()` + `PoolStats` struct |
| `ingjoo-infra/src/handlers/health.rs` | health 端点增加 `pool` 统计字段 |
| `ingjoo-bin/src/main.rs` | 改用 `connect_pool_with_options()` |

### 配置默认值

| 参数 | 值 |
|------|---|
| max_connections | 10 |
| min_connections | 1 |
| acquire_timeout | 30s |
| idle_timeout | 600s |
| max_lifetime | 1800s |

### Health 端点响应示例

```json
{
  "status": "ok",
  "database": "connected",
  "pool": {
    "total_connections": 2,
    "idle_connections": 1,
    "active_connections": 1,
    "max_connections": 10
  },
  "uptime_secs": 42
}
```

---

## T4.8 限流中间件配置化

### 改动

| 文件 | 变更 |
|------|------|
| `ingjoo-infra/src/state.rs` | 新增 `RateLimitConfig` struct + AppState 字段 |
| `ingjoo-infra/src/router.rs` | 分层限流：public / protected / admin / CRUD |
| `ingjoo-infra/src/lib.rs` | 导出 `RateLimitConfig` |
| `ingjoo-bin/src/main.rs` | 环境变量读取配置 |

### 分层限流策略

| 路由组 | 令牌容量 | 补充速率 | 配置来源 |
|--------|---------|---------|---------|
| Public（注册/登录） | 10 | 1.0/sec | 环境变量 `INGJOO_RATE_LIMIT_MAX` / `INGJOO_RATE_LIMIT_REFILL` |
| Protected（认证用户） | 30 | 5.0/sec | 硬编码 |
| Admin（管理操作） | 60 | 10.0/sec | 硬编码 |
| CRUD（动态模型） | 30 | 5.0/sec | 硬编码 |

---

## 测试统计

| 指标 | 之前 | 之后 | 变化 |
|------|------|------|------|
| 集成测试 | 42 | 55 | +13 |
| 单元测试（多租户） | 3 | 9 | +6 |
| 基准测试 | 0 | 11 | +11 |
| 总测试数 | 218 | 293 | +75 |
| 失败数 | 0 | 0 | — |

---

## 依赖变更

| 依赖 | 版本 | 用途 |
|------|------|------|
| `clap` | 4 (derive + env) | CLI 参数解析 |
| `criterion` | 0.5 (html_reports) | 性能基准测试 |

---

## 阶段 4 完成度

```
T4.0 ✅  T4.1 ✅  T4.2 ✅  T4.3 ✅  T4.4 ✅
T4.5 ✅  T4.6 ✅  T4.7 ✅  T4.8 ✅

阶段 4：9/9 任务完成 ✅
```
