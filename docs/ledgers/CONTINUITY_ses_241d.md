---
session: ses_241d
updated: 2026-04-24T06:43:06.832Z
---

# Session Summary

## Goal
Create a typed `StoreError` enum and replace `anyhow::Result` with `StoreResult` in all store trait methods across the ingjoo framework, enabling structured error handling (NotFound, UniqueViolation, ForeignKeyViolation, etc.) instead of opaque `anyhow::Error`.

## Constraints & Preferences
- Do NOT remove `anyhow` as a dependency anywhere
- Do NOT change any test file contents (139 tests must pass)
- Do NOT modify `ingjoo-security`, `ingjoo-cache`, or `ingjoo-macros` crates
- Keep `anyhow::Result` ONLY in `ScaffTransaction::run_in_transaction` (closure returns arbitrary errors)
- `cargo build && cargo test && cargo clippy --all-targets -- -D warnings` must all succeed with 0 errors
- Add `AppError::Conflict(String)` variant returning HTTP 409

## Progress
### Done
- [x] Created `ingjoo-core/src/db/error.rs` — `StoreError` enum (NotFound, UniqueViolation, ForeignKeyViolation, Database, Config, Io) with Display, Error, From<std::io::Error>, and From<sqlx::Error> impls
- [x] Updated `ingjoo-core/src/db/mod.rs` — added `pub mod error;`
- [x] Updated `ingjoo-core/src/db/traits.rs` — replaced `use anyhow::Result` with `use super::error::StoreResult`, changed ALL trait method signatures from `Result<T>` to `StoreResult<T>`, kept `anyhow::Result<T>` for `ScaffTransaction::run_in_transaction`
- [x] Updated `ingjoo-core/src/lib.rs` — added `pub use db::error::{StoreError, StoreResult};` re-export
- [x] Partially updated `ingjoo-infra/src/db/mod.rs` — added imports for `StoreResult`/`StoreError`, changed first inherent method (`create_user`) return type to `StoreResult<User>`

### In Progress
- [ ] Updating `ingjoo-infra/src/db/mod.rs` — need to change ALL remaining `Db` inherent method return types from `Result<T>` to `StoreResult<T>` (except `run_migrations` and `ScaffTransaction` impl which keep `anyhow::Result`). The `ScaffTransaction` impl needs explicit `anyhow::Result<T>` in its signature.

### Blocked
- (none)

## Key Decisions
- **Put `From<sqlx::Error> for StoreError` in ingjoo-core**: Despite the task spec saying sqlx is NOT a dependency of ingjoo-core, the actual `ingjoo-core/Cargo.toml` has `sqlx.workspace = true` (non-optional). This avoids needing `.map_err(map_sqlx_err)?` on every sqlx `?` call in Db methods (there are ~25+ such calls). The orphan rule allows it since StoreError is local to ingjoo-core.
- **Mock must enforce uniqueness**: The `test_register_duplicate_email_returns_400` test uses `MockScaffDb`. Removing the pre-check in auth.rs means the mock's `create_user` must check for duplicate emails and return `StoreError::UniqueViolation`.
- **Register handler maps UniqueViolation→BadRequest(400)**: The general `From<StoreError> for AppError` maps UniqueViolation→Conflict(409), but auth.rs `register` explicitly catches UniqueViolation and returns `AppError::BadRequest("邮箱已注册")` to keep the test expecting 400 passing.
- **Replace blanket `From<E: Into<anyhow::Error>> for AppError`**: Must be replaced with specific `From<anyhow::Error>` and `From<StoreError>` impls to avoid conflicting with the new `From<StoreError>` impl.

## Next Steps
1. **Finish `ingjoo-infra/src/db/mod.rs`**: Read full file and rewrite with all `Result<T>` → `StoreResult<T>` for Db inherent methods. Keep `anyhow::Result` for `run_migrations` and `ScaffTransaction` impl. The `ScaffTransaction` impl needs `anyhow::Result<T>` explicitly in the async fn signature and the closure Future type. The `use anyhow::Result;` import stays for these.
2. **Update `ingjoo-infra/src/db/mock.rs`**: Change `use anyhow::Result` → `use ingjoo_core::db::error::StoreResult;`, add email uniqueness check to `create_user` returning `StoreError::UniqueViolation`, update `ScaffTransaction` impl to use explicit `anyhow::Result<T>`
3. **Update `ingjoo-infra/src/middleware/error.rs`**: Add `Conflict(String)` variant, replace blanket `From<E: Into<anyhow::Error>>` with specific `From<anyhow::Error>` and `From<StoreError>` impls, add Conflict to IntoResponse (409)
4. **Update `ingjoo-infra/src/handlers/auth.rs`**: Remove `get_user_by_email` pre-check, add `.map_err(|e| match e { StoreError::UniqueViolation{..} => AppError::BadRequest("邮箱已注册".into()), _ => AppError::from(e) })` after `create_user`
5. **Update `ingjoo-infra/src/db/traits.rs`**: Add `pub use ingjoo_core::db::error::{StoreError, StoreResult};` re-export
6. **Update `ingjoo-infra/src/lib.rs`**: Add StoreError/StoreResult to re-exports
7. **Verify**: `cargo build && cargo test && cargo clippy --all-targets -- -D warnings`

## Critical Context
- **Test that constrains auth.rs changes**: `test_register_duplicate_email_returns_400` in `ingjoo-infra/tests/router_test.rs` expects 400 status. The `register_user` helper registers then second call expects 400. This test uses `MockScaffDb` (mock feature), NOT real SQLite.
- **Db methods pattern**: Most use `.map_err(Into::into)` on fetch_one/fetch_optional/fetch_all results, some use `?` directly on execute/query_scalar. With `From<sqlx::Error> for StoreError` in ingjoo-core, both patterns work seamlessly.
- **ScaffTransaction impl in Db**: Uses `self.pool.begin().await?`, `tx.commit().await?` — sqlx errors auto-convert to anyhow::Error since the function returns `anyhow::Result<T>`.
- **MockScaffDb `create_user` current code**: `users.lock().await.insert(user.id.to_string(), stored)` — no uniqueness check. Must add email uniqueness check.
- **AppError blanket impl conflict**: Current `impl<E: Into<anyhow::Error>> From<E> for AppError` covers ALL error types. Must be replaced with specific impls: `From<anyhow::Error>` → `Internal`, `From<StoreError>` → specific mapping (NotFound→NotFound, UniqueViolation→Conflict, ForeignKeyViolation→BadRequest, others→Internal).

## File Operations
### Read
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/Cargo.toml` — has `sqlx.workspace = true` (non-optional)
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/traits.rs` — 180+ lines, ~50 methods all returning `Result<T>` (now changed to `StoreResult<T>`)
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/Cargo.toml` — `sqlx = { workspace = true, optional = true }`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/mod.rs` — ~700 lines, Db struct with all inherent methods and trait impls
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/mock.rs` — ~400+ lines, MockScaffDb with in-memory HashMaps
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/middleware/error.rs` — 39 lines, AppError enum with blanket From impl
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/auth.rs` — register handler has pre-check `get_user_by_email` before `create_user`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/handlers/settings.rs` — 41 lines, uses `?` on store calls
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/lib.rs` — 43 lines, re-exports with feature gates
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/ids.rs` — `pub use ingjoo_core::db::ids::UserId;`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/models.rs` — re-exports from ingjoo_core
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/traits.rs` — `pub use ingjoo_core::db::traits::{...};`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/db_test.rs` — tests using `ScaffDb` with real SQLite
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/router_test.rs` — HTTP-level tests, has `test_register_duplicate_email_returns_400`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/tests/handlers_test.rs` — uses `MockScaffDb`, has `test_register_duplicate_email_returns_400` expecting 400

### Modified
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/error.rs` — **CREATED**: StoreError enum + StoreResult type alias + Display/Error/From impls
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/mod.rs` — added `pub mod error;`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/db/traits.rs` — replaced import + all `Result<T>` → `StoreResult<T>`, ScaffTransaction uses `anyhow::Result<T>`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-core/src/lib.rs` — added `pub use db::error::{StoreError, StoreResult};`
- `/Users/mom988/data/my_dev/ingjoo_rs/source/ingjoo/crates/ingjoo-infra/src/db/mod.rs` — added StoreResult/StoreError imports, changed `create_user` return type (PARTIALLY DONE)
