# ingjoo 项目进度报告

**日期**: 2026-04-26  
**版本**: v0.1.0  
**状态**: ✅ 阶段 1-13 全部完成  
**测试基线**: 415 单元测试 + 集成测试通过，0 失败  
**构建**: 0 errors, 0 clippy warnings, TypeScript 通过, `next build` 通过  
**代码规模**: ~25,500 行 Rust，130+ 源文件，12 页面  

---

## 一、总览

```
阶段  1  让框架能跑              ✅  2026-04-18
阶段  2  让框架可靠              ✅  2026-04-19
阶段  3  让框架名副其实          ✅  2026-04-20
阶段  4  让框架可扩展            ✅  2026-04-20
阶段  5  扩展激活                ✅  2026-04-21
阶段  6  生产就绪                ✅  2026-04-21
阶段  7  扩展实现（DB 后端）     ✅  2026-04-22
阶段 7.5 输入安全                ✅  2026-04-23
阶段  8  搜索集成 + QA           ✅  2026-04-24
阶段  9  前端对接                ✅  2026-04-26
阶段 10  模块系统 + 空库体验     ✅  2026-04-26
阶段 11  通知系统 + 前端增强     ✅  2026-04-26
阶段 12  前端通知铃铛 + SSE 修复  ✅  2026-04-26
阶段 13  集成测试修复 + Profile 完善  ✅  2026-04-26

---

## 二、项目规模

### 2.1 后端（Rust）

| Crate | 代码行数 | 文件数 | 测试数 | 成熟度 |
|-------|---------|--------|--------|--------|
| `ingjoo-core` | 3,920 | 40 | 82 | **成熟** — Domain DSL、Store trait、Dialect、NotificationStore |
| `ingjoo-infra` | 15,800 | 75 | 197 | **成熟** — DB、认证、handler、中间件、15 扩展实现、通知系统 |
| `ingjoo-security` | 622 | 2 | 22 | **成熟** — 三层 RBAC 引擎 |
| `ingjoo-bin` | 2,650 | 4 | 80 | **可用** — axum 路由、CLI、80 集成测试 |
| `ingjoo-queue` | 1,585 | 8 | 25 | **完整** — 内存/SQL 双后端、WorkerPool、Cron |
| `ingjoo-cache` | 342 | 3 | 5 | **完整** — Moka 缓存 |
| `ingjoo-macros` | 210 | 1 | — | **可用** — `#[derive(IngjooModel)]` |

**Rust 后端合计**：~25,500 行代码，130+ 文件，415 单元测试 + 集成测试。

### 2.2 前端

| 仓库 | 技术 | 说明 | 状态 |
|------|------|------|------|
| `@ingjoo/web` | React + TypeScript + Rollup | 共享组件库（auth/i18n/fetch/Domain DSL/模块 API） | ✅ |
| `web-base` | Next.js 16 + Tailwind CSS 4 | 业务前端，12 个页面 | ✅ |

### 2.3 前端页面清单

| 页面 | 路由 | 状态 |
|------|------|------|
| 登录 | `/login` | ✅ 完整功能 |
| 注册 | `/login?register=1` | ✅ 完整功能 |
| 忘记密码 | `/forgot-password` | ✅ 完整功能 |
| 仪表盘 | `/dashboard` | ✅ 完整功能 |
| 个人中心 | `/profile` | ✅ 完整功能 |
| 通知中心 | `/notifications` | ✅ 完整功能 |
| 管理后台 | `/admin` | ✅ 完整（用户管理 + 角色编辑 + 创建用户 + 审计日志过滤） |
| 应用管理 | `/admin/apps` | ✅ 模块安装/卸载 |
| 系统设置 | `/admin/settings` | ✅ 分组设置编辑器 |
| 全局搜索 | `/search` | ✅ 完整（类型筛选 + URL 同步 + 300ms 防抖） |

### 2.4 数据库迁移

| 版本 | 表 | 用途 |
|------|---|------|
| v1 | users | 用户表 |
| v2 | settings, refresh_tokens | 系统设置 + token 存储 |
| v3 | model_accesses, record_rules, groups, group_implied, user_groups | 权限引擎 |
| v4 | queue_jobs | 任务队列 |
| v5 | scheduled_jobs | 定时任务 |
| v6 | ir_menu, ir_view, ir_action | 菜单/视图/动作元数据 |
| v7 | ir_attachment | 附件存储 |
| v8 | ir_translation | 多语言翻译 |
| v9 | ir_state_machine, ir_state_transition, ir_state_record | 状态机 |
| v10 | ir_search_index | 搜索索引 |
| v11 | ir_module | 模块注册表 |
| v12 | ir_settings_definition | 设置定义目录 |
| v14 | mail_messages, mail_notifications | 通知消息系统 |
| v15 | model_access (perm_import/perm_export) | 导入导出权限字段 |

---

## 三、路线图详情

### 阶段 1：让框架能跑 ✅

> 目标：实现端到端 HTTP 请求流，串联所有已有组件。

| # | 任务 | 优先级 | 状态 |
|---|------|--------|------|
| T1.1 | axum 路由骨架 | **P0** | ✅ |
| T1.2 | JWT 提取器 + Auth 中间件 | **P0** | ✅ |
| T1.3 | TokenClaims 增加 role + groups | **P0** | ✅ |
| T1.4 | SecurityPolicy 接线到 CRUD | **P0** | ✅ |
| T1.5 | User CRUD handler | **P1** | ✅ |
| T1.6 | Settings CRUD handler | **P1** | ✅ |
| T1.7 | 集成测试 | **P1** | ✅ |

### 阶段 2：让框架可靠 ✅

> 目标：防止回归、修复错误处理、规范化迁移。

| # | 任务 | 优先级 | 状态 |
|---|------|--------|------|
| T2.1 | GitHub Actions CI | **P0** | ✅ |
| T2.2 | ingjoo-infra 测试覆盖 | **P0** | ✅ |
| T2.3 | StoreError 类型化错误体系 | **P1** | ✅ |
| T2.4 | 版本化迁移系统 (v1-v4) | **P1** | ✅ |
| T2.5 | Justfile 开发命令 | **P1** | ✅ |
| T2.6 | .env.example | **P2** | ✅ |
| T2.7 | rustfmt.toml | **P2** | ✅ |

### 阶段 3：让框架名副其实 ✅

> 目标：动态模型注册、队列、定时任务、关系字段。

| # | 任务 | 优先级 | 状态 |
|---|------|--------|------|
| T3.1 | 动态模型注册系统 (ModelRegistry + GenericDb) | **P0** | ✅ |
| T3.2 | `#[derive(IngjooModel)]` 派生宏 | **P1** | ✅ |
| T3.3 | 权限持久化 (model_accesses + record_rules) | **P1** | ✅ |
| T3.4 | ingjoo-queue crate (内存/SQL 双后端) | **P1** | ✅ |
| T3.5 | 定时任务 Cron 调度器 | **P1** | ✅ |
| T3.6 | Many2one 关系字段 + FK 约束 | **P2** | ✅ |
| T3.7 | 审计日志字段自动填充 | **P2** | ✅ |
| T3.8 | WebSocket 广播通知 | **P2** | ✅ |

### 阶段 4：让框架可扩展 ✅

> 目标：菜单/视图/动作元数据、插件系统、CLI。

| # | 任务 | 优先级 | 状态 |
|---|------|--------|------|
| T4.0 | 菜单 + 视图 + 动作元数据系统 | **P0** | ✅ |
| T4.1 | 插件/模块热加载 | **P1** | ✅ |
| T4.2 | 公共 API rustdoc | **P1** | ✅ |
| T4.3 | 性能基准测试 (criterion) | **P2** | ✅ |
| T4.5 | ingjoo-cli 管理工具 | **P2** | ✅ |
| T4.6 | 多租户集合隔离测试 | **P2** | ✅ |
| T4.7 | 数据库连接池可观测性 | **P3** | ✅ |
| T4.8 | 限流中间件 | **P3** | ✅ |

### 阶段 5：扩展激活 ✅

> 目标：安全头、noop 补全、缓存集成。

| # | 任务 | 优先级 | 状态 |
|---|------|--------|------|
| T5.1 | 安全响应头中间件 | **P0** | ✅ |
| T5.2 | 5 个缺失 trait 的 noop 实现 | **P0** | ✅ |
| T5.3 | 扩展 feature flag | **P0** | ✅ |
| T5.4 | FrameworkCache 集成到 AppState | **P1** | ✅ |

### 阶段 6：生产就绪 ✅

> 目标：14 个扩展 trait 接入 AppState、条件编译、审计日志、缓存测试。

| # | 任务 | 优先级 | 状态 |
|---|------|--------|------|
| T6.1 | AppState 接入 14 个扩展 trait | **P0** | ✅ |
| T6.2 | main.rs 条件编译 (feature-gated) | **P0** | ✅ |
| T6.3 | CRUD handler 审计日志 | **P0** | ✅ |
| T6.4 | 缓存集成测试 | **P1** | ✅ |
| T6.5 | --all-features 编译修复 | **P0** | ✅ |

### 阶段 7：扩展实现 ✅

> 目标：DB 后端替换 noop — EventBus、IdGenerator、Lock、RelationLoader、TranslationStore、StateMachine、TextSplitter。

| # | 任务 | 优先级 | 状态 | 实现 |
|---|------|--------|------|------|
| T7.1 | EventBus | **P0** | ✅ | tokio broadcast，256 容量通道 |
| T7.2 | IdGenerator | **P0** | ✅ | uuid v4 |
| T7.3 | Lock | **P0** | ✅ | SQLite 顾问锁，带过期时间 |
| T7.4 | DbRelationLoader | **P1** | ✅ | 批量 Many2one + display_name |
| T7.5 | DbTranslationStore | **P1** | ✅ | ir_translation CRUD |
| T7.6 | DbStateMachine | **P1** | ✅ | 注册/转换/查询（3 表） |
| T7.7 | CharTextSplitter | **P1** | ✅ | 字符计数分块，带重叠 |
| T7.8 | 迁移 v8/v9 | **P0** | ✅ | ir_translation、ir_state_machine DDL |
| T7.9 | state.rs 接线 + main.rs | **P0** | ✅ | 真实默认值替代 noop |

### 阶段 7.5：输入安全 ✅

| # | 任务 | 交付物 |
|---|------|--------|
| T7.5.1 | HtmlInputSanitizer | 标签白名单、被阻标签内容抑制、属性剥离 |
| T7.5.2 | FsDocumentLoader | 递归文件扫描，10MB 限制，元数据提取 |
| T7.5.3 | state.rs 真实默认值 | sanitizer/document_loader/text_splitter 替代 noop |
| T7.5.4 | 集成测试 | 7 translation + 7 state_machine |

### 阶段 8：搜索集成 + QA ✅

| # | 任务 | 优先级 | 状态 |
|---|------|--------|------|
| T8.1 | DbSearchEngine 实现 | **P0** | ✅ |
| T8.2 | 搜索接线 (build_search + with_search) | **P0** | ✅ |
| T8.3 | UTF-8 char 边界修复 | **P0** | ✅ |
| T8.4 | Clippy 清零 | **P0** | ✅ |
| T8.5 | 搜索集成测试 | **P0** | ✅ |
| T8.6 | 文档同步 | **P1** | ✅ |
| T8.7 | QA 系统化测试 (18 端点) | **P0** | ✅ |
| T8.8 | QA 修复：logout 路由 | **P0** | ✅ |
| T8.9 | QA 修复：/api/auth/me | **P0** | ✅ |
| T8.10 | 种子数据激活 | **P1** | ✅ |

### 阶段 9：前端对接 ✅

> 目标：修复前后端对接，实现完整登录→Dashboard 流程。

| # | 任务 | 优先级 | 状态 |
|---|------|--------|------|
| T9.1 | /register 页面 (?register=1) | **P0** | ✅ |
| T9.2 | Auth token 持久化 (localStorage + Authorization header) | **P0** | ✅ |
| T9.3 | /api/auth/me 前端对接 | **P0** | ✅ |
| T9.4 | 通知 API 前端 stub (4 个 Route Handler) | **P1** | ✅ |
| T9.5 | admin/search 骨架页面 | **P2** | ✅ |
| T9.6 | 后端新端点 (dashboard/users/audit-log) | **P0** | ✅ |
| T9.7 | Preferences 路径对齐 | **P1** | ✅ |
| T9.8 | 死代码清理 | **P2** | ✅ |

### 阶段 10：模块系统 + 空库体验 ✅

> 目标：新数据库首次启动即可用——默认管理员、设置定义、模块管理页面。

| # | 任务 | 优先级 | 改动范围 | 状态 |
|---|------|--------|---------|------|
| T1 | 迁移 v11 — ir_module 表 | **P0** | `migration.rs` | ✅ |
| T2 | 迁移 v12 — ir_settings_definition 表 | **P0** | `migration.rs` | ✅ |
| T3-T5 | seed_core_data() — 默认管理员 + 设置定义 | **P0** | `seed.rs` | ✅ |
| T6 | GET /api/settings/definitions handler | **P0** | `handlers/settings.rs` | ✅ |
| T7-T8 | handlers/modules.rs — 列表/安装/卸载/升级 | **P0** | `handlers/modules.rs` 🆕 | ✅ |
| T9 | 路由注册 | **P0** | `router.rs` + `handlers/mod.rs` | ✅ |
| T10 | seed_core_data() 接入 main.rs 启动 | **P0** | `main.rs` | ✅ |
| T11-T13 | @ingjoo/web SDK 类型 + API 函数 | **P1** | `module-api.ts` 🆕 + `types.ts` + `index.ts` | ✅ |
| T14 | /admin/apps 页面 | **P1** | `page.tsx` 🆕 | ✅ |
| T15 | /admin/settings 页面 | **P1** | `page.tsx` 🆕 | ✅ |
| T16 | ModuleRegistrations 组件 + 侧边栏菜单 | **P1** | `ModuleRegistrations.tsx` 🆕 + `layout.tsx` | ✅ |
| T17 | @ingjoo/web 重新构建 | **P1** | npm build | ✅ |
| T18 | 登录页 Suspense boundary 修复 | **P1** | `login/page.tsx` | ✅ |

**退出标准**：✅ 新建数据库启动后，admin/admin 可登录，/api/settings/definitions 返回 3 条定义，/api/modules CRUD 全流程通过，前端 admin/apps 和 admin/settings 页面渲染正常。

### 阶段 11：通知系统 + 前端增强 ✅

> 目标：实现后端通知 CRUD + SSE 实时推送，增强管理后台和搜索页面，配置 WebSocket 代理。

#### 后端：通知系统

| # | 任务 | 改动范围 | 状态 |
|---|------|---------|------|
| T11.1 | 迁移 v14 — `mail_messages` + `mail_notifications` 表 | `migration.rs` | ✅ |
| T11.2 | 迁移 v15 — `model_access` 增加 `perm_import`/`perm_export` | `migration.rs` | ✅ |
| T11.3 | `MailMessage`、`MailNotification`、`NotificationItem` 结构体 | `models.rs` | ✅ |
| T11.4 | `NotificationStore` trait（6 异步方法） | `extension/notification.rs` 🆕 | ✅ |
| T11.5 | `DbNotificationStore` 实现（JOIN 查询 + 布尔映射） | `extension_impl/notification.rs` 🆕 | ✅ |
| T11.6 | `NoopNotificationStore` 默认实现 | `extension_noop.rs` | ✅ |
| T11.7 | `MockIngjooDb` 通知存储实现 | `db/mock.rs` | ✅ |
| T11.8 | 4 个通知 handler（list/unread-count/mark-read/mark-all-read） | `handlers/notification.rs` 🆕 | ✅ |
| T11.9 | SSE 事件推送 handler（`futures_util::stream::unfold`） | `handlers/events.rs` 🆕 | ✅ |
| T11.10 | AppState 接线 + 路由注册（5 端点）+ main.rs 构建 | `state.rs` + `router.rs` + `main.rs` | ✅ |
| T11.11 | 删除前端通知 stub（4 个 Route Handler） | `api/notifications/` 🗑️ | ✅ |

**新增 API 端点**：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/notifications` | GET | 通知列表（分页） |
| `/api/notifications/unread-count` | GET | 未读计数 |
| `/api/notifications/mark-all-read` | POST | 全部标记已读 |
| `/api/notifications/{id}/mark-read` | POST | 单条标记已读 |
| `/api/events` | GET | SSE 实时推送 |

#### 前端：页面增强

| # | 任务 | 改动范围 | 状态 |
|---|------|---------|------|
| T11.F1 | 管理后台 — 角色编辑（内联下拉 + 确认） | `admin/page.tsx` | ✅ |
| T11.F2 | 管理后台 — 创建用户对话框（表单 + 验证） | `admin/page.tsx` | ✅ |
| T11.F3 | 管理后台 — 审计日志过滤（操作类型 + 日期范围） | `admin/page.tsx` | ✅ |
| T11.F4 | 搜索页 — 类型筛选（6 种类型 toggle） | `search/page.tsx` | ✅ |
| T11.F5 | 搜索页 — URL 参数同步（`?q=&types=`） | `search/page.tsx` | ✅ |
| T11.F6 | 搜索页 — 300ms 防抖 + `globalSearch` API | `search/page.tsx` | ✅ |
| T11.F7 | WebSocket 代理配置（`/ws` → `localhost:3000/ws`） | `next.config.ts` | ✅ |
| T11.F8 | Toast 通知系统（成功/失败，3s 自动消失） | `admin/page.tsx` | ✅ |

**退出标准**：✅ 415 测试通过，前端构建 0 错误，通知 API 可用，管理后台角色编辑/创建用户/审计过滤完整，搜索页类型筛选+防抖+URL同步完整。

### 阶段 12：前端通知铃铛 + SSE 认证修复 ✅

> 目标：完成通知铃铛前端对接，修复 SSE 无法发送 Bearer token 的认证缺陷。

| # | 任务 | 改动范围 | 状态 |
|---|------|---------|------|
| T12.1 | NotificationBell 组件（badge + 下拉 + mark-read + mark-all-read） | `NotificationBell.tsx` | ✅ |
| T12.2 | SSE 认证修复 — fetch-based SSE 替代 EventSource | `useSSE.ts` | ✅ |
| T12.3 | 通知中心完整页面（分页 + 筛选 + SSE 实时刷新） | `notifications/page.tsx` | ✅ |
| T12.4 | 共享 timeAgo 工具函数提取 | `utils.ts` 🆕 | ✅ |
| T12.5 | Header 通知铃铛接线 | `Header.tsx` | ✅ |

**退出标准**：✅ NotificationBell 显示未读计数 + 下拉列表，SSE 通过 fetch 发送 Bearer token，timeAgo 共享函数消除重复代码。

### 阶段 13：集成测试修复 + Profile 完善 ✅

> 目标：解决集成测试 runtime-in-runtime 问题，完善 Profile 页面功能，提升代码质量和用户体验。

| # | 任务 | 改动范围 | 状态 |
|---|------|---------|------|
| T13.1 | 修复 handlers_test.rs tokio runtime 嵌套问题 | `ingjoo-infra/tests/handlers_test.rs` | ✅ 验证通过（15 测试全通过，问题不存在） |
| T13.2 | Profile 头像上传（前端 + 后端 multipart endpoint） | `profile/page.tsx` + `handlers/auth.rs` + `router.rs` + `api.ts` | ✅ |
| T13.3 | Profile 偏好设置持久化（PUT /api/auth/preferences 对接） | `profile/page.tsx` | ✅ 已实现（usePreferences → api → 后端完整链路） |
| T13.4 | 项目文档纳入版本控制（docs/ → ingjoo repo） | `source/ingjoo/docs/` | ✅ |
| T13.5 | ROADMAP 页面数修正（8→12） | `dev-docs/en/ROADMAP.md` + `dev-docs/zh/ROADMAP.md` | ✅ |
| T13.6 | 前端构建修复 — NavMenu 类型 + Suspense + usePreferences 类型 | `header.tsx` + `login/page.tsx` + `api.ts` | ✅ |
| T13.7 | 头像安全加固 — 路径遍历防护 + 死代码清理 + 集成测试 | `handlers/auth.rs` + `integration_test.rs` | ✅ 4 个头像测试 + 路径遍历拒绝 |

**新增 API 端点**：

| 端点 | 方法 | 认证 | 说明 |
|------|------|------|------|
| `/api/auth/avatar` | POST | 受保护 | 上传头像（multipart, 2MB, image-only） |
| `/api/avatars/{filename}` | GET | 公共 | 头像文件服务 |

**退出标准**：✅ 集成测试全量通过（415，0 failures），Profile 页面支持头像上传 + 偏好设置持久化，前端构建 0 错误。

---

## 四、API 端点清单

### 4.1 认证相关

| 端点 | 方法 | 认证 | 说明 |
|------|------|------|------|
| `/api/auth/register` | POST | 公共 | 用户注册 |
| `/api/auth/login` | POST | 公共 | 用户登录 |
| `/api/auth/refresh` | POST | 公共 | 刷新 token |
| `/api/auth/logout` | POST | 受保护 | 登出（清除 refresh token） |
| `/api/auth/profile` | GET | 受保护 | 获取当前用户信息 |
| `/api/auth/me` | GET | 受保护 | 当前用户（前端刷新用） |
| `/api/auth/preferences` | GET/PUT | 受保护 | 用户偏好 |
| `/api/auth/change-password` | POST | 受保护 | 修改密码 |
| `/api/auth/avatar` | POST | 受保护 | 上传头像 |
| `/api/avatars/{filename}` | GET | 公共 | 头像文件服务 |

### 4.2 数据操作

| 端点 | 方法 | 认证 | 说明 |
|------|------|------|------|
| `/api/data/{model}` | GET/POST | 受保护 | 列表查询（Domain DSL）/ 创建 |
| `/api/data/{model}/{id}` | GET/PUT/DELETE | 受保护 | 单条 CRUD |

### 4.3 系统管理

| 端点 | 方法 | 认证 | 说明 |
|------|------|------|------|
| `/api/settings` | GET/PUT | 受保护 | 系统设置值 |
| `/api/settings/definitions` | GET | 管理员 | 设置定义目录 |
| `/api/modules` | GET/POST | 管理员 | 模块列表 / 安装模块 |
| `/api/modules/{name}` | DELETE/PUT | 管理员 | 卸载 / 升级模块 |
| `/api/menus` | GET/POST/PUT/DELETE | 受保护 | 菜单管理 |
| `/api/views` | GET/POST/PUT/DELETE | 受保护 | 视图管理 |
| `/api/actions` | GET/POST/PUT/DELETE | 受保护 | 动作管理 |
| `/api/schedules` | GET/POST/PUT/DELETE | 管理员 | 定时任务 |
| `/api/dashboard/stats` | GET | 受保护 | 仪表盘统计 |
| `/api/users/search` | GET | 受保护 | 用户搜索 |
| `/api/admin/audit-log` | GET | 管理员 | 审计日志 |
| `/api/permissions/*` | GET/POST/PUT/DELETE | 管理员 | 权限管理 |
| `/api/notifications` | GET | 受保护 | 通知列表 |
| `/api/notifications/unread-count` | GET | 受保护 | 未读计数 |
| `/api/notifications/mark-all-read` | POST | 受保护 | 全部标记已读 |
| `/api/notifications/{id}/mark-read` | POST | 受保护 | 单条标记已读 |
| `/api/events` | GET | 受保护 | SSE 实时推送 |

### 4.4 公共端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/health` | GET | 健康检查 |
| `/ws` | WebSocket | 广播通知 |
| `/api/captcha` | GET | 验证码（feature-gated） |

---

## 五、架构现状图

```
                         ┌───────────────┐
                         │   web-base    │  Next.js 16 前端
                          │  (12 页面)   │  localhost:3001
                         └───────┬───────┘
                                 │ API 调用
                         ┌───────▼───────┐
                         │  ingjoo-bin   │  ✅ 完整 HTTP 服务
                         │  (2,585 行)   │  localhost:3000
                         └───────┬───────┘
                                 │
               ┌─────────────────┼─────────────────┐
               │                 │                  │
       ┌───────▼──────┐  ┌──────▼──────┐  ┌───────▼──────┐
       │  middleware   │  │  handlers   │  │   router     │
        │  auth/sec/db  │  │  20 模块    │  │  30+ 端点    │
       └───────┬──────┘  └──────┬──────┘  └──────────────┘
               │                │
      ┌────────┼────────────────┼────────────┐
      │        │                │            │
┌─────▼───┐ ┌──▼────────┐ ┌────▼─────┐ ┌───▼──────┐
│security │ │   cache   │ │   core   │ │  queue   │
│三层 RBAC│ │  Moka     │ │trait+DSL │ │SQL+内存  │
│已接线   │ │缓存策略   │ │          │ │(22 测试) │
└────┬────┘ └────┬──────┘ └────┬─────┘ └───┬──────┘
     │           │              │           │
     └───────────┴──────┬───────┴───────────┘
                        │
                 ┌──────▼──────┐
                  │ ingjoo-infra│  ✅ (197 测试)
                  │DB/认证/存储 │
                  │15 扩展实现  │
                 └─────────────┘
```

---

## 六、技术债务

| 问题 | 严重程度 | 说明 |
|------|---------|------|
| E2E 测试框架 | 低 | Playwright 自动化 — Phase 14+ |

> **已解决**（Phase 11+集成测试补充）：~~retry 测试偶发失败~~ → jitter 阈值修正（1.5→1.25）；~~WebSocket 前端代理~~ → 已配置 rewrites；~~通知系统后端~~ → 已实现 NotificationStore + handler；~~admin 页面功能~~ → 角色编辑 + 创建用户 + 审计过滤；~~search 页面~~ → 类型筛选 + 防抖 + URL 同步；~~Header 通知铃铛~~ → NotificationBell + fetch-based SSE + 共享 timeAgo；~~项目文档未纳入版本控制~~ → docs/ 已迁入 ingjoo repo；~~ROADMAP 页面数不一致~~ → 统一修正为 12。

---

## 七、种子数据

空数据库首次启动时自动填充：

| 数据 | 内容 |
|------|------|
| 管理员用户 | `admin@ingjoo.local` / `admin`（role=admin） |
| 用户组 | admin、user、viewer + implied 关系 |
| 设置定义 | site_name（莺竹）、allow_registration（true）、default_language（zh-CN） |
| 菜单 | 仪表盘、应用管理、系统设置 |
| 动作 | dashboard、apps、settings |

---

## 八、里程碑时间线

```
2026-04-18  Phase 1    ██████       让框架能跑
2026-04-19  Phase 2    ██████       让框架可靠
2026-04-20  Phase 3-4  ██████████   名副其实 + 可扩展
2026-04-21  Phase 5-6  ██████████   扩展激活 + 生产就绪
2026-04-22  Phase 7    ██████       扩展实现 (DB 后端)
2026-04-23  Phase 7.5  ████         输入安全
2026-04-24  Phase 8    ██████       搜索集成 + QA
2026-04-26  Phase 9    ██████       前端对接
2026-04-26  Phase 10   ██████       模块系统 + 空库体验
2026-04-26  Phase 11   ██████       通知系统 + 前端增强
2026-04-26  Phase 12   ████         前端通知铃铛 + SSE 修复
2026-04-26  Phase 13   ████         集成测试修复 + Profile 完善
                                         ✅ 完成
```

---

## 九、QA 调查结果 — 已确认预期行为

> Phase 17 QA 期间发现以下端点返回空数据，经代码调查确认为预期行为：

| 端点 | 返回 | 原因 | 结论 |
|------|------|------|------|
| `GET /api/actions` | 0 项 | `require_admin()` 前置检查，QA 使用普通用户 | ✅ 预期：admin-only |
| `GET /api/menus` | 0 项 | 同上，菜单管理为 admin 功能 | ✅ 预期：admin-only |
| `GET /api/articles` | 空 | 测试用户无 article 数据，seed 仅创建 admin demo 数据 | ✅ 预期：无数据 |
| `GET /api/search?q=test` | 空 | `seed_core_data` 仅索引 admin demo records | ✅ 预期：无索引 |

---

## 十、下一步建议

> Phase 17 已完成（418 tests default / 426 with mock, 0 failures）。以下为 Phase 18+ 待办。

| 优先级 | 任务 | 对应计划 | 预估工时 | 说明 |
|--------|------|---------|---------|------|
| **P1** | 多数据库管理（Odoo 对齐） | Phase 18 (PROGRESS-PLAN T18-*) | 8 天 | 配置层 + API + 启动行为 + 前端 |
| **P1** | E2E 测试框架 | T4.4 | 2 天 | Playwright 自动化（登录→Dashboard→Admin 全流程） |
| **P2** | 自动化规则引擎 | — | 3-5 天 | 基于事件触发的自动化规则系统 |
| **P2** | 邮件/短信真实 provider | — | 2-3 天 | 替换 noop，接入 SMTP / 短信网关 |

---

*报告生成时间: 2026-04-26*
