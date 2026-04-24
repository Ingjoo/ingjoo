//! 版本化迁移系统
//!
//! 通过 `_migration_versions` 表追踪已执行的迁移，
//! 每次启动时自动执行待运行的增量迁移。

use anyhow::Result;
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;

/// 迁移定义
struct Migration {
    version: i64,
    name: &'static str,
    /// SQLite 专用 SQL
    up_sqlite: Option<&'static str>,
    /// 通用 SQL（PostgreSQL 等）
    up_generic: Option<&'static str>,
}

/// 获取所有已知迁移（按版本号排序）
fn all_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            name: "rename_username_to_name",
            up_sqlite: Some("ALTER TABLE users RENAME COLUMN username TO name"),
            up_generic: None,
        },
        Migration {
            version: 2,
            name: "add_storage_type_to_attachments",
            up_sqlite: Some(
                "ALTER TABLE attachments ADD COLUMN storage_type TEXT NOT NULL DEFAULT 'local'",
            ),
            up_generic: Some(
                "ALTER TABLE attachments ADD COLUMN storage_type TEXT NOT NULL DEFAULT 'local'",
            ),
        },
        Migration {
            version: 3,
            name: "add_notification_channels_to_preferences",
            up_sqlite: Some(
                "ALTER TABLE user_preferences ADD COLUMN notification_channels TEXT NOT NULL DEFAULT '{\"email\":true,\"in_app\":true,\"push\":false}'",
            ),
            up_generic: Some(
                "ALTER TABLE user_preferences ADD COLUMN notification_channels TEXT NOT NULL DEFAULT '{\"email\":true,\"in_app\":true,\"push\":false}'",
            ),
        },
        Migration {
            version: 4,
            name: "create_queue_jobs_table",
            up_sqlite: Some(
                r#"CREATE TABLE IF NOT EXISTS queue_jobs (
                    id TEXT PRIMARY KEY,
                    queue TEXT NOT NULL DEFAULT 'default',
                    name TEXT NOT NULL,
                    payload TEXT NOT NULL DEFAULT '{}',
                    priority INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL DEFAULT 'pending',
                    attempts INTEGER NOT NULL DEFAULT 0,
                    max_attempts INTEGER NOT NULL DEFAULT 3,
                    run_at TEXT,
                    started_at TEXT,
                    completed_at TEXT,
                    error TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_queue_jobs_status ON queue_jobs(queue, status);
                CREATE INDEX IF NOT EXISTS idx_queue_jobs_priority ON queue_jobs(priority ASC, created_at ASC)"#,
            ),
            up_generic: Some(
                r#"CREATE TABLE IF NOT EXISTS queue_jobs (
                    id VARCHAR PRIMARY KEY,
                    queue VARCHAR NOT NULL DEFAULT 'default',
                    name VARCHAR NOT NULL,
                    payload TEXT NOT NULL DEFAULT '{}',
                    priority INTEGER NOT NULL DEFAULT 0,
                    status VARCHAR NOT NULL DEFAULT 'pending',
                    attempts INTEGER NOT NULL DEFAULT 0,
                    max_attempts INTEGER NOT NULL DEFAULT 3,
                    run_at TIMESTAMPTZ,
                    started_at TIMESTAMPTZ,
                    completed_at TIMESTAMPTZ,
                    error TEXT,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS idx_queue_jobs_status ON queue_jobs(queue, status);
                CREATE INDEX IF NOT EXISTS idx_queue_jobs_priority ON queue_jobs(priority ASC, created_at ASC)"#,
            ),
        },
        Migration {
            version: 5,
            name: "create_scheduled_jobs_table",
            up_sqlite: Some(
                r#"CREATE TABLE IF NOT EXISTS scheduled_jobs (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    cron_expr TEXT NOT NULL,
                    queue TEXT NOT NULL DEFAULT 'default',
                    job_name TEXT NOT NULL,
                    payload TEXT NOT NULL DEFAULT '{}',
                    status TEXT NOT NULL DEFAULT 'active',
                    max_attempts INTEGER NOT NULL DEFAULT 3,
                    last_fire_time TEXT,
                    next_fire_time TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_status ON scheduled_jobs(status);
                CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_next_fire ON scheduled_jobs(next_fire_time ASC)"#,
            ),
            up_generic: Some(
                r#"CREATE TABLE IF NOT EXISTS scheduled_jobs (
                    id VARCHAR PRIMARY KEY,
                    name VARCHAR NOT NULL,
                    cron_expr VARCHAR NOT NULL,
                    queue VARCHAR NOT NULL DEFAULT 'default',
                    job_name VARCHAR NOT NULL,
                    payload TEXT NOT NULL DEFAULT '{}',
                    status VARCHAR NOT NULL DEFAULT 'active',
                    max_attempts INTEGER NOT NULL DEFAULT 3,
                    last_fire_time TIMESTAMPTZ,
                    next_fire_time TIMESTAMPTZ,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_status ON scheduled_jobs(status);
                CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_next_fire ON scheduled_jobs(next_fire_time ASC)"#,
            ),
        },
        Migration {
            version: 6,
            name: "create_ir_metadata_tables",
            up_sqlite: Some(
                r#"CREATE TABLE IF NOT EXISTS ir_menu (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    parent_id TEXT,
                    sequence INTEGER NOT NULL DEFAULT 10,
                    action_id TEXT,
                    web_icon TEXT,
                    active INTEGER NOT NULL DEFAULT 1,
                    group_ids TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_ir_menu_parent ON ir_menu(parent_id, sequence);
                CREATE INDEX IF NOT EXISTS idx_ir_menu_action ON ir_menu(action_id);

                CREATE TABLE IF NOT EXISTS ir_view (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    model TEXT NOT NULL,
                    type TEXT NOT NULL DEFAULT 'form',
                    priority INTEGER NOT NULL DEFAULT 16,
                    arch TEXT NOT NULL DEFAULT '{}',
                    inherit_id TEXT,
                    active INTEGER NOT NULL DEFAULT 1,
                    group_ids TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE UNIQUE INDEX IF NOT EXISTS idx_ir_view_unique ON ir_view(model, type, priority);
                CREATE INDEX IF NOT EXISTS idx_ir_view_model ON ir_view(model, type);

                CREATE TABLE IF NOT EXISTS ir_action (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    type TEXT NOT NULL DEFAULT 'act_window',
                    res_model TEXT,
                    view_mode TEXT NOT NULL DEFAULT 'list,form',
                    view_ids TEXT NOT NULL DEFAULT '[]',
                    domain TEXT,
                    context TEXT,
                    page_limit INTEGER DEFAULT 80,
                    target TEXT NOT NULL DEFAULT 'current',
                    search_view_id TEXT,
                    url TEXT,
                    help TEXT,
                    group_ids TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_ir_action_model ON ir_action(res_model)"#,
            ),
            up_generic: Some(
                r#"CREATE TABLE IF NOT EXISTS ir_menu (
                    id VARCHAR PRIMARY KEY,
                    name VARCHAR NOT NULL,
                    parent_id VARCHAR,
                    sequence INTEGER NOT NULL DEFAULT 10,
                    action_id VARCHAR,
                    web_icon VARCHAR,
                    active BOOLEAN NOT NULL DEFAULT TRUE,
                    group_ids TEXT NOT NULL DEFAULT '[]',
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS idx_ir_menu_parent ON ir_menu(parent_id, sequence);
                CREATE INDEX IF NOT EXISTS idx_ir_menu_action ON ir_menu(action_id);

                CREATE TABLE IF NOT EXISTS ir_view (
                    id VARCHAR PRIMARY KEY,
                    name VARCHAR NOT NULL,
                    model VARCHAR NOT NULL,
                    type VARCHAR NOT NULL DEFAULT 'form',
                    priority INTEGER NOT NULL DEFAULT 16,
                    arch TEXT NOT NULL DEFAULT '{}',
                    inherit_id VARCHAR,
                    active BOOLEAN NOT NULL DEFAULT TRUE,
                    group_ids TEXT NOT NULL DEFAULT '[]',
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE UNIQUE INDEX IF NOT EXISTS idx_ir_view_unique ON ir_view(model, type, priority);
                CREATE INDEX IF NOT EXISTS idx_ir_view_model ON ir_view(model, type);

                CREATE TABLE IF NOT EXISTS ir_action (
                    id VARCHAR PRIMARY KEY,
                    name VARCHAR NOT NULL,
                    type VARCHAR NOT NULL DEFAULT 'act_window',
                    res_model VARCHAR,
                    view_mode VARCHAR NOT NULL DEFAULT 'list,form',
                    view_ids TEXT NOT NULL DEFAULT '[]',
                    domain TEXT,
                    context TEXT,
                    page_limit INTEGER DEFAULT 80,
                    target VARCHAR NOT NULL DEFAULT 'current',
                    search_view_id VARCHAR,
                    url VARCHAR,
                    help TEXT,
                    group_ids TEXT NOT NULL DEFAULT '[]',
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS idx_ir_action_model ON ir_action(res_model)"#,
            ),
        },
        Migration {
            version: 7,
            name: "create_audit_logs_table",
            up_sqlite: Some(
                r#"CREATE TABLE IF NOT EXISTS audit_logs (
                    id TEXT PRIMARY KEY,
                    user_id TEXT,
                    action TEXT NOT NULL,
                    resource TEXT NOT NULL,
                    resource_id TEXT,
                    detail TEXT,
                    ip TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
                CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs(resource, resource_id);
                CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at)"#,
            ),
            up_generic: Some(
                r#"CREATE TABLE IF NOT EXISTS audit_logs (
                    id VARCHAR PRIMARY KEY,
                    user_id VARCHAR,
                    action VARCHAR NOT NULL,
                    resource VARCHAR NOT NULL,
                    resource_id VARCHAR,
                    detail TEXT,
                    ip VARCHAR,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );
                CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
                CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs(resource, resource_id);
                CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at)"#,
            ),
        },
    ]
}

/// 运行所有待执行的迁移
pub async fn run_pending_migrations(pool: &Pool, dialect: &Dialect) -> Result<()> {
    let migrations = all_migrations();

    // 获取已执行的版本号（表可能刚创建，查询失败说明无已应用迁移）
    let applied: Vec<i64> = sqlx::query_scalar(
        "SELECT version FROM _migration_versions ORDER BY version",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for migration in &migrations {
        if applied.contains(&migration.version) {
            continue;
        }

        let sql = match dialect {
            Dialect::Sqlite => migration.up_sqlite,
            _ => migration.up_generic,
        };

        if let Some(sql) = sql {
            let prepared = dialect.prepare(sql);
            let stmts = Dialect::split_ddl(&prepared);
            for stmt in stmts {
                let result = sqlx::query(stmt)
                    .execute(pool)
                    .await;
                if let Err(ref e) = result {
                    let msg = e.to_string();
                    if msg.contains("duplicate column name") || msg.contains("no such column") || msg.contains("already exists") {
                        tracing::debug!("迁移 v{} 跳过（状态已符合）: {}", migration.version, msg);
                    } else {
                        result?;
                    }
                }
            }
        }

        // 记录已执行
        sqlx::query(
            &dialect.prepare(
                "INSERT INTO _migration_versions (version, name) VALUES (?, ?)",
            ),
        )
        .bind(migration.version)
        .bind(migration.name)
        .execute(pool)
        .await?;

        tracing::info!("已执行迁移 v{}: {}", migration.version, migration.name);
    }

    Ok(())
}
