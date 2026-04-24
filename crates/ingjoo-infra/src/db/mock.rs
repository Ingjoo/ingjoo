use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;
use tokio::sync::Mutex;

use super::models::*;
use super::ids::UserId;
use super::traits::*;
use ingjoo_core::PaginatedResult;

struct RefreshTokenEntry {
    _id: String,
    user_id: String,
    expires_at: String,
}

struct PasswordResetEntry {
    id: String,
    user_id: String,
    token: String,
    expires_at: String,
    used: bool,
}

struct CaptchaEntry {
    answer: String,
    expires_at: String,
    used: bool,
}

struct SmsCodeEntry {
    id: String,
    phone: String,
    code: String,
    purpose: String,
    ip_address: Option<String>,
    expires_at: String,
    used: bool,
}

pub struct MockScaffDb {
    users: Mutex<HashMap<String, User>>,
    refresh_tokens: Mutex<HashMap<String, RefreshTokenEntry>>,
    password_reset_tokens: Mutex<HashMap<String, PasswordResetEntry>>,
    captcha: Mutex<HashMap<String, CaptchaEntry>>,
    sms_codes: Mutex<HashMap<String, SmsCodeEntry>>,
    settings: Mutex<HashMap<String, String>>,
    preferences: Mutex<HashMap<String, UserPreferences>>,
    attachments: Mutex<HashMap<String, Attachment>>,
    module_settings: Mutex<HashMap<String, ModuleSetting>>,
}

impl MockScaffDb {
    pub fn new() -> Self {
        Self {
            users: Mutex::new(HashMap::new()),
            refresh_tokens: Mutex::new(HashMap::new()),
            password_reset_tokens: Mutex::new(HashMap::new()),
            captcha: Mutex::new(HashMap::new()),
            sms_codes: Mutex::new(HashMap::new()),
            settings: Mutex::new(HashMap::new()),
            preferences: Mutex::new(HashMap::new()),
            attachments: Mutex::new(HashMap::new()),
            module_settings: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl UserStore for MockScaffDb {
    async fn create_user(&self, user: &User) -> Result<User> {
        let stored = user.clone();
        self.users.lock().await.insert(user.id.to_string(), stored);
        Ok(user.clone())
    }

    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>> {
        let users = self.users.lock().await;
        Ok(users
            .values()
            .find(|u| u.email == email)
            .cloned())
    }

    async fn get_user_by_id(&self, id: &UserId) -> Result<Option<User>> {
        let users = self.users.lock().await;
        Ok(users.get(id.as_ref()).cloned())
    }

    async fn get_user_by_oauth(
        &self,
        provider: &str,
        oauth_id: &str,
    ) -> Result<Option<User>> {
        let users = self.users.lock().await;
        Ok(users
            .values()
            .find(|u| {
                u.oauth_provider.as_deref() == Some(provider)
                    && u.oauth_id.as_deref() == Some(oauth_id)
            })
            .cloned())
    }

    async fn get_user_by_phone(&self, phone: &str) -> Result<Option<User>> {
        let users = self.users.lock().await;
        Ok(users
            .values()
            .find(|u| u.phone.as_deref() == Some(phone))
            .cloned())
    }

    async fn update_user(
        &self,
        id: &UserId,
        name: Option<&str>,
        avatar_url: Option<&str>,
        bio: Option<&str>,
    ) -> Result<Option<User>> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.get_mut(id.as_ref()) {
            if let Some(v) = name {
                user.name = v.to_string();
            }
            if let Some(v) = avatar_url {
                user.avatar_url = Some(v.to_string());
            }
            if let Some(v) = bio {
                user.bio = Some(v.to_string());
            }
            user.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(Some(user.clone()))
        } else {
            Ok(None)
        }
    }

    async fn update_user_role(&self, id: &UserId, role: &str) -> Result<Option<User>> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.get_mut(id.as_ref()) {
            user.role = role.to_string();
            user.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(Some(user.clone()))
        } else {
            Ok(None)
        }
    }

    async fn update_user_password(
        &self,
        id: &UserId,
        password_hash: &str,
    ) -> Result<Option<User>> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.get_mut(id.as_ref()) {
            user.password_hash = Some(password_hash.to_string());
            user.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(Some(user.clone()))
        } else {
            Ok(None)
        }
    }

    async fn list_users(&self, limit: i64, offset: i64) -> Result<PaginatedResult<User>> {
        use ingjoo_core::PaginatedResult;
        let users = self.users.lock().await;
        let total = users.len() as i64;
        let mut list: Vec<User> = users.values().cloned().collect();
        list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let items: Vec<User> = list.into_iter().skip(offset as usize).take(limit as usize).collect();
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    async fn search_users(&self, query: &str, limit: i64) -> Result<Vec<crate::db::models::UserPublic>> {
        let users = self.users.lock().await;
        let q = query.to_lowercase();
        let results: Vec<crate::db::models::UserPublic> = users.values()
            .filter(|u| u.name.to_lowercase().contains(&q) || u.email.to_lowercase().contains(&q))
            .map(|u| crate::db::models::UserPublic::from(u))
            .take(limit as usize)
            .collect();
        Ok(results)
    }
}

#[async_trait]
impl TokenStore for MockScaffDb {
    async fn create_refresh_token(
        &self,
        id: &str,
        user_id: &UserId,
        token_hash: &str,
        expires_at: &str,
    ) -> Result<()> {
        self.refresh_tokens.lock().await.insert(
            token_hash.to_string(),
            RefreshTokenEntry {
                _id: id.to_string(),
                user_id: user_id.to_string(),
                expires_at: expires_at.to_string(),
            },
        );
        Ok(())
    }

    async fn get_refresh_token(&self, token_hash: &str) -> Result<Option<(String, String)>> {
        let tokens = self.refresh_tokens.lock().await;
        Ok(tokens
            .get(token_hash)
            .map(|e| (e.user_id.clone(), e.expires_at.clone())))
    }

    async fn delete_refresh_token(&self, token_hash: &str) -> Result<()> {
        self.refresh_tokens.lock().await.remove(token_hash);
        Ok(())
    }

    async fn create_password_reset_token(
        &self,
        id: &str,
        user_id: &UserId,
        token: &str,
        expires_at: &str,
    ) -> Result<()> {
        self.password_reset_tokens.lock().await.insert(
            id.to_string(),
            PasswordResetEntry {
                id: id.to_string(),
                user_id: user_id.to_string(),
                token: token.to_string(),
                expires_at: expires_at.to_string(),
                used: false,
            },
        );
        Ok(())
    }

    async fn get_password_reset_token(
        &self,
        token: &str,
    ) -> Result<Option<(String, String, i64, String)>> {
        let tokens = self.password_reset_tokens.lock().await;
        Ok(tokens
            .values()
            .find(|e| e.token == token && !e.used)
            .map(|e| {
                (
                    e.id.clone(),
                    e.user_id.clone(),
                    0_i64,
                    e.expires_at.clone(),
                )
            }))
    }

    async fn mark_password_reset_used(&self, id: &str) -> Result<()> {
        self.password_reset_tokens.lock().await.remove(id);
        Ok(())
    }
}

#[async_trait]
impl CaptchaStore for MockScaffDb {
    async fn create_captcha(&self, id: &str, answer: &str, expires_at: &str) -> Result<()> {
        self.captcha.lock().await.insert(
            id.to_string(),
            CaptchaEntry {
                answer: answer.to_string(),
                expires_at: expires_at.to_string(),
                used: false,
            },
        );
        Ok(())
    }

    async fn get_captcha(&self, id: &str) -> Result<Option<(String, i64, String)>> {
        let captcha = self.captcha.lock().await;
        Ok(captcha.get(id).map(|e| {
            (
                e.answer.clone(),
                0_i64,
                e.expires_at.clone(),
            )
        }))
    }

    async fn mark_captcha_used(&self, id: &str) -> Result<()> {
        if let Some(entry) = self.captcha.lock().await.get_mut(id) {
            entry.used = true;
        }
        Ok(())
    }
}

#[async_trait]
impl SmsCodeStore for MockScaffDb {
    async fn create_sms_code(
        &self,
        id: &str,
        phone: &str,
        code: &str,
        purpose: &str,
        ip_address: Option<&str>,
        expires_at: &str,
    ) -> Result<()> {
        self.sms_codes.lock().await.insert(
            id.to_string(),
            SmsCodeEntry {
                id: id.to_string(),
                phone: phone.to_string(),
                code: code.to_string(),
                purpose: purpose.to_string(),
                ip_address: ip_address.map(|s| s.to_string()),
                expires_at: expires_at.to_string(),
                used: false,
            },
        );
        Ok(())
    }

    async fn get_latest_sms_code(
        &self,
        phone: &str,
        purpose: &str,
    ) -> Result<Option<(String, String, i64, String)>> {
        let codes = self.sms_codes.lock().await;
        let matching: Vec<&SmsCodeEntry> = codes
            .values()
            .filter(|e| e.phone == phone && e.purpose == purpose)
            .collect();
        Ok(matching.last().map(|e| {
            (
                e.code.clone(),
                e.id.clone(),
                if e.used { 1 } else { 0 },
                e.expires_at.clone(),
            )
        }))
    }

    async fn get_sms_code_sent_within(
        &self,
        _phone: &str,
        _purpose: &str,
        _seconds: i64,
    ) -> Result<bool> {
        Ok(false)
    }

    async fn mark_sms_code_used(&self, id: &str) -> Result<()> {
        if let Some(entry) = self.sms_codes.lock().await.get_mut(id) {
            entry.used = true;
        }
        Ok(())
    }

    async fn count_sms_by_ip(&self, _ip: &str, _hours: i64) -> Result<i64> {
        Ok(0)
    }

    async fn count_sms_by_phone_today(&self, _phone: &str) -> Result<i64> {
        Ok(0)
    }
}

#[async_trait]
impl SettingsStore for MockScaffDb {
    async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let settings = self.settings.lock().await;
        Ok(settings.get(key).cloned())
    }

    async fn get_all_settings(&self) -> Result<HashMap<String, String>> {
        let settings = self.settings.lock().await;
        Ok(settings.clone())
    }

    async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.settings
            .lock()
            .await
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn set_settings(&self, pairs: &[(String, String)]) -> Result<()> {
        let mut settings = self.settings.lock().await;
        for (k, v) in pairs {
            settings.insert(k.clone(), v.clone());
        }
        Ok(())
    }

    async fn seed_settings(&self, pairs: &[(String, String)]) -> Result<()> {
        self.set_settings(pairs).await
    }
}

#[async_trait]
impl PreferenceStore for MockScaffDb {
    async fn get_user_preferences(&self, user_id: &UserId) -> Result<UserPreferences> {
        let prefs = self.preferences.lock().await;
        Ok(prefs.get(user_id.as_ref()).cloned().unwrap_or_else(|| UserPreferences {
            user_id: user_id.clone(),
            theme: "light".to_string(),
            inline_edit: 0,
            remember_pos: 0,
            line_numbers: 0,
            last_collection: None,
            last_entry: None,
            language: "zh-CN".to_string(),
            notification_channels: Some("{\"email\":true,\"in_app\":true,\"push\":false}".to_string()),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }))
    }

    async fn upsert_user_preferences(
        &self,
        user_id: &UserId,
        input: &UpdatePreferences,
    ) -> Result<UserPreferences> {
        let mut prefs = self.preferences.lock().await;
        let existing = prefs.get(user_id.as_ref()).cloned().unwrap_or_else(|| UserPreferences {
            user_id: user_id.clone(),
            theme: "light".to_string(),
            inline_edit: 0,
            remember_pos: 0,
            line_numbers: 0,
            last_collection: None,
            last_entry: None,
            language: "zh-CN".to_string(),
            notification_channels: Some("{\"email\":true,\"in_app\":true,\"push\":false}".to_string()),
            updated_at: chrono::Utc::now().to_rfc3339(),
        });
        let mut updated = existing;
        if let Some(ref v) = input.theme {
            updated.theme = v.clone();
        }
        if let Some(v) = input.inline_edit {
            updated.inline_edit = if v { 1 } else { 0 };
        }
        if let Some(v) = input.remember_pos {
            updated.remember_pos = if v { 1 } else { 0 };
        }
        if let Some(v) = input.line_numbers {
            updated.line_numbers = if v { 1 } else { 0 };
        }
        if let Some(ref v) = input.last_collection {
            updated.last_collection = Some(v.clone());
        }
        if let Some(ref v) = input.last_entry {
            updated.last_entry = Some(v.clone());
        }
        if let Some(ref v) = input.language {
            updated.language = v.clone();
        }
        if let Some(ref v) = input.notification_channels {
            updated.notification_channels = Some(v.clone());
        }
        updated.updated_at = chrono::Utc::now().to_rfc3339();
        prefs.insert(user_id.to_string(), updated.clone());
        Ok(updated)
    }
}

#[async_trait]
impl AttachmentStore for MockScaffDb {
    async fn create_attachment(
        &self,
        id: &str,
        user_id: &UserId,
        input: &CreateAttachment,
        storage_path: &str,
    ) -> Result<Attachment> {
        let attachment = Attachment {
            id: id.to_string(),
            user_id: user_id.clone(),
            filename: input.filename.clone(),
            mime_type: input.mime_type.clone(),
            size: input.size,
            storage_path: storage_path.to_string(),
            entity_type: input.entity_type.clone(),
            entity_id: input.entity_id.clone(),
            storage_type: "local".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.attachments
            .lock()
            .await
            .insert(id.to_string(), attachment.clone());
        Ok(attachment)
    }

    async fn list_attachments(
        &self,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<PaginatedResult<Attachment>> {
        use ingjoo_core::PaginatedResult;
        let attachments = self.attachments.lock().await;
        let mut list: Vec<Attachment> = attachments
            .values()
            .filter(|a| {
                entity_type.map_or(true, |t| a.entity_type.as_deref() == Some(t))
                    && entity_id.map_or(true, |t| a.entity_id.as_deref() == Some(t))
            })
            .cloned()
            .collect();
        let total = list.len() as i64;
        list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let items = list
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    async fn get_attachment(&self, id: &str) -> Result<Option<Attachment>> {
        let attachments = self.attachments.lock().await;
        Ok(attachments.get(id).cloned())
    }

    async fn delete_attachment(&self, id: &str) -> Result<bool> {
        Ok(self.attachments.lock().await.remove(id).is_some())
    }

    async fn list_all_attachments(&self, limit: i64, offset: i64) -> Result<PaginatedResult<Attachment>> {
        use ingjoo_core::PaginatedResult;
        let attachments = self.attachments.lock().await;
        let total = attachments.len() as i64;
        let mut list: Vec<Attachment> = attachments.values().cloned().collect();
        list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let items = list
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    async fn update_attachment_storage(
        &self,
        id: &str,
        storage_path: &str,
        storage_type: &str,
    ) -> Result<bool> {
        let mut attachments = self.attachments.lock().await;
        if let Some(a) = attachments.get_mut(id) {
            a.storage_path = storage_path.to_string();
            a.storage_type = storage_type.to_string();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[async_trait]
impl ModuleSettingStore for MockScaffDb {
    async fn list_module_settings(
        &self,
        scope: &str,
        scope_id: Option<&str>,
        module: Option<&str>,
    ) -> Result<Vec<ModuleSetting>> {
        let settings = self.module_settings.lock().await;
        let list: Vec<ModuleSetting> = settings
            .values()
            .filter(|s| {
                s.scope == scope
                    && scope_id.map_or(true, |v| s.scope_id.as_deref() == Some(v))
                    && module.map_or(true, |v| s.module == v)
            })
            .cloned()
            .collect();
        Ok(list)
    }

    async fn get_module_setting(
        &self,
        scope: &str,
        scope_id: Option<&str>,
        module: &str,
        key: &str,
    ) -> Result<Option<String>> {
        let settings = self.module_settings.lock().await;
        Ok(settings
            .values()
            .find(|s| {
                s.scope == scope
                    && scope_id.map_or(true, |v| s.scope_id.as_deref() == Some(v))
                    && s.module == module
                    && s.key == key
            })
            .map(|s| s.value.clone()))
    }

    async fn set_module_setting(
        &self,
        id: &str,
        scope: &str,
        scope_id: Option<&str>,
        module: &str,
        key: &str,
        value: &str,
    ) -> Result<ModuleSetting> {
        let setting = ModuleSetting {
            id: id.to_string(),
            scope: scope.to_string(),
            scope_id: scope_id.map(|s| s.to_string()),
            module: module.to_string(),
            key: key.to_string(),
            value: value.to_string(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        self.module_settings
            .lock()
            .await
            .insert(id.to_string(), setting.clone());
        Ok(setting)
    }

    async fn delete_module_setting(&self, id: &str) -> Result<bool> {
        Ok(self.module_settings.lock().await.remove(id).is_some())
    }

    async fn get_effective_setting(
        &self,
        module: &str,
        key: &str,
        collection_id: Option<&str>,
    ) -> Result<Option<String>> {
        if let Some(cid) = collection_id {
            let collection_val = self
                .get_module_setting("collection", Some(cid), module, key)
                .await?;
            if collection_val.is_some() {
                return Ok(collection_val);
            }
        }
        self.get_module_setting("global", None, module, key).await
    }
}

#[async_trait]
impl super::traits::ScaffTransaction for MockScaffDb {
    async fn run_in_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>> + Send,
        T: Send,
    {
        f().await
    }
}
