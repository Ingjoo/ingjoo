# Improvement Roadmap

> Last updated: 2026-04-26
> Status: v0.1.0 — Phase 1-17 complete

## Current State Summary

| Crate | Lines | Tests | Maturity | Status |
|-------|-------|-------|----------|--------|
| `ingjoo-core` | 3,920 | 82 | **Mature** | Domain DSL, Store traits, Dialect, NotificationStore |
| `ingjoo-infra` | 15,800 | 197 | **Mature** | Full DB impl, auth, storage, handlers, middleware, router, 15 extension impls |
| `ingjoo-security` | 622 | 22 | **Mature** | 3-layer RBAC engine — wired to CRUD handlers |
| `ingjoo-cache` | 342 | 5 | **Complete** | Moka-based caching — working |
| `ingjoo-queue` | 1,585 | 25 | **Complete** | In-memory + SQL queue, WorkerPool, retry policy, cron scheduler |
| `ingjoo-bin` | 2,650 | 80 | **Working** | Full axum router with 80 integration tests passing |
| `ingjoo-macros` | 210 | — | **Working** | `#[derive(IngjooModel)]` proc macro |

**Total**: ~25,500 lines, 415 tests, 130+ source files.

### Frontend Repos

| Repo | Tech | Pages | Status |
|------|------|-------|--------|
| `ingjoo-js` (`@ingjoo/web`) | React + TypeScript + Rollup | — | ✅ Shared component library, auth/i18n/fetch/Domain DSL |
| `web-base` | Next.js 16 + Tailwind CSS 4 | 12 | ✅ Full frontend-backend integration |

---

## Phase 1: Make It Run ✅ COMPLETE

> Goal: End-to-end HTTP request flow that exercises all existing components.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T1.1 | Build axum router skeleton in `ingjoo-bin` | **P0** | ✅ | `main.rs` with working HTTP server, CORS, tracing |
| T1.2 | JWT axum extractor + Auth middleware | **P0** | ✅ | `middleware/auth.rs` — Bearer token extraction, validation, `CurrentUser` injection |
| T1.3 | Add `role` + `groups` fields to `TokenClaims` | **P0** | ✅ | JWT tokens contain `sub` + `role` + `groups` + `exp` + `iat` |
| T1.4 | Wire `SecurityPolicy` into CRUD handlers | **P0** | ✅ | Loads policy from DB per-request; all 5 CRUD handlers wired |
| T1.5 | User CRUD handlers (register/login/profile) | **P1** | ✅ | `/api/auth/register`, `/api/auth/login`, `/api/auth/profile` |
| T1.6 | Settings CRUD handlers | **P1** | ✅ | `/api/settings/*` endpoints |
| T1.7 | Integration test: HTTP → Auth → Security → DB → Response | **P1** | ✅ | 7 integration tests (register/login/refresh/settings etc.) |

**Exit criteria**: ✅ `cargo run` starts server, `curl` can register/login/get profile with JWT auth and permission checks.

---

## Phase 2: Make It Reliable ✅ COMPLETE

> Goal: Prevent regressions, fix error handling, formalize migrations.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T2.1 | GitHub Actions CI: `test` + `clippy` + `fmt --check` | **P0** | ✅ | `.github/workflows/ci.yml` |
| T2.2 | `ingjoo-infra` test coverage | **P0** | ✅ | 17 unit + 23 DB + 7 router + 13 GenericDB tests |
| T2.3 | Unified typed error hierarchy at Store trait boundary | **P1** | ✅ | `StoreError` enum (Database / Config / Io / NotFound / Conflict) |
| T2.4 | Versioned migration system | **P1** | ✅ | v1-v4 migrations, multi-statement DDL support, auto-skip existing tables |
| T2.5 | Justfile for dev commands | **P1** | ✅ | `just test`, `just dev`, `just migrate`, `just lint` |
| T2.6 | `.env.example` with all configurable vars | **P2** | ✅ | `DATABASE_URL`, `CORS_ORIGIN`, JWT secrets, etc. |
| T2.7 | `rustfmt.toml` configuration | **P2** | ✅ | Consistent formatting rules |

**Exit criteria**: ✅ CI green on every push, `ingjoo-infra` has 60 tests, errors are matchable.

---

## Phase 3: Make It Real ✅ COMPLETE

> Goal: Implement the framework's headline features — dynamic model registration, queue, scheduled tasks.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T3.1 | **Dynamic model registry** | **P0** | ✅ | `ModelRegistry` + `GenericDb` — runtime schema definition, auto-DDL, CRUD |
| T3.2 | **`ingjoo-macros` derive macros** | **P1** | ✅ | `#[derive(IngjooModel)]` — generates `fn descriptor()` from struct + `#[ingjoo]` attrs |
| T3.3 | **Permission persistence** | **P1** | ✅ | DB tables for `model_accesses` + `record_rules` + CRUD API + DB wiring |
| T3.4 | **Queue management system** | **P1** | ✅ | `ingjoo-queue` crate — in-memory + SQL backends, WorkerPool, retry policy |
| T3.5 | **Scheduled tasks (cron)** | **P1** | ✅ | `Scheduler` + `ScheduleStore` + `scheduled_jobs` table + admin CRUD API |
| T3.6 | **Relationship field abstractions** | **P2** | ✅ | `Many2one` field type, `RelationConfig`, FK constraint in DDL generation |
| T3.7 | **Audit log fields** | **P2** | ✅ | Auto-populated `create_uid`/`write_uid`/`create_date`/`write_date` |
| T3.8 | **WebSocket support** | **P2** | ✅ | Broadcast channel + WS handler, `/ws` public endpoint |

**Exit criteria**: ✅ Models definable at runtime, background jobs enqueued, permissions via API, scheduled tasks, relationship fields, WebSocket notifications, derive macros.

### T3.4 Queue Management — COMPLETE

```
ingjoo-queue
├── src/
│   ├── lib.rs           # Queue trait, JobStatus, QueuedJob
│   ├── memory.rs        # InMemoryQueue — VecDeque backend (testing)
│   ├── sql.rs           # SqlQueue — atomic dequeue (UPDATE WHERE status=pending)
│   ├── worker.rs        # WorkerPool + JobHandler trait, Semaphore concurrency
│   └── retry.rs         # RetryPolicy — exponential backoff + ±20% jitter
├── tests/
│   ├── memory_test.rs   # 14 tests
│   └── sql_test.rs      # 9 tests
└── migration v4         # queue_jobs table + 2 indexes
```

Design decisions (actual implementation):
- **Dual backend**: `InMemoryQueue` (tests) + `SqlQueue` (production), shared `Queue` trait
- **Persistence**: `queue_jobs` table, survives restarts, atomic dequeue
- **Priority**: Numeric (lower = higher), FIFO within same priority
- **Retry**: Exponential backoff + max attempts + dead-letter queue
- **Concurrency**: Tokio semaphore-based worker pool, broadcast graceful shutdown

### T3.5 Scheduled Tasks — COMPLETE

```
ingjoo-queue/src/
├── scheduler.rs         # ScheduledJob, ScheduleStore trait, SqlScheduleStore, Scheduler<Q>
└── (existing queue files)

ingjoo-infra/src/
├── db/migration.rs      # v5: scheduled_jobs table + indexes
└── handlers/schedule.rs # CRUD endpoints: /api/schedules, /api/schedules/{id}
```

Design decisions:
- **`cron` crate** for cron expression parsing (7-field format: sec min hour day month weekday year)
- **`ScheduleStore` trait** with `SqlScheduleStore` implementation (same Pool+Dialect pattern as SqlQueue)
- **Adaptive sleep**: scheduler calculates sleep duration from next fire time
- **Queue integration**: `Scheduler<Q: Queue>` fires jobs into the queue system
- **Admin CRUD API**: 5 endpoints with admin-only access

### T3.2 Derive Macros — COMPLETE

```
ingjoo-macros/src/
└── lib.rs              # #[derive(IngjooModel)] proc macro

ingjoo-infra/tests/
└── derive_test.rs      # 4 tests: basic fields, audit+many2one, json+timestamp, manual comparison
```

Attribute syntax:
- Struct: `#[ingjoo(table = "name", audit)]`
- Field: `#[ingjoo(type = "text|integer|float|boolean|timestamp|json|many2one", required, unique, related = "model")]`

### T3.6 Relationship Fields — COMPLETE

- `FieldType::Many2one` + `RelationConfig { related_model, foreign_key, through }`
- DDL generation: FK constraint via `Dialect::reference()` (`REFERENCES table(id) ON DELETE SET NULL`)

### T3.8 WebSocket — COMPLETE

- `AppState.events: broadcast::Sender<String>` broadcast channel (capacity 256)
- `handlers/ws.rs`: WS upgrade + split sink/stream + broadcast receive
- Route: `/ws` public endpoint

---

## Phase 4: Make It Extensible (ongoing)

> Goal: Enable ecosystem growth — plugins, documentation, performance.

| # | Task | Priority | Depends On | Status |
|---|------|----------|------------|--------|
| T4.0 | **Menu + View + Action metadata system** | **P0** | T3.1 | ✅ |
| T4.1 | Plugin/module hot-loading system | P1 | T3.1 | ✅ |
| T4.2 | Public API rustdoc (`///` on all public items) | P1 | Phase 2 | ✅ |
| T4.3 | Performance benchmarks (criterion) | P2 | Phase 2 | ✅ |
| T4.4 | Full integration test suite | P2 | Phase 2 | ✅ |
| T4.5 | `ingjoo-cli` management tool | P2 | Phase 2 | ✅ |
| T4.6 | Multi-tenant collection isolation tests | P2 | T3.3 | ✅ |
| T4.7 | Database connection pooling observability | P3 | — | ✅ |
| T4.8 | Rate limiting middleware | P3 | Phase 1 | ✅ |

### T4.0 Metadata System — COMPLETE

```
ingjoo-core/src/module/
└── metadata.rs          # ViewType, ActionType, MenuDescriptor, ViewDescriptor, ActionDescriptor

ingjoo-infra/src/
├── db/
│   ├── migration.rs     # v6: ir_menu + ir_view + ir_action 三表 + 索引
│   └── seed.rs          # 幂等种子数据 (demo action + views + menu)
└── handlers/
    ├── menu.rs          # GET/POST/PUT/DELETE /api/menus — 树构建 + 分组可见性过滤
    ├── view.rs          # GET/POST/PUT/DELETE /api/views — 按 model/type 查询
    └── action.rs        # GET/POST/PUT/DELETE /api/actions — 复合响应 (action+views+model)
```

Design decisions:
- **JSON arch format** (not XML like Odoo) — native for JSON API consumers
- **Single-table actions** with `type` discriminator (not PostgreSQL INHERITS)
- **Named slots** for view inheritance (v2 reserved, v1 no inheritance)
- **Composite `GET /api/actions/{id}`** — returns action + views + model schema in one request (2-round-trip frontend flow)
- **Menu visibility** via JWT `groups` intersection (zero extra DB queries)
- **`page_limit` column** to avoid SQLite reserved word `limit`

---

## Phase 5: Extension Activation ✅ COMPLETE

> Goal: Activate existing extension components, add security headers, complete noop coverage, integrate cache.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T5.1 | Security headers middleware activation | **P0** | ✅ | `security_headers_middleware` wired into router — X-Content-Type-Options, X-Frame-Options, CSP, etc. |
| T5.2 | Noop implementations for all 5 remaining traits | **P0** | ✅ | NoopStateMachine, NoopSearchEngine, NoopPaymentProvider, NoopRelationLoader, NoopTranslationStore |
| T5.3 | Feature flags for extension implementations | **P0** | ✅ | `content-filter`, `data-mask`, `vector-pg`, `signature` features in Cargo.toml + aes-gcm dep |
| T5.4 | FrameworkCache integration into AppState | **P1** | ✅ | `cache: Arc<FrameworkCache<SecurityPolicy, ()>>` in AppState, SecurityPolicy caching in load_security_policy(), cache invalidation on permission mutations |
| T5.5 | ROADMAP update | **P2** | ✅ | Both en/zh ROADMAPs updated with Phase 5 |

**Exit criteria**: ✅ All extension traits have noop fallbacks, security headers active on every response, SecurityPolicy loading cached, feature-gated implementations compilable.

---

## Phase 6: Production Readiness ✅ COMPLETE

> Goal: Wire all 14 extension traits into AppState, feature-gated conditional compilation in main.rs, audit logging in CRUD handlers, cache integration tests, all-features compilation verification.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T6.1 | AppState extension trait wiring | **P0** | ✅ | `state.rs` — 14 `Arc<dyn Trait>` fields + noop defaults + 14 `with_xxx()` builder methods |
| T6.2 | main.rs conditional compilation | **P0** | ✅ | `ingjoo-bin/Cargo.toml` feature forwarding + `build_audit()`/`build_content_filter()`/`build_data_mask()`/`build_signature()` functions |
| T6.3 | CRUD handler audit logging | **P0** | ✅ | Audit log write after successful create/update/delete via `state.audit.create_audit_log()` (fire-and-forget) |
| T6.4 | Cache integration tests | **P1** | ✅ | 3 tests: put+get hit, different key isolation, invalidate clears all |
| T6.5 | `--all-features` compilation fix | **P0** | ✅ | Re-export `GroupId`, `Group`, `GroupImplied`, `ModelAccessRow`, `RecordRuleRow`, `GroupStore`, `AccessStore` from infra's `ids.rs`/`models.rs`/`traits.rs` |
| T6.6 | ROADMAP + progress report update | **P2** | ✅ | Both en/zh ROADMAPs + progress report |

**Exit criteria**: ✅ 14 extension traits wired into AppState, main.rs selects implementations by feature, CRUD audit logging active, cache tests passing, `cargo check --workspace --all-features` zero errors.

---

## Phase 7: Extension Implementation ✅ COMPLETE

> Goal: Implement real DB-backed extension traits to replace noop stubs — EventBus, IdGenerator, Lock, RelationLoader, TranslationStore, StateMachine, CharTextSplitter. Wire into state.rs and main.rs.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T7.1 | EventBus (in-process broadcast) | **P0** | ✅ | `event_bus_impl.rs` — tokio broadcast channel, async subscriber dispatch |
| T7.2 | IdGenerator (UUID v4) | **P0** | ✅ | `id_generator_impl.rs` — uuid::Uuid::new_v4() |
| T7.3 | Lock (DB advisory lock) | **P0** | ✅ | `lock_impl.rs` — SQLite-based mutex with expiration |
| T7.4 | DbRelationLoader | **P1** | ✅ | `relation_loader_impl.rs` — batch load Many2one relations with display_name |
| T7.5 | DbTranslationStore | **P1** | ✅ | `translation.rs` — ir_translation table, set/get/batch/remove/list_languages |
| T7.6 | DbStateMachine | **P1** | ✅ | `state_machine_impl.rs` — ir_state_machine/transition/record tables, register/transition/query |
| T7.7 | CharTextSplitter | **P1** | ✅ | `text_splitter_impl.rs` — character-count chunking with overlap |
| T7.8 | Migrations v8/v9 | **P0** | ✅ | DDL for ir_translation, ir_state_machine, ir_state_transition, ir_state_record |
| T7.9 | state.rs wiring + main.rs | **P0** | ✅ | Real defaults for all implemented traits, feature-gated in main.rs |

**Exit criteria**: ✅ 6 extension traits have DB-backed implementations, CharTextSplitter implemented, 370 tests passing, 0 clippy warnings.

### Phase 7.5: Additional Extension Implementations ✅ COMPLETE

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T7.5.1 | HtmlInputSanitizer | **P1** | ✅ | `sanitizer.rs` — tag whitelist, content suppression for blocked tags, attribute stripping |
| T7.5.2 | FsDocumentLoader | **P1** | ✅ | `document_loader.rs` — recursive file scanning, 10MB limit, metadata extraction |
| T7.5.3 | state.rs real defaults | **P0** | ✅ | sanitizer/document_loader/text_splitter use real impls instead of noop |
| T7.5.4 | Integration tests | **P0** | ✅ | 7 translation + 7 state_machine tests using tempfile-based SQLite |

---

## Additional Completed Items (not in original ROADMAP)

| Task | Description |
|------|-------------|
| Postgres compatibility | `Dialect` supports SQLite + Postgres dual backend (placeholder, time functions, auto-increment PK, DDL splitting) |
| Group permission system | `groups` + `group_implied` + `user_groups` tables, full group management CRUD API |
| Record-level permission filtering | CRUD handlers load `SecurityPolicy` from DB per-request with model_access + record_rule |

---

## Priority Matrix (Next Steps)

```
PHASE 7.5 COMPLETE (370 tests, 0 failures, 0 clippy warnings)
├── T7.5.1 HtmlInputSanitizer — ✅ tag whitelist + content suppression
├── T7.5.2 FsDocumentLoader — ✅ recursive file scanning, 10MB limit
├── T7.5.3 state.rs real defaults — ✅ sanitizer/document_loader/text_splitter
└── T7.5.4 Integration tests — ✅ 7 translation + 7 state_machine tests

PHASE 9 COMPLETE (272 tests, 0 clippy warnings, login→Dashboard QA verified)
├── T9.1 /register page — ✅ ?register=1 query param
├── T9.2 Auth persistence — ✅ localStorage + Authorization header
├── T9.3 /me frontend — ✅ /auth/profile user state restore
├── T9.4 Notification stubs — ✅ 4 Next.js Route Handlers
├── T9.5 admin/search — ✅ skeleton pages
├── T9.6 New API endpoints — ✅ dashboard stats / users search / audit-log
├── T9.7 Preferences path — ✅ /auth/preferences
└── T9.8 Dead code cleanup — ✅ removed providers.tsx

PHASE 12 COMPLETE (414 tests, frontend SSE auth fixed)
├── T12.1 NotificationBell — ✅ badge + dropdown + mark-read
├── T12.2 SSE auth fix — ✅ fetch-based SSE with Bearer token
├── T12.3 Notifications page — ✅ pagination + filtering + SSE refresh
├── T12.4 Shared timeAgo utility — ✅ utils.ts
└── T12.5 Header notification bell wiring — ✅

PHASE 13 COMPLETE (Integration test fix + Profile + Frontend build fixes + Security)
├── T13.1 Fix handlers_test.rs runtime-in-runtime — ✅ verified not a bug (15 tests pass)
├── T13.2 Profile avatar upload — ✅ multipart handler + serve + frontend UI
├── T13.3 Profile preferences persistence — ✅ already implemented end-to-end
├── T13.4 Docs versioned in ingjoo repo — ✅
├── T13.5 Page count fix (8→12) — ✅
├── T13.6 Frontend build fixes — ✅ NavMenu types + Suspense + UpdatePreferences type
└── T13.7 Avatar security hardening — ✅ path traversal protection + integration tests (4 avatar tests)

PHASE 14 COMPLETE (Infrastructure wiring + Multi-tenant tests + Performance benchmarks)
├── T14.1 Email/SMS wiring — ✅ NoopEmailProvider + NoopSmsProvider + AppState + main.rs
├── T14.2 S3/OSS storage wiring — ✅ build_file_storage() + env-var config + S3 fallback
├── T14.3 Multi-tenant isolation tests — ✅ 3 new integration tests (cross-tenant / 3-layer / create-scoped)
└── T14.4 Performance benchmarks — ✅ policy_bench + domain/registry expanded + Justfile bench

PHASE 8 COMPLETE

PHASE 7 COMPLETE
├── T7.1 EventBus — ✅ tokio broadcast
├── T7.2 IdGenerator — ✅ UUID v4
├── T7.3 Lock — ✅ SQLite advisory lock
├── T7.4 DbRelationLoader — ✅ batch Many2one
├── T7.5 DbTranslationStore — ✅ ir_translation CRUD
├── T7.6 DbStateMachine — ✅ register/transition/query
├── T7.7 CharTextSplitter — ✅ char-count chunking
├── T7.8 Migrations v8/v9 — ✅ DDL
└── T7.9 Wiring — ✅ state.rs + main.rs

PHASE 14 COMPLETE
├── T14.1 Email/SMS wiring — ✅ NoopEmailProvider + NoopSmsProvider + AppState
├── T14.2 S3/OSS file storage — ✅ build_file_storage() + env-var config + S3 fallback
├── T14.3 Multi-tenant isolation tests — ✅ 3 new integration tests
└── T14.4 Performance benchmarks — ✅ policy_bench + domain/registry + Justfile bench

PHASE 15 IN PROGRESS (QA Fixes + Documentation Sync)
├── T15.1 Full QA test — ✅ 12 pages, 10 issues, health score 49→65
├── T15.2 ISSUE-004 Search fix — ✅ POST + JSON body + response unwrap
├── T15.3 ISSUE-002 Settings redirect — ✅ /settings → /admin/settings
├── T15.4 ISSUE-003 Admin users list — ✅ /users/search?q= + type fix
├── T15.5 ISSUE-006 Test user — ✅ admin2 created
├── T15.6 QA report — ✅ .gstack/qa-reports/
├── T15.7 Doc sync — ✅ PROGRESS-PLAN/Roadmap/AGENTS.md
├── T15.8 ISSUE-010 Audit log API — 📋 in progress
├── T15.9 ISSUE-005 Forgot password API — 📋 in progress
└── T15.10 ISSUE-001 WebSocket proxy — 📋 pending
├── T6.1 AppState trait wiring — ✅ 14 fields + noop defaults + builder methods
├── T6.2 Conditional compilation — ✅ feature-gated build_xxx() + ingjoo-bin feature forwarding
├── T6.3 Audit logging — ✅ CRUD success fire-and-forget audit
├── T6.4 Cache tests — ✅ 3 integration tests (put/get/invalidate)
├── T6.5 --all-features — ✅ Re-exports completed, zero errors
└── T6.6 ROADMAP — ✅ Updated

PREVIOUS PHASES (ALL COMPLETE)
├── Phase 1: Make It Run — ✅
├── Phase 2: Make It Reliable — ✅
├── Phase 3: Make It Real — ✅
├── Phase 4: Make It Extensible — ✅
└── Phase 5: Extension Activation — ✅
```

---

## Architecture Status Map

```
                         ┌─────────────┐
                         │  ingjoo-bin │  ✅ Full HTTP server
                         │  (306 loc)  │
                         └──────┬──────┘
                                │
              ┌─────────────────┼─────────────────┐
              │                 │                  │
      ┌───────▼──────┐  ┌──────▼──────┐  ┌───────▼──────┐
      │  middleware   │  │  handlers   │  │   router     │
      │  ✅ Complete  │  │  ✅ Complete │  │  ✅ Complete │
      └───────┬──────┘  └──────┬──────┘  └──────────────┘
              │                │
     ┌────────┼────────────────┼────────────┐
     │        │                │            │
┌────▼───┐ ┌──▼────────┐ ┌────▼─────┐ ┌───▼──────┐
│security│ │   cache   │ │   core   │ │  queue   │
│ ✅     │ │ ✅ wired  │ │ ✅      │ │ ✅       │
│ wired  │ │ to Policy │ │          │ │ (25 tests)│
└────┬───┘ └────┬──────┘ └────┬─────┘ └───┬──────┘
     │          │              │           │
     └──────────┴──────┬───────┴───────────┘
                       │
                ┌──────▼──────┐
                 │  ingjoo-infra│  ✅ Complete (197 tests)
                │  (DB/Auth/  │
                │   Storage)  │
                └─────────────┘
```

Legend:
- ✅ Complete: Code done with tests
- ✅ unused: Code done but not called from business flows

---

## Phase 17: View Pipeline Enhancement ✅ COMPLETE

> Goal: Enhance the Action/View rendering pipeline — structured view arch, kanban wiring, form editing, search view expansion, saved filters.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T17.1 | Structured view arch JSON Schema | **P0** | ✅ Done | ViewArch types (ListArch, FormArch, KanbanArch, SearchArch) defined in metadata.rs |
| T17.2 | Kanban view wiring in ViewRenderer | **P0** | ✅ Done | ViewRenderer supports `activeView === "kanban"` branch, SDK KanbanBoard wired + rollup multi-entry fix |
| T17.3 | Form view inline editing | **P0** | ✅ Done | ViewRenderer form mode with editing state, dirty field detection, PUT submit, FormField component |
| T17.4 | Search view seed data expansion | **P1** | ✅ Done | `seed.rs` adds article/product search views + actions + menus |
| T17.5 | Search favorites (saved filters) | **P2** | ✅ Done | ir_search_favorite table + CRUD API + SearchPanel favorites UI |

---

## Phase 18: Automation Rules Engine 📋 Pending

> Goal: Implement Odoo-style automation rules engine — auto-trigger actions based on conditions (create/update/notify).

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T18.1 | `ir_action_rule` table + migration | **P0** | 📋 | DDL: model, trigger, filter_domain, action_type, action_id, active, sequence |
| T18.2 | `ActionRule` type definition | **P0** | 📋 | `ingjoo-core` ActionRule + ActionRuleStore trait |
| T18.3 | EventBus subscription + rule evaluation | **P0** | 📋 | EventBus consumer: model_event → filter_domain evaluation → action trigger |
| T18.4 | Built-in actions: update field / send notification | **P1** | 📋 | `UpdateField` / `SendNotification` action type implementations |
| T18.5 | CRUD API `/api/action-rules` | **P1** | 📋 | Admin CRUD handler + route registration |
| T18.6 | Batch execution optimization | **P2** | 📋 | Batch rule evaluation + N+1 query avoidance |

---

## Phase 19: Computed Fields + Module Inheritance 📋 Pending

> Goal: Implement computed fields and module inheritance mechanisms.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T19.1 | `compute` / `depends` registration | **P0** | 📋 | ModelRegistry supports `computed_fields` + `depends` declarations |
| T19.2 | Field computation engine | **P0** | 📋 | Dependency change → auto recompute → batch update, cycle detection |
| T19.3 | View inheritance (`inherit_id` merge) | **P0** | 📋 | `ir_view.inherit_id` runtime merge: extend arch, add fields/buttons |
| T19.4 | Model extension (non-inheritance) | **P1** | 📋 | Add fields/methods to existing models via `inherit_id` without altering original table |

---

## Phase 20: Module Dependencies + Install Order 📋 Pending

> Goal: Complete module dependency resolution with correct install/uninstall ordering and conflict detection.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T20.1 | Dependency graph resolver | **P0** | 📋 | DAG topological sort + cycle detection + install/uninstall order |
| T20.2 | Install-time dependency check | **P0** | 📋 | Pre-install check: auto-install missing dependencies |
| T20.3 | Uninstall-time reverse dependency check | **P0** | 📋 | Pre-uninstall check: reject if other modules depend on it |
| T20.4 | Module metadata API enhancement | **P1** | 📋 | `/api/modules` returns dependency graph + status |

---

## Phase 21: TestContext Helper 📋 Pending

> Goal: Provide unified test utility struct to simplify integration test authoring.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T21.1 | `TestContext` struct | **P0** | 📋 | Encapsulates Db + AppState + test user, one-line test environment setup |
| T21.2 | Test fixtures | **P1** | 📋 | Predefined test data factories (create_test_user, create_test_record) |
| T21.3 | Assertion helper macros | **P2** | 📋 | `assert_record_exists!` / `assert_field_equals!` / `assert_domain_matches!` |

---

## Phase 22: Translation Integration + File Management 📋 Pending

> Goal: Integrate translation into CRUD serialization, implement unified file management.

| # | Task | Priority | Status | Deliverable |
|---|------|----------|--------|-------------|
| T22.1 | Translation serialization integration | **P0** | 📋 | GET /api/data/{model} auto-returns translated fields by Accept-Language |
| T22.2 | Translation management UI | **P1** | 📋 | Admin translation page: language switch + field translation editing |
| T22.3 | `ir_attachment` unified file management | **P1** | 📋 | Attachment model + upload/download/association API + frontend component |
| T22.4 | File preview integration | **P2** | 📋 | PDF/image/Office file online preview |
