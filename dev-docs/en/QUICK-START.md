# Quick Start Guide

## Prerequisites

| Requirement | Version | Notes |
|-------------|---------|-------|
| Rust | 1.75+ | `rustup update stable` |
| SQLite | 3.x | Default development database |
| PostgreSQL | 15+ | Optional, for production-like testing |

## Build

```bash
# From workspace root (source/ingjoo/)
cargo build
```

## Test

```bash
# Run all tests (85 tests across 3 crates)
cargo test

# Test a specific crate
cargo test -p ingjoo-core
cargo test -p ingjoo-security
cargo test -p ingjoo-cache

# Note: ingjoo-infra has zero tests currently
```

## Database Setup

### SQLite (default, zero config)

Set the database URL:

```bash
export DATABASE_URL="sqlite:./dev.db"
```

The framework auto-creates tables on startup via `run_migrations()`. No manual schema setup needed.

### PostgreSQL

```bash
export DATABASE_URL="postgres://user:pass@localhost:5432/ingjoo_dev"
```

The Dialect system auto-detects the database type from the URL scheme (`sqlite:` vs `postgres:`).

## Run

```bash
cargo run -p ingjoo-bin
```

**Current behavior**: Connects to the database, runs migrations, binds to port 3000, but does **not** start an HTTP server. The binary is a skeleton — see ROADMAP.md Phase 1 for the plan to build it out.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | Required | Database connection string |
| `PORT` | `3000` | Server bind port |
| `RUST_LOG` | `info` | Tracing level (`debug`, `trace`, `warn`, `error`) |

## Crate Feature Flags

### ingjoo-infra

```toml
# Default: DB + Auth
ingjoo-infra = { path = "crates/ingjoo-infra" }

# All features: DB + Auth + Email + SMS + Captcha + S3 Storage
ingjoo-infra = { path = "crates/ingjoo-infra", features = ["full"] }

# Mock in-memory DB (for testing)
ingjoo-infra = { path = "crates/ingjoo-infra", features = ["mock"] }
```

| Feature | What it enables |
|---------|----------------|
| `db` (default) | `ScaffDb` — SQLite/Postgres implementation |
| `auth` (default) | `JwtAuthProvider` — JWT + Argon2 |
| `email` | SMTP email via `lettre` |
| `sms` | Tencent Cloud SMS |
| `captcha` | Image captcha generation |
| `s3` | AWS S3 file storage |
| `mock` | `MockScaffDb` — in-memory test double |

## Project Structure

```
ingjoo/
├── Cargo.toml              # Workspace root
├── ARCHITECTURE.md          # Full architecture document
├── CODE_STYLE.md            # Coding conventions
├── dev-docs/                # Development documentation
│   ├── en/                  # English (AI-first)
│   └── zh/                  # Chinese
└── crates/
    ├── ingjoo-core/         # Traits, models, DSL, dialect
    ├── ingjoo-infra/        # Concrete implementations (DB, auth, storage)
    ├── ingjoo-security/     # 3-layer RBAC engine
    ├── ingjoo-cache/        # Moka-based caching
    ├── ingjoo-bin/          # HTTP server entry point (skeleton)
    └── ingjoo-macros/       # Derive macros (empty)
```

## What Works Today

| Component | Status | How to use |
|-----------|--------|------------|
| Domain DSL | ✅ Working | `Domain::parse(r#"[["name", "=", "test"]]"#)` → SQL condition |
| Dialect abstraction | ✅ Working | `Dialect::Sqlite` / `Dialect::Postgres` for DDL + placeholder conversion |
| Store traits | ✅ Working | `Arc<dyn ScaffStore>` — 8 async traits for data access |
| Security policy engine | ✅ Working | `SecurityPolicy::check_access(model, role, op)` |
| Cache | ✅ Working | `FrameworkCache::new()` — scope, user, settings caches |
| JWT auth provider | ✅ Working | `JwtAuthProvider::new(config)` — hash/verify/create tokens |
| Mock DB | ✅ Working | `MockScaffDb::new()` — full in-memory implementation |
| HTTP server | ❌ Not built | `ingjoo-bin` has no axum routes |
| Dynamic model registration | ❌ Not built | `module/mod.rs` is 12 lines |
| Derive macros | ❌ Not built | `ingjoo-macros` is empty |

## Development Workflow

```bash
# 1. Make changes to a crate
vim crates/ingjoo-core/src/query/domain.rs

# 2. Run tests for that crate
cargo test -p ingjoo-core

# 3. Check for warnings
cargo clippy -p ingjoo-core -- -D warnings

# 4. Check formatting
cargo fmt --check

# 5. Build everything
cargo build
```

## Common Tasks

### Add a new Store trait method

1. Add the method signature to the individual trait in `ingjoo-core/src/db/traits.rs`
2. Implement it in `ingjoo-infra/src/db/mod.rs` (real DB)
3. Implement it in `ingjoo-infra/src/db/mock.rs` (mock)
4. Add tests

### Use the Domain DSL

```rust
use ingjoo_core::query::domain::Domain;

// Simple filter
let domain = Domain::parse(r#"[["status", "=", "active"]]"#)?;
let condition = domain.apply_to_query("SELECT * FROM users", &Dialect::Sqlite);

// Compound expression
let domain = Domain::parse(r#"["&", ["status", "=", "active"], ["role", "in", ["admin", "manager"]]]"#)?;
```

### Use the Security Policy

```rust
use ingjoo_security::policy::{SecurityPolicy, ModelAccess, RecordRule};

let policy = SecurityPolicy {
    model_accesses: vec![ModelAccess {
        model: "entry".into(),
        role: "viewer".into(),
        read: true, write: false, create: false, delete: false, import: false, export: false,
    }],
    record_rules: vec![],
};

assert!(policy.check_access("entry", "viewer", "read"));
assert!(!policy.check_access("entry", "viewer", "write"));
```
