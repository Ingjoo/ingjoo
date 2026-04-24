pub mod ids;
pub mod models;
pub mod traits;
#[cfg(feature = "mock")]
pub mod mock;

use anyhow::Result;
use async_trait::async_trait;
use ingjoo_core::pool::Pool;
use std::pin::Pin;
use ingjoo_core::PaginatedResult;
use ingjoo_core::Dialect;
use ingjoo_core::db::traits::*;
use ingjoo_core::db::ids::UserId;
use ingjoo_core::db::models::UserPublic;

pub use ingjoo_core::db::models::User;
pub use ingjoo_core::db::models::UserPreferences;
pub use ingjoo_core::db::models::UpdatePreferences;
pub use ingjoo_core::db::models::Attachment;
pub use ingjoo_core::db::models::CreateAttachment;
pub use ingjoo_core::db::models::ModuleSetting;
pub use ingjoo_core::db::models::SetModuleSetting;

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
                "#;

        for stmt in Dialect::split_ddl(&dialect.prepare(ddl)) {
            sqlx::query(&stmt).execute(pool).await?;
        }

        if *dialect == Dialect::Sqlite {
            migrate_name_field(pool).await;
        }
        migrate_attachments_storage_type(pool, dialect).await;
        migrate_notification_channels(pool, dialect).await;

        Ok(())
    }
}

async fn migrate_notification_channels(pool: &Pool, dialect: &Dialect) {
    let _ = sqlx::query(
        &dialect.prepare("ALTER TABLE user_preferences ADD COLUMN notification_channels TEXT NOT NULL DEFAULT '{\"email\":true,\"in_app\":true,\"push\":false}'"),
    )
    .execute(pool)
    .await;
}

async fn migrate_attachments_storage_type(pool: &Pool, dialect: &Dialect) {
    let _ = sqlx::query(
        &dialect.prepare("ALTER TABLE attachments ADD COLUMN storage_type TEXT NOT NULL DEFAULT 'local'"),
    )
    .execute(pool)
    .await;
}

async fn migrate_name_field(pool: &Pool) {
    let _ = sqlx::query(
        "ALTER TABLE users RENAME COLUMN username TO name",
    )
    .execute(pool)
    .await;
}

impl Db {
    // ==================== Users ====================

    pub async fn create_user(&self, user: &User) -> Result<User> {
        sqlx::query_as::<_, User>(
            &self.sql("INSERT INTO users (id, email, name, password_hash, avatar_url, bio, role, oauth_provider, oauth_id, phone)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *")
        )
        .bind(&user.id).bind(&user.email).bind(&user.name)
        .bind(&user.password_hash).bind(&user.avatar_url).bind(&user.bio)
        .bind(&user.role).bind(&user.oauth_provider).bind(&user.oauth_id).bind(&user.phone)
        .fetch_one(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users WHERE email = ?"))
            .bind(email).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_user_by_id(&self, id: &UserId) -> Result<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users WHERE id = ?"))
            .bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_user_by_oauth(&self, provider: &str, oauth_id: &str) -> Result<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users WHERE oauth_provider = ? AND oauth_id = ?"))
            .bind(provider).bind(oauth_id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn update_user(&self, id: &UserId, name: Option<&str>, avatar_url: Option<&str>, bio: Option<&str>) -> Result<Option<User>> {
        sqlx::query_as::<_, User>(
            &self.sql("UPDATE users SET name=COALESCE(?, name), avatar_url=COALESCE(?, avatar_url), bio=COALESCE(?, bio), updated_at=datetime('now') WHERE id=? RETURNING *")
        )
        .bind(name).bind(avatar_url).bind(bio)
        .bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn update_user_role(&self, id: &UserId, role: &str) -> Result<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("UPDATE users SET role=?, updated_at=datetime('now') WHERE id=? RETURNING *"))
            .bind(role).bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn update_user_password(&self, id: &UserId, password_hash: &str) -> Result<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("UPDATE users SET password_hash=?, updated_at=datetime('now') WHERE id=? RETURNING *"))
            .bind(password_hash).bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_user_by_phone(&self, phone: &str) -> Result<Option<User>> {
        sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users WHERE phone = ?"))
            .bind(phone).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn list_users(&self, limit: i64, offset: i64) -> Result<PaginatedResult<User>> {
        let total: i64 = sqlx::query_scalar(&self.sql("SELECT COUNT(*) FROM users"))
            .fetch_one(&self.pool).await?;
        let items = sqlx::query_as::<_, User>(&self.sql("SELECT * FROM users ORDER BY created_at DESC LIMIT ? OFFSET ?"))
            .bind(limit).bind(offset).fetch_all(&self.pool).await?;
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    pub async fn search_users(&self, query: &str, limit: i64) -> Result<Vec<UserPublic>> {
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

    pub async fn create_refresh_token(&self, id: &str, user_id: &UserId, token_hash: &str, expires_at: &str) -> Result<()> {
        sqlx::query(&self.sql("INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at) VALUES (?, ?, ?, ?)"))
            .bind(id).bind(user_id).bind(token_hash).bind(expires_at).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_refresh_token(&self, token_hash: &str) -> Result<Option<(String, String)>> {
        sqlx::query_as(&self.sql("SELECT user_id, expires_at FROM refresh_tokens WHERE token_hash = ?"))
            .bind(token_hash).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn delete_refresh_token(&self, token_hash: &str) -> Result<()> {
        sqlx::query(&self.sql("DELETE FROM refresh_tokens WHERE token_hash = ?")).bind(token_hash).execute(&self.pool).await?;
        Ok(())
    }

    // ==================== Password Reset ====================

    pub async fn create_password_reset_token(&self, id: &str, user_id: &UserId, token: &str, expires_at: &str) -> Result<()> {
        sqlx::query(&self.sql("INSERT INTO password_reset_tokens (id, user_id, token, expires_at) VALUES (?, ?, ?, ?)"))
            .bind(id).bind(user_id).bind(token).bind(expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_password_reset_token(&self, token: &str) -> Result<Option<(String, String, i64, String)>> {
        sqlx::query_as(&self.sql("SELECT user_id, expires_at, used, id FROM password_reset_tokens WHERE token = ?"))
            .bind(token).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn mark_password_reset_used(&self, id: &str) -> Result<()> {
        sqlx::query(&self.sql("UPDATE password_reset_tokens SET used = 1 WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }

    // ==================== Captcha ====================

    pub async fn create_captcha(&self, id: &str, answer: &str, expires_at: &str) -> Result<()> {
        sqlx::query(&self.sql("INSERT INTO captcha_codes (id, answer, expires_at) VALUES (?, ?, ?)"))
            .bind(id).bind(answer).bind(expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_captcha(&self, id: &str) -> Result<Option<(String, i64, String)>> {
        sqlx::query_as(&self.sql("SELECT answer, used, expires_at FROM captcha_codes WHERE id = ?"))
            .bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn mark_captcha_used(&self, id: &str) -> Result<()> {
        sqlx::query(&self.sql("UPDATE captcha_codes SET used = 1 WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }

    // ==================== SMS ====================

    pub async fn create_sms_code(&self, id: &str, phone: &str, code: &str, purpose: &str, ip_address: Option<&str>, expires_at: &str) -> Result<()> {
        sqlx::query(&self.sql("INSERT INTO sms_codes (id, phone, code, purpose, ip_address, expires_at) VALUES (?, ?, ?, ?, ?, ?)"))
            .bind(id).bind(phone).bind(code).bind(purpose).bind(ip_address).bind(expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_latest_sms_code(&self, phone: &str, purpose: &str) -> Result<Option<(String, String, i64, String)>> {
        sqlx::query_as(&self.sql("SELECT code, expires_at, used, id FROM sms_codes WHERE phone = ? AND purpose = ? ORDER BY created_at DESC LIMIT 1"))
            .bind(phone).bind(purpose).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn get_sms_code_sent_within(&self, phone: &str, purpose: &str, seconds: i64) -> Result<bool> {
        let sql = self.dialect.format_sql(&format!("SELECT COUNT(*) FROM sms_codes WHERE phone = ? AND purpose = ? AND created_at > {}", self.dialect.now_offset_bind("seconds")));
        let (count,): (i64,) = sqlx::query_as(&sql)
            .bind(phone).bind(purpose).bind(format!("-{}", seconds))
            .fetch_one(&self.pool).await?;
        Ok(count > 0)
    }

    pub async fn mark_sms_code_used(&self, id: &str) -> Result<()> {
        sqlx::query(&self.sql("UPDATE sms_codes SET used = 1 WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn count_sms_by_ip(&self, ip: &str, hours: i64) -> Result<i64> {
        let sql = self.dialect.format_sql(&format!("SELECT COUNT(*) FROM sms_codes WHERE ip_address = ? AND created_at > {}", self.dialect.now_offset_bind("hours")));
        let (count,): (i64,) = sqlx::query_as(&sql)
            .bind(ip).bind(format!("-{}", hours))
            .fetch_one(&self.pool).await?;
        Ok(count)
    }

    pub async fn count_sms_by_phone_today(&self, phone: &str) -> Result<i64> {
        let sql = self.dialect.format_sql(&format!("SELECT COUNT(*) FROM sms_codes WHERE phone = ? AND created_at > {}", self.dialect.now_offset_negative("24 hours")));
        let (count,): (i64,) = sqlx::query_as(&sql)
            .bind(phone).fetch_one(&self.pool).await?;
        Ok(count)
    }

    // ==================== Settings ====================

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(&self.sql("SELECT value FROM settings WHERE key = ?"))
            .bind(key).fetch_optional(&self.pool).await?;
        Ok(row.map(|(v,)| v))
    }

    pub async fn get_all_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        let rows: Vec<(String, String)> = sqlx::query_as(&self.sql("SELECT key, value FROM settings ORDER BY key"))
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            &self.sql("INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')")
        ).bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn set_settings(&self, pairs: &[(String, String)]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for (k, v) in pairs {
            sqlx::query(
                &self.sql("INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')")
            ).bind(k).bind(v).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn seed_settings(&self, pairs: &[(String, String)]) -> Result<()> {
        for (key, value) in pairs {
            sqlx::query(&self.sql("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO NOTHING"))
                .bind(key).bind(value).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn get_user_preferences(&self, user_id: &UserId) -> Result<UserPreferences> {
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

    pub async fn upsert_user_preferences(&self, user_id: &UserId, input: &UpdatePreferences) -> Result<UserPreferences> {
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

    pub async fn create_attachment(&self, id: &str, user_id: &UserId, input: &CreateAttachment, storage_path: &str) -> Result<Attachment> {
        sqlx::query_as::<_, Attachment>(
            &self.sql("INSERT INTO attachments (id, user_id, filename, mime_type, size, storage_path, entity_type, entity_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *")
        )
        .bind(id).bind(user_id).bind(&input.filename).bind(&input.mime_type)
        .bind(input.size).bind(storage_path)
        .bind(&input.entity_type).bind(&input.entity_id)
        .fetch_one(&self.pool).await.map_err(Into::into)
    }

    pub async fn list_attachments(&self, entity_type: Option<&str>, entity_id: Option<&str>, limit: i64, offset: i64) -> Result<PaginatedResult<Attachment>> {
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

    pub async fn get_attachment(&self, id: &str) -> Result<Option<Attachment>> {
        sqlx::query_as::<_, Attachment>(&self.sql("SELECT * FROM attachments WHERE id = ?"))
            .bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn delete_attachment(&self, id: &str) -> Result<bool> {
        let result = sqlx::query(&self.sql("DELETE FROM attachments WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    // ==================== Module Settings ====================

    pub async fn list_module_settings(&self, scope: &str, scope_id: Option<&str>, module: Option<&str>) -> Result<Vec<ModuleSetting>> {
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

    pub async fn get_module_setting(&self, scope: &str, scope_id: Option<&str>, module: &str, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            &self.sql("SELECT value FROM module_settings WHERE scope = ? AND scope_id IS ? AND module = ? AND key = ?")
        ).bind(scope).bind(scope_id).bind(module).bind(key)
         .fetch_optional(&self.pool).await?;
        Ok(row.map(|(v,)| v))
    }

    pub async fn set_module_setting(&self, id: &str, scope: &str, scope_id: Option<&str>, module: &str, key: &str, value: &str) -> Result<ModuleSetting> {
        sqlx::query_as::<_, ModuleSetting>(
            &self.sql("INSERT INTO module_settings (id, scope, scope_id, module, key, value, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
             ON CONFLICT(scope, scope_id, module, key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')
             RETURNING *")
        ).bind(id).bind(scope).bind(scope_id).bind(module).bind(key).bind(value)
         .fetch_one(&self.pool).await.map_err(Into::into)
    }

    pub async fn delete_module_setting(&self, id: &str) -> Result<bool> {
        let result = sqlx::query(&self.sql("DELETE FROM module_settings WHERE id = ?"))
            .bind(id).execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_effective_setting(&self, module: &str, key: &str, collection_id: Option<&str>) -> Result<Option<String>> {
        if let Some(ws_id) = collection_id {
            let ws_val = self.get_module_setting("document_collection", Some(ws_id), module, key).await?;
            if ws_val.is_some() { return Ok(ws_val); }
        }
        self.get_module_setting("system", None, module, key).await
    }

    // ==================== Storage Migration ====================

    pub async fn list_all_attachments(&self, limit: i64, offset: i64) -> Result<PaginatedResult<Attachment>> {
        let total: i64 = sqlx::query_scalar(&self.sql("SELECT COUNT(*) FROM attachments"))
            .fetch_one(&self.pool).await?;
        let items = sqlx::query_as::<_, Attachment>(
            &self.sql("SELECT * FROM attachments ORDER BY created_at ASC LIMIT ? OFFSET ?")
        )
        .bind(limit).bind(offset)
        .fetch_all(&self.pool).await?;
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    pub async fn update_attachment_storage(&self, id: &str, storage_path: &str, storage_type: &str) -> Result<bool> {
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
    async fn create_user(&self, user: &User) -> Result<User> { Db::create_user(self, user).await }
    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>> { Db::get_user_by_email(self, email).await }
    async fn get_user_by_id(&self, id: &UserId) -> Result<Option<User>> { Db::get_user_by_id(self, id).await }
    async fn get_user_by_oauth(&self, provider: &str, oauth_id: &str) -> Result<Option<User>> { Db::get_user_by_oauth(self, provider, oauth_id).await }
    async fn get_user_by_phone(&self, phone: &str) -> Result<Option<User>> { Db::get_user_by_phone(self, phone).await }
    async fn update_user(&self, id: &UserId, name: Option<&str>, avatar_url: Option<&str>, bio: Option<&str>) -> Result<Option<User>> { Db::update_user(self, id, name, avatar_url, bio).await }
    async fn update_user_role(&self, id: &UserId, role: &str) -> Result<Option<User>> { Db::update_user_role(self, id, role).await }
    async fn update_user_password(&self, id: &UserId, password_hash: &str) -> Result<Option<User>> { Db::update_user_password(self, id, password_hash).await }
    async fn list_users(&self, limit: i64, offset: i64) -> Result<PaginatedResult<User>> { Db::list_users(self, limit, offset).await }
    async fn search_users(&self, query: &str, limit: i64) -> Result<Vec<UserPublic>> { Db::search_users(self, query, limit).await }
}

#[async_trait]
impl TokenStore for Db {
    async fn create_refresh_token(&self, id: &str, user_id: &UserId, token_hash: &str, expires_at: &str) -> Result<()> { Db::create_refresh_token(self, id, user_id, token_hash, expires_at).await }
    async fn get_refresh_token(&self, token_hash: &str) -> Result<Option<(String, String)>> { Db::get_refresh_token(self, token_hash).await }
    async fn delete_refresh_token(&self, token_hash: &str) -> Result<()> { Db::delete_refresh_token(self, token_hash).await }
    async fn create_password_reset_token(&self, id: &str, user_id: &UserId, token: &str, expires_at: &str) -> Result<()> { Db::create_password_reset_token(self, id, user_id, token, expires_at).await }
    async fn get_password_reset_token(&self, token: &str) -> Result<Option<(String, String, i64, String)>> { Db::get_password_reset_token(self, token).await }
    async fn mark_password_reset_used(&self, id: &str) -> Result<()> { Db::mark_password_reset_used(self, id).await }
}

#[async_trait]
impl CaptchaStore for Db {
    async fn create_captcha(&self, id: &str, answer: &str, expires_at: &str) -> Result<()> { Db::create_captcha(self, id, answer, expires_at).await }
    async fn get_captcha(&self, id: &str) -> Result<Option<(String, i64, String)>> { Db::get_captcha(self, id).await }
    async fn mark_captcha_used(&self, id: &str) -> Result<()> { Db::mark_captcha_used(self, id).await }
}

#[async_trait]
impl SmsCodeStore for Db {
    async fn create_sms_code(&self, id: &str, phone: &str, code: &str, purpose: &str, ip_address: Option<&str>, expires_at: &str) -> Result<()> { Db::create_sms_code(self, id, phone, code, purpose, ip_address, expires_at).await }
    async fn get_latest_sms_code(&self, phone: &str, purpose: &str) -> Result<Option<(String, String, i64, String)>> { Db::get_latest_sms_code(self, phone, purpose).await }
    async fn get_sms_code_sent_within(&self, phone: &str, purpose: &str, seconds: i64) -> Result<bool> { Db::get_sms_code_sent_within(self, phone, purpose, seconds).await }
    async fn mark_sms_code_used(&self, id: &str) -> Result<()> { Db::mark_sms_code_used(self, id).await }
    async fn count_sms_by_ip(&self, ip: &str, hours: i64) -> Result<i64> { Db::count_sms_by_ip(self, ip, hours).await }
    async fn count_sms_by_phone_today(&self, phone: &str) -> Result<i64> { Db::count_sms_by_phone_today(self, phone).await }
}

#[async_trait]
impl SettingsStore for Db {
    async fn get_setting(&self, key: &str) -> Result<Option<String>> { Db::get_setting(self, key).await }
    async fn get_all_settings(&self) -> Result<std::collections::HashMap<String, String>> { Db::get_all_settings(self).await }
    async fn set_setting(&self, key: &str, value: &str) -> Result<()> { Db::set_setting(self, key, value).await }
    async fn set_settings(&self, pairs: &[(String, String)]) -> Result<()> { Db::set_settings(self, pairs).await }
    async fn seed_settings(&self, pairs: &[(String, String)]) -> Result<()> { Db::seed_settings(self, pairs).await }
}

#[async_trait]
impl PreferenceStore for Db {
    async fn get_user_preferences(&self, user_id: &UserId) -> Result<UserPreferences> { Db::get_user_preferences(self, user_id).await }
    async fn upsert_user_preferences(&self, user_id: &UserId, input: &UpdatePreferences) -> Result<UserPreferences> { Db::upsert_user_preferences(self, user_id, input).await }
}

#[async_trait]
impl AttachmentStore for Db {
    async fn create_attachment(&self, id: &str, user_id: &UserId, input: &CreateAttachment, storage_path: &str) -> Result<Attachment> { Db::create_attachment(self, id, user_id, input, storage_path).await }
    async fn list_attachments(&self, entity_type: Option<&str>, entity_id: Option<&str>, limit: i64, offset: i64) -> Result<PaginatedResult<Attachment>> { Db::list_attachments(self, entity_type, entity_id, limit, offset).await }
    async fn get_attachment(&self, id: &str) -> Result<Option<Attachment>> { Db::get_attachment(self, id).await }
    async fn delete_attachment(&self, id: &str) -> Result<bool> { Db::delete_attachment(self, id).await }
    async fn list_all_attachments(&self, limit: i64, offset: i64) -> Result<PaginatedResult<Attachment>> { Db::list_all_attachments(self, limit, offset).await }
    async fn update_attachment_storage(&self, id: &str, storage_path: &str, storage_type: &str) -> Result<bool> { Db::update_attachment_storage(self, id, storage_path, storage_type).await }
}

#[async_trait]
impl ModuleSettingStore for Db {
    async fn list_module_settings(&self, scope: &str, scope_id: Option<&str>, module: Option<&str>) -> Result<Vec<ModuleSetting>> { Db::list_module_settings(self, scope, scope_id, module).await }
    async fn get_module_setting(&self, scope: &str, scope_id: Option<&str>, module: &str, key: &str) -> Result<Option<String>> { Db::get_module_setting(self, scope, scope_id, module, key).await }
    async fn set_module_setting(&self, id: &str, scope: &str, scope_id: Option<&str>, module: &str, key: &str, value: &str) -> Result<ModuleSetting> { Db::set_module_setting(self, id, scope, scope_id, module, key, value).await }
    async fn delete_module_setting(&self, id: &str) -> Result<bool> { Db::delete_module_setting(self, id).await }
    async fn get_effective_setting(&self, module: &str, key: &str, collection_id: Option<&str>) -> Result<Option<String>> { Db::get_effective_setting(self, module, key, collection_id).await }
}

#[async_trait]
impl traits::ScaffTransaction for Db {
    async fn run_in_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>> + Send,
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
