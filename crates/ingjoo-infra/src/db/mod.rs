pub mod generic;
pub mod ids;
pub mod migration;
pub mod models;
pub mod seed;
pub mod traits;
#[cfg(feature = "mock")]
pub mod mock;

use anyhow::Result;
use async_trait::async_trait;
use ingjoo_core::db::error::StoreResult;
use ingjoo_core::pool::Pool;
use std::pin::Pin;
use ingjoo_core::PaginatedResult;
use ingjoo_core::Dialect;
use ingjoo_core::db::traits::*;
use ingjoo_core::db::ids::UserId;
use ingjoo_core::db::ids::GroupId;
use ingjoo_core::db::models::UserPublic;
use ingjoo_core::extension::audit::{AuditStore, AuditEntry, AuditQuery};

pub use ingjoo_core::db::models::User;
pub use ingjoo_core::db::models::UserPreferences;
pub use ingjoo_core::db::models::UpdatePreferences;
pub use ingjoo_core::db::models::Attachment;
pub use ingjoo_core::db::models::CreateAttachment;
pub use ingjoo_core::db::models::ModuleSetting;
pub use ingjoo_core::db::models::SetModuleSetting;
pub use ingjoo_core::db::models::Group;
pub use ingjoo_core::db::models::GroupImplied;
pub use ingjoo_core::db::models::UserGroup;
pub use ingjoo_core::db::models::ModelAccessRow;
pub use ingjoo_core::db::models::RecordRuleRow;

pub struct Db {
    pool: Pool,
    dialect: Dialect,
}

impl Db {
    pub fn new(pool: Pool) -> Self {
        Self { pool, dialect: Dialect::Sqlite }
    }

    pub fn with_dialect(pool: Pool, dialect: Dialect) -> Self {
        Self { pool, dialect }
    }

    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    pub fn dialect(&self) -> &Dialect {
        &self.dialect
    }

    fn sql(&self, query: &str) -> String {
        self.dialect.prepare(query)
    }

    pub async fn run_migrations(pool: &Pool, dialect: &Dialect) -> Result<()> {
        let ddl = r#"
            CREATE TABLE IF NOT EXISTS users (
                id              TEXT PRIMARY KEY,
                email           TEXT UNIQUE NOT NULL,
                name            TEXT UNIQUE NOT NULL,
                password_hash   TEXT,
                avatar_url      TEXT,
                bio             TEXT,
                role            TEXT NOT NULL DEFAULT 'user',
                oauth_provider  TEXT,
                oauth_id        TEXT,
                phone           TEXT UNIQUE,
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS refresh_tokens (
                id          TEXT PRIMARY KEY,
                user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                token_hash  TEXT NOT NULL,
                expires_at  TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS password_reset_tokens (
                id          TEXT PRIMARY KEY,
                user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                token       TEXT NOT NULL,
                used        INTEGER NOT NULL DEFAULT 0,
                expires_at  TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS captcha_codes (
                id          TEXT PRIMARY KEY,
                answer      TEXT NOT NULL,
                used        INTEGER NOT NULL DEFAULT 0,
                expires_at  TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS sms_codes (
                id          TEXT PRIMARY KEY,
                phone       TEXT NOT NULL,
                code        TEXT NOT NULL,
                purpose     TEXT NOT NULL,
                ip_address  TEXT,
                used        INTEGER NOT NULL DEFAULT 0,
                expires_at  TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS settings (
                key    TEXT PRIMARY KEY,
                value  TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_sms_codes_phone ON sms_codes(phone);
            CREATE INDEX IF NOT EXISTS idx_sms_codes_ip ON sms_codes(ip_address);
            CREATE INDEX IF NOT EXISTS idx_captcha_codes_id ON captcha_codes(id);

            CREATE TABLE IF NOT EXISTS user_preferences (
                user_id        TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
                theme          TEXT NOT NULL DEFAULT 'system',
                inline_edit    INTEGER NOT NULL DEFAULT 1,
                remember_pos   INTEGER NOT NULL DEFAULT 1,
                line_numbers   INTEGER NOT NULL DEFAULT 0,
                last_collection TEXT,
                last_entry     TEXT,
                language       TEXT NOT NULL DEFAULT 'zh-CN',
                updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS attachments (
                id            TEXT PRIMARY KEY,
                user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                filename      TEXT NOT NULL,
                mime_type     TEXT NOT NULL,
                size          INTEGER NOT NULL,
                storage_path  TEXT NOT NULL,
                entity_type   TEXT,
                entity_id     TEXT,
                created_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_attachments_user ON attachments(user_id);
            CREATE INDEX IF NOT EXISTS idx_attachments_entity ON attachments(entity_type, entity_id);

            CREATE TABLE IF NOT EXISTS module_settings (
                id         TEXT PRIMARY KEY,
                scope      TEXT NOT NULL,
                scope_id   TEXT,
                module     TEXT NOT NULL,
                key        TEXT NOT NULL,
                value      TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(scope, scope_id, module, key)
            );

            CREATE INDEX IF NOT EXISTS idx_module_settings_lookup ON module_settings(scope, scope_id, module);

            CREATE TABLE IF NOT EXISTS _migration_versions (
                version     INTEGER PRIMARY KEY,
                name        TEXT NOT NULL,
                applied_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS groups (
                id           TEXT PRIMARY KEY,
                name         TEXT UNIQUE NOT NULL,
                display_name TEXT,
                comment      TEXT,
                created_at   TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS group_implied (
                group_id         TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
                implied_group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
                PRIMARY KEY (group_id, implied_group_id)
            );

            CREATE TABLE IF NOT EXISTS user_groups (
                user_id  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
                PRIMARY KEY (user_id, group_id)
            );

            CREATE TABLE IF NOT EXISTS model_access (
                id         TEXT PRIMARY KEY,
                group_id   TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
                model      TEXT NOT NULL,
                perm_read  INTEGER NOT NULL DEFAULT 0,
                perm_write INTEGER NOT NULL DEFAULT 0,
                perm_create INTEGER NOT NULL DEFAULT 0,
                perm_delete INTEGER NOT NULL DEFAULT 0,
                perm_import INTEGER NOT NULL DEFAULT 0,
                perm_export INTEGER NOT NULL DEFAULT 0
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_model_access_uniq ON model_access(group_id, model);

            CREATE TABLE IF NOT EXISTS record_rule (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                model       TEXT NOT NULL,
                group_id    TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
                domain      TEXT NOT NULL,
                perm_read   INTEGER NOT NULL DEFAULT 0,
                perm_write  INTEGER NOT NULL DEFAULT 0,
                perm_create INTEGER NOT NULL DEFAULT 0,
                perm_delete INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_record_rule_model ON record_rule(model);
                "#;

        for stmt in Dialect::split_ddl(&dialect.prepare(ddl)) {
            sqlx::query(stmt).execute(pool).await?;
        }

        migration::run_pending_migrations(pool, dialect).await?;
        seed_default_groups(pool, dialect).await?;

        Ok(())
    }
}

async fn seed_default_groups(pool: &Pool, dialect: &Dialect) -> Result<()> {
    let groups = [
        ("admin", "admin", "管理员", "系统管理员，拥有全部权限"),
        ("user", "user", "普通用户", "可读写，不可删除"),
        ("viewer", "viewer", "只读用户", "仅可查看和导出"),
    ];
    for (id, name, display_name, comment) in &groups {
        sqlx::query(
            &dialect.prepare("INSERT INTO groups (id, name, display_name, comment) VALUES (?, ?, ?, ?) ON CONFLICT(id) DO NOTHING")
        ).bind(id).bind(name).bind(display_name).bind(comment)
         .execute(pool).await?;
    }

    let implications = [("admin", "user"), ("user", "viewer")];
    for (gid, implied) in &implications {
        sqlx::query(
            &dialect.prepare("INSERT OR IGNORE INTO group_implied (group_id, implied_group_id) VALUES (?, ?)")
        ).bind(gid).bind(implied)
         .execute(pool).await?;
    }

    let models = ["collection", "entry", "source", "project", "user"];
    seed::seed_model_access(pool, dialect, &models).await?;

    sqlx::query(
        &dialect.prepare("INSERT OR IGNORE INTO record_rule (id, name, model, group_id, domain, perm_read, perm_write, perm_create, perm_delete) VALUES ('rule_viewer_entry', 'viewer: only published entries', 'entry', 'viewer', '[\"status\", \"=\", \"published\"]', 1, 0, 0, 0)")
    ).execute(pool).await?;

    Ok(())
}

impl Db {
    // ==================== Users ====================

    pub async fn create_user(&self, user: &User) -> StoreResult<User> {
        sqlx::query_as::<_, User>(
            &self.sql("INSERT INTO users (id, email, name, password_hash, avatar_url, bio, role, oauth_provider, oauth_id, phone)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *")
        )
        .bind(&user.id).bind(&user.email).bind(&user.name)
        .bind(&user.password_hash).bind(&user.avatar_url).bind(&user.bio)
        .bind(&user.role).bind(&user.oauth_provider).bind(&user.oauth_id).bind(&user.phone)
        .fetch_one(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_user_by_email(&self, email: &str) -> StoreResult<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users WHERE email = ?"))
            .bind(email).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_user_by_id(&self, id: &UserId) -> StoreResult<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users WHERE id = ?"))
            .bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_user_by_oauth(&self, provider: &str, oauth_id: &str) -> StoreResult<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users WHERE oauth_provider = ? AND oauth_id = ?"))
            .bind(provider).bind(oauth_id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn update_user(&self, id: &UserId, name: Option<&str>, avatar_url: Option<&str>, bio: Option<&str>) -> StoreResult<Option<User>> {
        sqlx::query_as::<_, User>(
            &self.sql("UPDATE users SET name=COALESCE(?, name), avatar_url=COALESCE(?, avatar_url), bio=COALESCE(?, bio), updated_at=datetime('now') WHERE id=? RETURNING *")
        )
        .bind(name).bind(avatar_url).bind(bio)
        .bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn update_user_role(&self, id: &UserId, role: &str) -> StoreResult<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("UPDATE users SET role=?, updated_at=datetime('now') WHERE id=? RETURNING *"))
            .bind(role).bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn update_user_password(&self, id: &UserId, password_hash: &str) -> StoreResult<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("UPDATE users SET password_hash=?, updated_at=datetime('now') WHERE id=? RETURNING *"))
            .bind(password_hash).bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_user_by_phone(&self, phone: &str) -> StoreResult<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users WHERE phone = ?"))
            .bind(phone).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn list_users(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<User>> {
        let total: i64 = sqlx::query_scalar(&self.sql("SELECT COUNT(*) FROM users"))
            .fetch_one(&self.pool).await?;
        let items = sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users ORDER BY created_at DESC LIMIT ? OFFSET ?"))
            .bind(limit).bind(offset).fetch_all(&self.pool).await?;
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    pub async fn search_users(&self, query: &str, limit: i64) -> StoreResult<Vec<UserPublic>> {
        let pattern = format!("%{}%", query);
        let rows = sqlx::query_as::<_, UserPublic>(
            &self.sql("SELECT id, email, name, avatar_url, bio, role, phone FROM users WHERE name LIKE ?1 OR email LIKE ?1 LIMIT ?2")
        )
        .bind(&pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ==================== Refresh Tokens ====================

    pub async fn create_refresh_token(&self, id: &str, user_id: &UserId, token_hash: &str, expires_at: &str) -> StoreResult<()> {
        sqlx::query(&self.sql("INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at) VALUES (?, ?, ?, ?)"))
            .bind(id).bind(user_id).bind(token_hash).bind(expires_at).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_refresh_token(&self, token_hash: &str) -> StoreResult<Option<(String, String)>> {
        sqlx::query_as(&self.sql("SELECT user_id, expires_at FROM refresh_tokens WHERE token_hash = ?"))
            .bind(token_hash).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn delete_refresh_token(&self, token_hash: &str) -> StoreResult<()> {
        sqlx::query(&self.sql("DELETE FROM refresh_tokens WHERE token_hash = ?")).bind(token_hash).execute(&self.pool).await?;
        Ok(())
    }

    // ==================== Password Reset ====================

    pub async fn create_password_reset_token(&self, id: &str, user_id: &UserId, token: &str, expires_at: &str) -> StoreResult<()> {
        sqlx::query(&self.sql("INSERT INTO password_reset_tokens (id, user_id, token, expires_at) VALUES (?, ?, ?, ?)"))
            .bind(id).bind(user_id).bind(token).bind(expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_password_reset_token(&self, token: &str) -> StoreResult<Option<(String, String, i64, String)>> {
        sqlx::query_as(&self.sql("SELECT user_id, expires_at, used, id FROM password_reset_tokens WHERE token = ?"))
            .bind(token).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn mark_password_reset_used(&self, id: &str) -> StoreResult<()> {
        sqlx::query(&self.sql("UPDATE password_reset_tokens SET used = 1 WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }

    // ==================== Captcha ====================

    pub async fn create_captcha(&self, id: &str, answer: &str, expires_at: &str) -> StoreResult<()> {
        sqlx::query(&self.sql("INSERT INTO captcha_codes (id, answer, expires_at) VALUES (?, ?, ?)"))
            .bind(id).bind(answer).bind(expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_captcha(&self, id: &str) -> StoreResult<Option<(String, i64, String)>> {
        sqlx::query_as(&self.sql("SELECT answer, used, expires_at FROM captcha_codes WHERE id = ?"))
            .bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn mark_captcha_used(&self, id: &str) -> StoreResult<()> {
        sqlx::query(&self.sql("UPDATE captcha_codes SET used = 1 WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }

    // ==================== SMS ====================

    pub async fn create_sms_code(&self, id: &str, phone: &str, code: &str, purpose: &str, ip_address: Option<&str>, expires_at: &str) -> StoreResult<()> {
        sqlx::query(&self.sql("INSERT INTO sms_codes (id, phone, code, purpose, ip_address, expires_at) VALUES (?, ?, ?, ?, ?, ?)"))
            .bind(id).bind(phone).bind(code).bind(purpose).bind(ip_address).bind(expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_latest_sms_code(&self, phone: &str, purpose: &str) -> StoreResult<Option<(String, String, i64, String)>> {
        sqlx::query_as(&self.sql("SELECT code, expires_at, used, id FROM sms_codes WHERE phone = ? AND purpose = ? ORDER BY created_at DESC LIMIT 1"))
            .bind(phone).bind(purpose).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_sms_code_sent_within(&self, phone: &str, purpose: &str, seconds: i64) -> StoreResult<bool> {
        let sql = self.dialect.format_sql(&format!("SELECT COUNT(*) FROM sms_codes WHERE phone = ? AND purpose = ? AND created_at > {}", self.dialect.now_offset_bind("seconds")));
        let (count,): (i64,) = sqlx::query_as(&sql)
            .bind(phone).bind(purpose).bind(format!("-{}", seconds))
            .fetch_one(&self.pool).await?;
        Ok(count > 0)
    }

    pub async fn mark_sms_code_used(&self, id: &str) -> StoreResult<()> {
        sqlx::query(&self.sql("UPDATE sms_codes SET used = 1 WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn count_sms_by_ip(&self, ip: &str, hours: i64) -> StoreResult<i64> {
        let sql = self.dialect.format_sql(&format!("SELECT COUNT(*) FROM sms_codes WHERE ip_address = ? AND created_at > {}", self.dialect.now_offset_bind("hours")));
        let (count,): (i64,) = sqlx::query_as(&sql)
            .bind(ip).bind(format!("-{}", hours))
            .fetch_one(&self.pool).await?;
        Ok(count)
    }

    pub async fn count_sms_by_phone_today(&self, phone: &str) -> StoreResult<i64> {
        let sql = self.dialect.format_sql(&format!("SELECT COUNT(*) FROM sms_codes WHERE phone = ? AND created_at > {}", self.dialect.now_offset_negative("24 hours")));
        let (count,): (i64,) = sqlx::query_as(&sql)
            .bind(phone).fetch_one(&self.pool).await?;
        Ok(count)
    }

    // ==================== Settings ====================

    pub async fn get_setting(&self, key: &str) -> StoreResult<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(&self.sql("SELECT value FROM settings WHERE key = ?"))
            .bind(key).fetch_optional(&self.pool).await?;
        Ok(row.map(|(v,)| v))
    }

    pub async fn get_all_settings(&self) -> StoreResult<std::collections::HashMap<String, String>> {
        let rows: Vec<(String, String)> = sqlx::query_as(&self.sql("SELECT key, value FROM settings ORDER BY key"))
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> StoreResult<()> {
        sqlx::query(
            &self.sql("INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')")
        ).bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn set_settings(&self, pairs: &[(String, String)]) -> StoreResult<()> {
        let mut tx = self.pool.begin().await?;
        for (k, v) in pairs {
            sqlx::query(
                &self.sql("INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')")
            ).bind(k).bind(v).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn seed_settings(&self, pairs: &[(String, String)]) -> StoreResult<()> {
        for (key, value) in pairs {
            sqlx::query(&self.sql("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO NOTHING"))
                .bind(key).bind(value).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn get_user_preferences(&self, user_id: &UserId) -> StoreResult<UserPreferences> {
        let r = sqlx::query_as::<_, UserPreferences>(
            &self.sql("SELECT * FROM user_preferences WHERE user_id = ?")
        ).bind(user_id).fetch_optional(&self.pool).await?;
        match r {
            Some(p) => Ok(p),
            None => {
                let def = UserPreferences {
                    user_id: UserId(user_id.to_string()),
                    theme: "system".to_string(),
                    inline_edit: 1,
                    remember_pos: 1,
                    line_numbers: 0,
                    last_collection: None,
                    last_entry: None,
                    language: "zh-CN".to_string(),
                    notification_channels: Some("{\"email\":true,\"in_app\":true,\"push\":false}".to_string()),
                    updated_at: String::new(),
                };
                sqlx::query(
                    &self.sql("INSERT INTO user_preferences (user_id, theme, inline_edit, remember_pos, line_numbers, language) VALUES (?, 'system', 1, 1, 0, 'zh-CN')")
                ).bind(user_id).execute(&self.pool).await?;
                Ok(def)
            }
        }
    }

    pub async fn upsert_user_preferences(&self, user_id: &UserId, input: &UpdatePreferences) -> StoreResult<UserPreferences> {
        let current = self.get_user_preferences(user_id).await?;
        let theme = input.theme.as_ref().unwrap_or(&current.theme);
        let inline_edit = match input.inline_edit { Some(v) => if v { 1i64 } else { 0i64 }, None => current.inline_edit };
        let remember_pos = match input.remember_pos { Some(v) => if v { 1i64 } else { 0i64 }, None => current.remember_pos };
        let line_numbers = match input.line_numbers { Some(v) => if v { 1i64 } else { 0i64 }, None => current.line_numbers };
        let last_collection = input.last_collection.as_ref().or(current.last_collection.as_ref());
        let last_entry = input.last_entry.as_ref().or(current.last_entry.as_ref());
        let language = input.language.as_ref().unwrap_or(&current.language);
        let notification_channels = input.notification_channels.as_ref().or(current.notification_channels.as_ref());
        sqlx::query_as::<_, UserPreferences>(
            &self.sql("INSERT INTO user_preferences (user_id, theme, inline_edit, remember_pos, line_numbers, last_collection, last_entry, language, notification_channels, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
             ON CONFLICT(user_id) DO UPDATE SET theme=excluded.theme, inline_edit=excluded.inline_edit, remember_pos=excluded.remember_pos,
             line_numbers=excluded.line_numbers, last_collection=excluded.last_collection, last_entry=excluded.last_entry, language=excluded.language, notification_channels=excluded.notification_channels, updated_at=datetime('now')
             RETURNING *")
        )
        .bind(user_id).bind(theme).bind(inline_edit).bind(remember_pos).bind(line_numbers)
        .bind(last_collection).bind(last_entry).bind(language).bind(notification_channels)
        .fetch_one(&self.pool).await.map_err(Into::into)
    }

    // ==================== Attachments ====================

    pub async fn create_attachment(&self, id: &str, user_id: &UserId, input: &CreateAttachment, storage_path: &str) -> StoreResult<Attachment> {
        sqlx::query_as::<_, Attachment>(
            &self.sql("INSERT INTO attachments (id, user_id, filename, mime_type, size, storage_path, entity_type, entity_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *")
        )
        .bind(id).bind(user_id).bind(&input.filename).bind(&input.mime_type)
        .bind(input.size).bind(storage_path)
        .bind(&input.entity_type).bind(&input.entity_id)
        .fetch_one(&self.pool).await.map_err(Into::into)
    }

    pub async fn list_attachments(&self, entity_type: Option<&str>, entity_id: Option<&str>, limit: i64, offset: i64) -> StoreResult<PaginatedResult<Attachment>> {
        let (items, total) = match (entity_type, entity_id) {
            (Some(et), Some(eid)) => {
                let total: i64 = sqlx::query_scalar(&self.sql("SELECT COUNT(*) FROM attachments WHERE entity_type = ? AND entity_id = ?"))
                    .bind(et).bind(eid).fetch_one(&self.pool).await?;
                let items = sqlx::query_as::<_, Attachment>(
                    &self.sql("SELECT * FROM attachments WHERE entity_type = ? AND entity_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?")
                ).bind(et).bind(eid).bind(limit).bind(offset)
                 .fetch_all(&self.pool).await?;
                (items, total)
            }
            _ => {
                let total: i64 = sqlx::query_scalar(&self.sql("SELECT COUNT(*) FROM attachments"))
                    .fetch_one(&self.pool).await?;
                let items = sqlx::query_as::<_, Attachment>(
                    &self.sql("SELECT * FROM attachments ORDER BY created_at DESC LIMIT ? OFFSET ?")
                ).bind(limit).bind(offset)
                 .fetch_all(&self.pool).await?;
                (items, total)
            }
        };
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    pub async fn get_attachment(&self, id: &str) -> StoreResult<Option<Attachment>> {
        sqlx::query_as::<_, Attachment>(&self.sql("SELECT * FROM attachments WHERE id = ?"))
            .bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn delete_attachment(&self, id: &str) -> StoreResult<bool> {
        let result = sqlx::query(&self.sql("DELETE FROM attachments WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    // ==================== Module Settings ====================

    pub async fn list_module_settings(&self, scope: &str, scope_id: Option<&str>, module: Option<&str>) -> StoreResult<Vec<ModuleSetting>> {
        match (scope_id, module) {
            (Some(sid), Some(m)) => {
                sqlx::query_as::<_, ModuleSetting>(
                    &self.sql("SELECT * FROM module_settings WHERE scope = ? AND scope_id = ? AND module = ? ORDER BY key")
                ).bind(scope).bind(sid).bind(m).fetch_all(&self.pool).await.map_err(Into::into)
            }
            (Some(sid), None) => {
                sqlx::query_as::<_, ModuleSetting>(
                    &self.sql("SELECT * FROM module_settings WHERE scope = ? AND scope_id = ? ORDER BY module, key")
                ).bind(scope).bind(sid).fetch_all(&self.pool).await.map_err(Into::into)
            }
            (None, Some(m)) => {
                sqlx::query_as::<_, ModuleSetting>(
                    &self.sql("SELECT * FROM module_settings WHERE scope = ? AND module = ? ORDER BY key")
                ).bind(scope).bind(m).fetch_all(&self.pool).await.map_err(Into::into)
            }
            _ => {
                sqlx::query_as::<_, ModuleSetting>(
                    &self.sql("SELECT * FROM module_settings WHERE scope = ? ORDER BY module, key")
                ).bind(scope).fetch_all(&self.pool).await.map_err(Into::into)
            }
        }
    }

    pub async fn get_module_setting(&self, scope: &str, scope_id: Option<&str>, module: &str, key: &str) -> StoreResult<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            &self.sql("SELECT value FROM module_settings WHERE scope = ? AND scope_id IS ? AND module = ? AND key = ?")
        ).bind(scope).bind(scope_id).bind(module).bind(key)
         .fetch_optional(&self.pool).await?;
        Ok(row.map(|(v,)| v))
    }

    pub async fn set_module_setting(&self, id: &str, scope: &str, scope_id: Option<&str>, module: &str, key: &str, value: &str) -> StoreResult<ModuleSetting> {
        sqlx::query_as::<_, ModuleSetting>(
            &self.sql("INSERT INTO module_settings (id, scope, scope_id, module, key, value, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
             ON CONFLICT(scope, scope_id, module, key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')
             RETURNING *")
        ).bind(id).bind(scope).bind(scope_id).bind(module).bind(key).bind(value)
         .fetch_one(&self.pool).await.map_err(Into::into)
    }

    pub async fn delete_module_setting(&self, id: &str) -> StoreResult<bool> {
        let result = sqlx::query(&self.sql("DELETE FROM module_settings WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_effective_setting(&self, module: &str, key: &str, collection_id: Option<&str>) -> StoreResult<Option<String>> {
        if let Some(ws_id) = collection_id {
            let ws_val = self.get_module_setting("document_collection", Some(ws_id), module, key).await?;
            if ws_val.is_some() { return Ok(ws_val); }
        }
        self.get_module_setting("system", None, module, key).await
    }

    // ==================== Storage Migration ====================

    pub async fn list_all_attachments(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<Attachment>> {
        let total: i64 = sqlx::query_scalar(&self.sql("SELECT COUNT(*) FROM attachments"))
            .fetch_one(&self.pool).await?;
        let items = sqlx::query_as::<_, Attachment>(
            &self.sql("SELECT * FROM attachments ORDER BY created_at ASC LIMIT ? OFFSET ?")
        )
        .bind(limit).bind(offset)
        .fetch_all(&self.pool).await?;
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    pub async fn update_attachment_storage(&self, id: &str, storage_path: &str, storage_type: &str) -> StoreResult<bool> {
        let result = sqlx::query(
            &self.sql("UPDATE attachments SET storage_path = ?, storage_type = ? WHERE id = ?")
        )
        .bind(storage_path).bind(storage_type).bind(id)
        .execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }
}

// ==================== Trait Implementations ====================

#[async_trait]
impl UserStore for Db {
    async fn create_user(&self, user: &User) -> StoreResult<User> { Db::create_user(self, user).await }
    async fn get_user_by_email(&self, email: &str) -> StoreResult<Option<User>> { Db::get_user_by_email(self, email).await }
    async fn get_user_by_id(&self, id: &UserId) -> StoreResult<Option<User>> { Db::get_user_by_id(self, id).await }
    async fn get_user_by_oauth(&self, provider: &str, oauth_id: &str) -> StoreResult<Option<User>> { Db::get_user_by_oauth(self, provider, oauth_id).await }
    async fn get_user_by_phone(&self, phone: &str) -> StoreResult<Option<User>> { Db::get_user_by_phone(self, phone).await }
    async fn update_user(&self, id: &UserId, name: Option<&str>, avatar_url: Option<&str>, bio: Option<&str>) -> StoreResult<Option<User>> { Db::update_user(self, id, name, avatar_url, bio).await }
    async fn update_user_role(&self, id: &UserId, role: &str) -> StoreResult<Option<User>> { Db::update_user_role(self, id, role).await }
    async fn update_user_password(&self, id: &UserId, password_hash: &str) -> StoreResult<Option<User>> { Db::update_user_password(self, id, password_hash).await }
    async fn list_users(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<User>> { Db::list_users(self, limit, offset).await }
    async fn search_users(&self, query: &str, limit: i64) -> StoreResult<Vec<UserPublic>> { Db::search_users(self, query, limit).await }
}

#[async_trait]
impl TokenStore for Db {
    async fn create_refresh_token(&self, id: &str, user_id: &UserId, token_hash: &str, expires_at: &str) -> StoreResult<()> { Db::create_refresh_token(self, id, user_id, token_hash, expires_at).await }
    async fn get_refresh_token(&self, token_hash: &str) -> StoreResult<Option<(String, String)>> { Db::get_refresh_token(self, token_hash).await }
    async fn delete_refresh_token(&self, token_hash: &str) -> StoreResult<()> { Db::delete_refresh_token(self, token_hash).await }
    async fn create_password_reset_token(&self, id: &str, user_id: &UserId, token: &str, expires_at: &str) -> StoreResult<()> { Db::create_password_reset_token(self, id, user_id, token, expires_at).await }
    async fn get_password_reset_token(&self, token: &str) -> StoreResult<Option<(String, String, i64, String)>> { Db::get_password_reset_token(self, token).await }
    async fn mark_password_reset_used(&self, id: &str) -> StoreResult<()> { Db::mark_password_reset_used(self, id).await }
}

#[async_trait]
impl CaptchaStore for Db {
    async fn create_captcha(&self, id: &str, answer: &str, expires_at: &str) -> StoreResult<()> { Db::create_captcha(self, id, answer, expires_at).await }
    async fn get_captcha(&self, id: &str) -> StoreResult<Option<(String, i64, String)>> { Db::get_captcha(self, id).await }
    async fn mark_captcha_used(&self, id: &str) -> StoreResult<()> { Db::mark_captcha_used(self, id).await }
}

#[async_trait]
impl SmsCodeStore for Db {
    async fn create_sms_code(&self, id: &str, phone: &str, code: &str, purpose: &str, ip_address: Option<&str>, expires_at: &str) -> StoreResult<()> { Db::create_sms_code(self, id, phone, code, purpose, ip_address, expires_at).await }
    async fn get_latest_sms_code(&self, phone: &str, purpose: &str) -> StoreResult<Option<(String, String, i64, String)>> { Db::get_latest_sms_code(self, phone, purpose).await }
    async fn get_sms_code_sent_within(&self, phone: &str, purpose: &str, seconds: i64) -> StoreResult<bool> { Db::get_sms_code_sent_within(self, phone, purpose, seconds).await }
    async fn mark_sms_code_used(&self, id: &str) -> StoreResult<()> { Db::mark_sms_code_used(self, id).await }
    async fn count_sms_by_ip(&self, ip: &str, hours: i64) -> StoreResult<i64> { Db::count_sms_by_ip(self, ip, hours).await }
    async fn count_sms_by_phone_today(&self, phone: &str) -> StoreResult<i64> { Db::count_sms_by_phone_today(self, phone).await }
}

#[async_trait]
impl SettingsStore for Db {
    async fn get_setting(&self, key: &str) -> StoreResult<Option<String>> { Db::get_setting(self, key).await }
    async fn get_all_settings(&self) -> StoreResult<std::collections::HashMap<String, String>> { Db::get_all_settings(self).await }
    async fn set_setting(&self, key: &str, value: &str) -> StoreResult<()> { Db::set_setting(self, key, value).await }
    async fn set_settings(&self, pairs: &[(String, String)]) -> StoreResult<()> { Db::set_settings(self, pairs).await }
    async fn seed_settings(&self, pairs: &[(String, String)]) -> StoreResult<()> { Db::seed_settings(self, pairs).await }
}

#[async_trait]
impl PreferenceStore for Db {
    async fn get_user_preferences(&self, user_id: &UserId) -> StoreResult<UserPreferences> { Db::get_user_preferences(self, user_id).await }
    async fn upsert_user_preferences(&self, user_id: &UserId, input: &UpdatePreferences) -> StoreResult<UserPreferences> { Db::upsert_user_preferences(self, user_id, input).await }
}

#[async_trait]
impl AttachmentStore for Db {
    async fn create_attachment(&self, id: &str, user_id: &UserId, input: &CreateAttachment, storage_path: &str) -> StoreResult<Attachment> { Db::create_attachment(self, id, user_id, input, storage_path).await }
    async fn list_attachments(&self, entity_type: Option<&str>, entity_id: Option<&str>, limit: i64, offset: i64) -> StoreResult<PaginatedResult<Attachment>> { Db::list_attachments(self, entity_type, entity_id, limit, offset).await }
    async fn get_attachment(&self, id: &str) -> StoreResult<Option<Attachment>> { Db::get_attachment(self, id).await }
    async fn delete_attachment(&self, id: &str) -> StoreResult<bool> { Db::delete_attachment(self, id).await }
    async fn list_all_attachments(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<Attachment>> { Db::list_all_attachments(self, limit, offset).await }
    async fn update_attachment_storage(&self, id: &str, storage_path: &str, storage_type: &str) -> StoreResult<bool> { Db::update_attachment_storage(self, id, storage_path, storage_type).await }
}

#[async_trait]
impl ModuleSettingStore for Db {
    async fn list_module_settings(&self, scope: &str, scope_id: Option<&str>, module: Option<&str>) -> StoreResult<Vec<ModuleSetting>> { Db::list_module_settings(self, scope, scope_id, module).await }
    async fn get_module_setting(&self, scope: &str, scope_id: Option<&str>, module: &str, key: &str) -> StoreResult<Option<String>> { Db::get_module_setting(self, scope, scope_id, module, key).await }
    async fn set_module_setting(&self, id: &str, scope: &str, scope_id: Option<&str>, module: &str, key: &str, value: &str) -> StoreResult<ModuleSetting> { Db::set_module_setting(self, id, scope, scope_id, module, key, value).await }
    async fn delete_module_setting(&self, id: &str) -> StoreResult<bool> { Db::delete_module_setting(self, id).await }
    async fn get_effective_setting(&self, module: &str, key: &str, collection_id: Option<&str>) -> StoreResult<Option<String>> { Db::get_effective_setting(self, module, key, collection_id).await }
}

impl Db {
    // ==================== Groups ====================

    pub async fn create_group(&self, group: &Group) -> StoreResult<Group> {
        sqlx::query_as::<_, Group>(
            &self.sql("INSERT INTO groups (id, name, display_name, comment) VALUES (?, ?, ?, ?) RETURNING *")
        ).bind(&group.id).bind(&group.name).bind(&group.display_name).bind(&group.comment)
         .fetch_one(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_group(&self, id: &GroupId) -> StoreResult<Option<Group>> {
        sqlx::query_as::<_, Group>(&self.sql("SELECT * FROM groups WHERE id = ?"))
            .bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_group_by_name(&self, name: &str) -> StoreResult<Option<Group>> {
        sqlx::query_as::<_, Group>(&self.sql("SELECT * FROM groups WHERE name = ?"))
            .bind(name).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn list_groups(&self) -> StoreResult<Vec<Group>> {
        sqlx::query_as::<_, Group>(&self.sql("SELECT * FROM groups ORDER BY name"))
            .fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub async fn update_group(&self, id: &GroupId, display_name: Option<&str>, comment: Option<&str>) -> StoreResult<Option<Group>> {
        sqlx::query_as::<_, Group>(
            &self.sql("UPDATE groups SET display_name=COALESCE(?, display_name), comment=COALESCE(?, comment), updated_at=datetime('now') WHERE id=? RETURNING *")
        ).bind(display_name).bind(comment).bind(id)
         .fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn delete_group(&self, id: &GroupId) -> StoreResult<bool> {
        let r = sqlx::query(&self.sql("DELETE FROM groups WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn set_implied_groups(&self, group_id: &GroupId, implied_ids: &[GroupId]) -> StoreResult<()> {
        sqlx::query(&self.sql("DELETE FROM group_implied WHERE group_id = ?"))
            .bind(group_id).execute(&self.pool).await?;
        for implied_id in implied_ids {
            sqlx::query(&self.sql("INSERT INTO group_implied (group_id, implied_group_id) VALUES (?, ?)"))
                .bind(group_id).bind(implied_id).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn get_implied_groups(&self, group_id: &GroupId) -> StoreResult<Vec<GroupImplied>> {
        sqlx::query_as::<_, GroupImplied>(&self.sql("SELECT * FROM group_implied WHERE group_id = ?"))
            .bind(group_id).fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub async fn add_user_to_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()> {
        sqlx::query(&self.sql("INSERT INTO user_groups (user_id, group_id) VALUES (?, ?) ON CONFLICT DO NOTHING"))
            .bind(user_id).bind(group_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn remove_user_from_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()> {
        sqlx::query(&self.sql("DELETE FROM user_groups WHERE user_id = ? AND group_id = ?"))
            .bind(user_id).bind(group_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_user_groups(&self, user_id: &UserId) -> StoreResult<Vec<Group>> {
        sqlx::query_as::<_, Group>(
            &self.sql("SELECT g.* FROM groups g JOIN user_groups ug ON g.id = ug.group_id WHERE ug.user_id = ? ORDER BY g.name")
        ).bind(user_id).fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub async fn resolve_all_groups(&self, user_id: &UserId) -> StoreResult<Vec<String>> {
        let direct = self.get_user_groups(user_id).await?;
        let mut result = std::collections::HashSet::new();
        let mut queue: std::collections::VecDeque<GroupId> = std::collections::VecDeque::new();
        for g in &direct {
            result.insert(g.name.clone());
            queue.push_back(g.id.clone());
        }
        while let Some(gid) = queue.pop_front() {
            let implied = self.get_implied_groups(&gid).await?;
            for imp in implied {
                if let Some(grp) = self.get_group(&imp.implied_group_id).await? {
                    if result.insert(grp.name.clone()) {
                        queue.push_back(grp.id.clone());
                    }
                }
            }
        }
        let mut names: Vec<String> = result.into_iter().collect();
        names.sort();
        Ok(names)
    }

    pub async fn set_user_groups(&self, user_id: &UserId, group_ids: &[GroupId]) -> StoreResult<()> {
        sqlx::query(&self.sql("DELETE FROM user_groups WHERE user_id = ?"))
            .bind(user_id).execute(&self.pool).await?;
        for gid in group_ids {
            self.add_user_to_group(user_id, gid).await?;
        }
        Ok(())
    }

    // ==================== Model Access ====================

    fn row_to_model_access(row: &sqlx::any::AnyRow) -> ModelAccessRow {
        use sqlx::Row;
        ModelAccessRow {
            id: row.get("id"),
            group_id: GroupId::new(row.get::<String, _>("group_id")),
            model: row.get("model"),
            perm_read: row.get::<i32, _>("perm_read") != 0,
            perm_write: row.get::<i32, _>("perm_write") != 0,
            perm_create: row.get::<i32, _>("perm_create") != 0,
            perm_delete: row.get::<i32, _>("perm_delete") != 0,
            perm_import: row.get::<i32, _>("perm_import") != 0,
            perm_export: row.get::<i32, _>("perm_export") != 0,
        }
    }

    fn row_to_record_rule(row: &sqlx::any::AnyRow) -> RecordRuleRow {
        use sqlx::Row;
        RecordRuleRow {
            id: row.get("id"),
            name: row.get("name"),
            group_id: GroupId::new(row.get::<String, _>("group_id")),
            model: row.get("model"),
            domain: row.get("domain"),
            perm_read: row.get::<i32, _>("perm_read") != 0,
            perm_write: row.get::<i32, _>("perm_write") != 0,
            perm_create: row.get::<i32, _>("perm_create") != 0,
            perm_delete: row.get::<i32, _>("perm_delete") != 0,
        }
    }

    pub async fn create_model_access(&self, access: &ModelAccessRow) -> StoreResult<ModelAccessRow> {
        sqlx::query(
            &self.sql("INSERT INTO model_access (id, group_id, model, perm_read, perm_write, perm_create, perm_delete, perm_import, perm_export) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        ).bind(&access.id).bind(&access.group_id).bind(&access.model)
         .bind(access.perm_read as i32).bind(access.perm_write as i32).bind(access.perm_create as i32)
         .bind(access.perm_delete as i32).bind(access.perm_import as i32).bind(access.perm_export as i32)
         .execute(&self.pool).await?;
        Ok(access.clone())
    }

    pub async fn get_model_access(&self, id: &str) -> StoreResult<Option<ModelAccessRow>> {
        let row = sqlx::query(&self.sql("SELECT * FROM model_access WHERE id = ?"))
            .bind(id).fetch_optional(&self.pool).await?;
        Ok(row.as_ref().map(|r| Self::row_to_model_access(r)))
    }

    pub async fn list_model_accesses(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<ModelAccessRow>> {
        let rows = match group_id {
            Some(gid) => sqlx::query(&self.sql("SELECT * FROM model_access WHERE group_id = ? ORDER BY model"))
                .bind(gid).fetch_all(&self.pool).await?,
            None => sqlx::query(&self.sql("SELECT * FROM model_access ORDER BY model"))
                .fetch_all(&self.pool).await?,
        };
        Ok(rows.iter().map(|r| Self::row_to_model_access(r)).collect())
    }

    pub async fn update_model_access(&self, id: &str, access: &ModelAccessRow) -> StoreResult<Option<ModelAccessRow>> {
        let result = sqlx::query(
            &self.sql("UPDATE model_access SET group_id=?, model=?, perm_read=?, perm_write=?, perm_create=?, perm_delete=?, perm_import=?, perm_export=? WHERE id=?")
        ).bind(&access.group_id).bind(&access.model)
         .bind(access.perm_read as i32).bind(access.perm_write as i32).bind(access.perm_create as i32)
         .bind(access.perm_delete as i32).bind(access.perm_import as i32).bind(access.perm_export as i32)
         .bind(id).execute(&self.pool).await?;
        if result.rows_affected() == 0 { return Ok(None); }
        Ok(Some(access.clone()))
    }

    pub async fn delete_model_access(&self, id: &str) -> StoreResult<bool> {
        let r = sqlx::query(&self.sql("DELETE FROM model_access WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(r.rows_affected() > 0)
    }

    // ==================== Record Rules ====================

    pub async fn create_record_rule(&self, rule: &RecordRuleRow) -> StoreResult<RecordRuleRow> {
        sqlx::query(
            &self.sql("INSERT INTO record_rule (id, name, group_id, model, domain, perm_read, perm_write, perm_create, perm_delete) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        ).bind(&rule.id).bind(&rule.name).bind(&rule.group_id).bind(&rule.model)
         .bind(&rule.domain).bind(rule.perm_read as i32).bind(rule.perm_write as i32)
         .bind(rule.perm_create as i32).bind(rule.perm_delete as i32)
         .execute(&self.pool).await?;
        Ok(rule.clone())
    }

    pub async fn get_record_rule(&self, id: &str) -> StoreResult<Option<RecordRuleRow>> {
        let row = sqlx::query(&self.sql("SELECT * FROM record_rule WHERE id = ?"))
            .bind(id).fetch_optional(&self.pool).await?;
        Ok(row.as_ref().map(|r| Self::row_to_record_rule(r)))
    }

    pub async fn list_record_rules(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<RecordRuleRow>> {
        let rows = match group_id {
            Some(gid) => sqlx::query(&self.sql("SELECT * FROM record_rule WHERE group_id = ? ORDER BY model, name"))
                .bind(gid).fetch_all(&self.pool).await?,
            None => sqlx::query(&self.sql("SELECT * FROM record_rule ORDER BY model, name"))
                .fetch_all(&self.pool).await?,
        };
        Ok(rows.iter().map(|r| Self::row_to_record_rule(r)).collect())
    }

    pub async fn update_record_rule(&self, id: &str, rule: &RecordRuleRow) -> StoreResult<Option<RecordRuleRow>> {
        let result = sqlx::query(
            &self.sql("UPDATE record_rule SET name=?, group_id=?, model=?, domain=?, perm_read=?, perm_write=?, perm_create=?, perm_delete=? WHERE id=?")
        ).bind(&rule.name).bind(&rule.group_id).bind(&rule.model).bind(&rule.domain)
         .bind(rule.perm_read as i32).bind(rule.perm_write as i32).bind(rule.perm_create as i32).bind(rule.perm_delete as i32)
         .bind(id).execute(&self.pool).await?;
        if result.rows_affected() == 0 { return Ok(None); }
        Ok(Some(rule.clone()))
    }

    pub async fn delete_record_rule(&self, id: &str) -> StoreResult<bool> {
        let r = sqlx::query(&self.sql("DELETE FROM record_rule WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn get_model_accesses_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<ModelAccessRow>> {
        if group_names.is_empty() {
            return Ok(vec![]);
        }
        let placeholders: Vec<&str> = group_names.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT ma.* FROM model_access ma JOIN groups g ON ma.group_id = g.id WHERE g.name IN ({})",
            placeholders.join(", ")
        );
        let prepared = self.sql(&sql);
        let mut query = sqlx::query(&prepared);
        for name in group_names {
            query = query.bind(name);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows.iter().map(|r| Self::row_to_model_access(r)).collect())
    }

    pub async fn get_record_rules_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<RecordRuleRow>> {
        if group_names.is_empty() {
            return Ok(vec![]);
        }
        let placeholders: Vec<&str> = group_names.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT rr.* FROM record_rule rr JOIN groups g ON rr.group_id = g.id WHERE g.name IN ({})",
            placeholders.join(", ")
        );
        let prepared = self.sql(&sql);
        let mut query = sqlx::query(&prepared);
        for name in group_names {
            query = query.bind(name);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows.iter().map(|r| Self::row_to_record_rule(r)).collect())
    }
}

#[async_trait]
impl GroupStore for Db {
    async fn create_group(&self, group: &Group) -> StoreResult<Group> { Db::create_group(self, group).await }
    async fn get_group(&self, id: &GroupId) -> StoreResult<Option<Group>> { Db::get_group(self, id).await }
    async fn get_group_by_name(&self, name: &str) -> StoreResult<Option<Group>> { Db::get_group_by_name(self, name).await }
    async fn list_groups(&self) -> StoreResult<Vec<Group>> { Db::list_groups(self).await }
    async fn update_group(&self, id: &GroupId, display_name: Option<&str>, comment: Option<&str>) -> StoreResult<Option<Group>> { Db::update_group(self, id, display_name, comment).await }
    async fn delete_group(&self, id: &GroupId) -> StoreResult<bool> { Db::delete_group(self, id).await }
    async fn set_implied_groups(&self, group_id: &GroupId, implied_ids: &[GroupId]) -> StoreResult<()> { Db::set_implied_groups(self, group_id, implied_ids).await }
    async fn get_implied_groups(&self, group_id: &GroupId) -> StoreResult<Vec<GroupImplied>> { Db::get_implied_groups(self, group_id).await }
    async fn add_user_to_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()> { Db::add_user_to_group(self, user_id, group_id).await }
    async fn remove_user_from_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()> { Db::remove_user_from_group(self, user_id, group_id).await }
    async fn get_user_groups(&self, user_id: &UserId) -> StoreResult<Vec<Group>> { Db::get_user_groups(self, user_id).await }
    async fn resolve_all_groups(&self, user_id: &UserId) -> StoreResult<Vec<String>> { Db::resolve_all_groups(self, user_id).await }
    async fn set_user_groups(&self, user_id: &UserId, group_ids: &[GroupId]) -> StoreResult<()> { Db::set_user_groups(self, user_id, group_ids).await }
}

#[async_trait]
impl AccessStore for Db {
    async fn create_model_access(&self, access: &ModelAccessRow) -> StoreResult<ModelAccessRow> { Db::create_model_access(self, access).await }
    async fn get_model_access(&self, id: &str) -> StoreResult<Option<ModelAccessRow>> { Db::get_model_access(self, id).await }
    async fn list_model_accesses(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<ModelAccessRow>> { Db::list_model_accesses(self, group_id).await }
    async fn update_model_access(&self, id: &str, access: &ModelAccessRow) -> StoreResult<Option<ModelAccessRow>> { Db::update_model_access(self, id, access).await }
    async fn delete_model_access(&self, id: &str) -> StoreResult<bool> { Db::delete_model_access(self, id).await }
    async fn create_record_rule(&self, rule: &RecordRuleRow) -> StoreResult<RecordRuleRow> { Db::create_record_rule(self, rule).await }
    async fn get_record_rule(&self, id: &str) -> StoreResult<Option<RecordRuleRow>> { Db::get_record_rule(self, id).await }
    async fn list_record_rules(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<RecordRuleRow>> { Db::list_record_rules(self, group_id).await }
    async fn update_record_rule(&self, id: &str, rule: &RecordRuleRow) -> StoreResult<Option<RecordRuleRow>> { Db::update_record_rule(self, id, rule).await }
    async fn delete_record_rule(&self, id: &str) -> StoreResult<bool> { Db::delete_record_rule(self, id).await }
    async fn get_model_accesses_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<ModelAccessRow>> { Db::get_model_accesses_for_groups(self, group_names).await }
    async fn get_record_rules_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<RecordRuleRow>> { Db::get_record_rules_for_groups(self, group_names).await }
}

#[async_trait]
impl AuditStore for Db {
    async fn create_audit_log(
        &self,
        user_id: Option<&str>,
        action: &str,
        resource: &str,
        resource_id: Option<&str>,
        detail: Option<serde_json::Value>,
        ip: Option<&str>,
    ) -> Result<AuditEntry, anyhow::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let detail_str = detail.map(|v| v.to_string());
        let row: ingjoo_core::db::models::AuditLog = sqlx::query_as::<_, ingjoo_core::db::models::AuditLog>(
            &self.sql("INSERT INTO audit_logs (id, user_id, action, resource, resource_id, detail, ip) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *")
        )
        .bind(&id)
        .bind(user_id)
        .bind(action)
        .bind(resource)
        .bind(resource_id)
        .bind(&detail_str)
        .bind(ip)
        .fetch_one(&self.pool).await?;

        Ok(AuditEntry {
            id: row.id,
            user_id: row.user_id,
            action: row.action,
            resource: row.resource,
            resource_id: row.resource_id,
            detail: row.detail.and_then(|s| serde_json::from_str(&s).ok()),
            ip: row.ip,
            created_at: row.created_at,
        })
    }

    async fn list_audit_logs(&self, query: AuditQuery) -> Result<Vec<AuditEntry>, anyhow::Error> {
        let mut sql_parts = vec!["SELECT * FROM audit_logs".to_string()];
        let mut conditions: Vec<String> = Vec::new();

        if query.user_id.is_some() { conditions.push("user_id = ?".to_string()); }
        if query.action.is_some() { conditions.push("action = ?".to_string()); }
        if query.resource.is_some() { conditions.push("resource = ?".to_string()); }
        if query.resource_id.is_some() { conditions.push("resource_id = ?".to_string()); }
        if query.ip.is_some() { conditions.push("ip = ?".to_string()); }

        if !conditions.is_empty() {
            sql_parts.push("WHERE".to_string());
            sql_parts.push(conditions.join(" AND "));
        }

        sql_parts.push("ORDER BY created_at DESC".to_string());

        let limit = query.limit.unwrap_or(100);
        let offset = query.offset.unwrap_or(0);
        sql_parts.push(format!("LIMIT {} OFFSET {}", limit, offset));

        let sql = self.sql(&sql_parts.join(" "));
        let mut q = sqlx::query(&sql);

        if let Some(ref v) = query.user_id { q = q.bind(v); }
        if let Some(ref v) = query.action { q = q.bind(v); }
        if let Some(ref v) = query.resource { q = q.bind(v); }
        if let Some(ref v) = query.resource_id { q = q.bind(v); }
        if let Some(ref v) = query.ip { q = q.bind(v); }

        let rows = q.fetch_all(&self.pool).await?;
        let entries: Vec<AuditEntry> = rows.iter().map(|row| {
            use sqlx::Row;
            AuditEntry {
                id: row.get("id"),
                user_id: row.get("user_id"),
                action: row.get("action"),
                resource: row.get("resource"),
                resource_id: row.get("resource_id"),
                detail: row.get::<Option<String>, _>("detail")
                    .and_then(|s| serde_json::from_str(&s).ok()),
                ip: row.get("ip"),
                created_at: row.get("created_at"),
            }
        }).collect();

        Ok(entries)
    }

    async fn get_audit_log(&self, id: &str) -> Result<Option<AuditEntry>, anyhow::Error> {
        let row = sqlx::query(&self.sql("SELECT * FROM audit_logs WHERE id = ?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|row| {
            use sqlx::Row;
            AuditEntry {
                id: row.get("id"),
                user_id: row.get("user_id"),
                action: row.get("action"),
                resource: row.get("resource"),
                resource_id: row.get("resource_id"),
                detail: row.get::<Option<String>, _>("detail")
                    .and_then(|s| serde_json::from_str(&s).ok()),
                ip: row.get("ip"),
                created_at: row.get("created_at"),
            }
        }))
    }
}

#[async_trait]
impl traits::IngjooTransaction for Db {
    async fn run_in_transaction<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce() -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>> + Send,
        T: Send,
    {
        let tx = self.pool.begin().await?;
        let result = f().await;
        match &result {
            Ok(_) => { tx.commit().await?; }
            Err(_) => { let _ = tx.rollback().await; }
        }
        result
    }
}
