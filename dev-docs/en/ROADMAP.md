# Improvement Roadmap

> Last updated: 2026-04-24
> Status: v0.1.0 — Phase 1-2 complete, Phase 3 complete

## Current State Summary

| Crate | Lines | Tests | Maturity | Status |
|-------|-------|-------|----------|--------|
| `ingjoo-core` | 2,137 | 68 | **Mature** | Domain DSL, Store traits, Dialect — production-ready |
| `ingjoo-infra` | 5,539 | 60 | **Mature** | Full DB impl, auth, storage, handlers, middleware, router |
| `ingjoo-security` | 478 | 16 | **Mature** | 3-layer RBAC engine — wired to CRUD handlers |
| `ingjoo-cache` | 216 | 5 | **Complete** | Moka-based caching — working |
| `ingjoo-queue` | 1,168 | 25 | **Complete** | In-memory + SQL queue, WorkerPool, retry policy, cron scheduler |
| `ingjoo-bin` | 306 | 7 | **Working** | Full axum router with integration tests passing |
| `ingjoo-macros` | 180 | 4 | **Working** | `#[derive(IngjooModel)]` proc macro for ModelDescriptor generation |

**Total**: ~10,024 lines, 185 tests, 64 source files.

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
| T4.1 | Plugin/module hot-loading system | P1 | T3.1 | ❌ |
| T4.2 | Public API rustdoc (`///` on all public items) | P1 | Phase 2 | ❌ |
| T4.3 | Performance benchmarks (criterion) | P2 | Phase 2 | ❌ |
| T4.4 | Full integration test suite | P2 | Phase 2 | ❌ |
| T4.5 | `ingjoo-cli` management tool | P2 | Phase 2 | ❌ |
| T4.6 | Multi-tenant collection isolation tests | P2 | T3.3 | ❌ |
| T4.7 | Database connection pooling observability | P3 | — | ❌ |
| T4.8 | Rate limiting middleware | P3 | Phase 1 | ❌ |

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

## Additional Completed Items (not in original ROADMAP)

| Task | Description |
|------|-------------|
| Postgres compatibility | `Dialect` supports SQLite + Postgres dual backend (placeholder, time functions, auto-increment PK, DDL splitting) |
| Group permission system | `groups` + `group_implied` + `user_groups` tables, full group management CRUD API |
| Record-level permission filtering | CRUD handlers load `SecurityPolicy` from DB per-request with model_access + record_rule |

---

## Priority Matrix (Next Steps)

```
PHASE 4 IN PROGRESS
├── T4.0 Menu+View+Action metadata — ✅ ir_menu/ir_view/ir_action + handlers + seed data

HIGH IMPACT (Phase 4)
├── T4.1 Plugin hot-loading
├── T4.4 Full integration test suite
└── T4.5 CLI management tool

LOWER PRIORITY
├── T4.2 Rustdoc
├── T4.3 Performance benchmarks
├── T4.6 Multi-tenant isolation tests
├── T4.7 Connection pool observability
└── T4.8 Rate limiting middleware
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
│ ✅     │ │ ✅ unused │ │ ✅      │ │ ✅       │
│ wired  │ │           │ │          │ │ (25 tests)│
└────┬───┘ └────┬──────┘ └────┬─────┘ └───┬──────┘
     │          │              │           │
     └──────────┴──────┬───────┴───────────┘
                       │
                ┌──────▼──────┐
                │  ingjoo-infra│  ✅ Complete (60 tests)
                │  (DB/Auth/  │
                │   Storage)  │
                └─────────────┘
```

Legend:
- ✅ Complete: Code done with tests
- ✅ unused: Code done but not called from business flows
