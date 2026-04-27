use async_trait::async_trait;
use ingjoo_core::db::error::{StoreError, StoreResult};
use std::collections::HashMap;
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::ids::GroupId;
use super::ids::UserId;
use super::models::*;
use super::traits::*;
use ingjoo_core::db::models::{MailMessage, MailNotification, NotificationItem};
use ingjoo_core::extension::audit::{AuditEntry, AuditQuery, AuditStore};
use ingjoo_core::extension::notification::NotificationStore;
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
    #[allow(dead_code)]
    ip_address: Option<String>,
    expires_at: String,
    used: bool,
}

pub struct MockIngjooDb {
    users: Mutex<HashMap<String, User>>,
    refresh_tokens: Mutex<HashMap<String, RefreshTokenEntry>>,
    password_reset_tokens: Mutex<HashMap<String, PasswordResetEntry>>,
    captcha: Mutex<HashMap<String, CaptchaEntry>>,
    sms_codes: Mutex<HashMap<String, SmsCodeEntry>>,
    settings: Mutex<HashMap<String, String>>,
    preferences: Mutex<HashMap<String, UserPreferences>>,
    attachments: Mutex<HashMap<String, Attachment>>,
    module_settings: Mutex<HashMap<String, ModuleSetting>>,
    groups: Mutex<HashMap<String, Group>>,
    group_implied: Mutex<HashMap<(String, String), GroupImplied>>,
    user_groups: Mutex<HashMap<String, HashSet<String>>>,
    model_accesses: Mutex<HashMap<String, ModelAccessRow>>,
    record_rules: Mutex<HashMap<String, RecordRuleRow>>,
    audit_logs: Arc<Mutex<Vec<AuditEntry>>>,
    mail_messages: Arc<Mutex<Vec<MailMessage>>>,
    mail_notifications: Arc<Mutex<Vec<MailNotification>>>,
}

impl Default for MockIngjooDb {
    fn default() -> Self {
        Self::new()
    }
}

impl MockIngjooDb {
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
            groups: Mutex::new(HashMap::new()),
            group_implied: Mutex::new(HashMap::new()),
            user_groups: Mutex::new(HashMap::new()),
            model_accesses: Mutex::new(HashMap::new()),
            record_rules: Mutex::new(HashMap::new()),
            audit_logs: Arc::new(Mutex::new(Vec::new())),
            mail_messages: Arc::new(Mutex::new(Vec::new())),
            mail_notifications: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl UserStore for MockIngjooDb {
    async fn create_user(&self, user: &User) -> StoreResult<User> {
        // 检查邮箱唯一性
        let users = self.users.lock().await;
        if users.values().any(|u| u.email == user.email) {
            return Err(StoreError::UniqueViolation { table: "users".to_string(), column: "email".to_string() });
        }
        drop(users);
        let stored = user.clone();
        self.users.lock().await.insert(user.id.to_string(), stored);
        Ok(user.clone())
    }

    async fn get_user_by_email(&self, email: &str) -> StoreResult<Option<User>> {
        let users = self.users.lock().await;
        Ok(users.values().find(|u| u.email == email).cloned())
    }

    async fn get_user_by_id(&self, id: &UserId) -> StoreResult<Option<User>> {
        let users = self.users.lock().await;
        Ok(users.get(id.as_ref()).cloned())
    }

    async fn get_user_by_oauth(&self, provider: &str, oauth_id: &str) -> StoreResult<Option<User>> {
        let users = self.users.lock().await;
        Ok(users
            .values()
            .find(|u| u.oauth_provider.as_deref() == Some(provider) && u.oauth_id.as_deref() == Some(oauth_id))
            .cloned())
    }

    async fn get_user_by_phone(&self, phone: &str) -> StoreResult<Option<User>> {
        let users = self.users.lock().await;
        Ok(users.values().find(|u| u.phone.as_deref() == Some(phone)).cloned())
    }

    async fn update_user(
        &self,
        id: &UserId,
        name: Option<&str>,
        avatar_url: Option<&str>,
        bio: Option<&str>,
    ) -> StoreResult<Option<User>> {
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

    async fn update_user_role(&self, id: &UserId, role: &str) -> StoreResult<Option<User>> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.get_mut(id.as_ref()) {
            user.role = role.to_string();
            user.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(Some(user.clone()))
        } else {
            Ok(None)
        }
    }

    async fn update_user_password(&self, id: &UserId, password_hash: &str) -> StoreResult<Option<User>> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.get_mut(id.as_ref()) {
            user.password_hash = Some(password_hash.to_string());
            user.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(Some(user.clone()))
        } else {
            Ok(None)
        }
    }

    async fn list_users(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<User>> {
        use ingjoo_core::PaginatedResult;
        let users = self.users.lock().await;
        let total = users.len() as i64;
        let mut list: Vec<User> = users.values().cloned().collect();
        list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let items: Vec<User> = list.into_iter().skip(offset as usize).take(limit as usize).collect();
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    async fn search_users(&self, query: &str, limit: i64) -> StoreResult<Vec<crate::db::models::UserPublic>> {
        let users = self.users.lock().await;
        let q = query.to_lowercase();
        let results: Vec<crate::db::models::UserPublic> = users
            .values()
            .filter(|u| u.name.to_lowercase().contains(&q) || u.email.to_lowercase().contains(&q))
            .map(crate::db::models::UserPublic::from)
            .take(limit as usize)
            .collect();
        Ok(results)
    }
}

#[async_trait]
impl TokenStore for MockIngjooDb {
    async fn create_refresh_token(
        &self,
        id: &str,
        user_id: &UserId,
        token_hash: &str,
        expires_at: &str,
    ) -> StoreResult<()> {
        self.refresh_tokens.lock().await.insert(
            token_hash.to_string(),
            RefreshTokenEntry { _id: id.to_string(), user_id: user_id.to_string(), expires_at: expires_at.to_string() },
        );
        Ok(())
    }

    async fn get_refresh_token(&self, token_hash: &str) -> StoreResult<Option<(String, String)>> {
        let tokens = self.refresh_tokens.lock().await;
        Ok(tokens.get(token_hash).map(|e| (e.user_id.clone(), e.expires_at.clone())))
    }

    async fn delete_refresh_token(&self, token_hash: &str) -> StoreResult<()> {
        self.refresh_tokens.lock().await.remove(token_hash);
        Ok(())
    }

    async fn create_password_reset_token(
        &self,
        id: &str,
        user_id: &UserId,
        token: &str,
        expires_at: &str,
    ) -> StoreResult<()> {
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

    async fn get_password_reset_token(&self, token: &str) -> StoreResult<Option<(String, String, i64, String)>> {
        let tokens = self.password_reset_tokens.lock().await;
        Ok(tokens
            .values()
            .find(|e| e.token == token && !e.used)
            .map(|e| (e.id.clone(), e.user_id.clone(), 0_i64, e.expires_at.clone())))
    }

    async fn mark_password_reset_used(&self, id: &str) -> StoreResult<()> {
        self.password_reset_tokens.lock().await.remove(id);
        Ok(())
    }
}

#[async_trait]
impl CaptchaStore for MockIngjooDb {
    async fn create_captcha(&self, id: &str, answer: &str, expires_at: &str) -> StoreResult<()> {
        self.captcha.lock().await.insert(
            id.to_string(),
            CaptchaEntry { answer: answer.to_string(), expires_at: expires_at.to_string(), used: false },
        );
        Ok(())
    }

    async fn get_captcha(&self, id: &str) -> StoreResult<Option<(String, i64, String)>> {
        let captcha = self.captcha.lock().await;
        Ok(captcha.get(id).map(|e| (e.answer.clone(), 0_i64, e.expires_at.clone())))
    }

    async fn mark_captcha_used(&self, id: &str) -> StoreResult<()> {
        if let Some(entry) = self.captcha.lock().await.get_mut(id) {
            entry.used = true;
        }
        Ok(())
    }
}

#[async_trait]
impl SmsCodeStore for MockIngjooDb {
    async fn create_sms_code(
        &self,
        id: &str,
        phone: &str,
        code: &str,
        purpose: &str,
        ip_address: Option<&str>,
        expires_at: &str,
    ) -> StoreResult<()> {
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
    ) -> StoreResult<Option<(String, String, i64, String)>> {
        let codes = self.sms_codes.lock().await;
        let matching: Vec<&SmsCodeEntry> =
            codes.values().filter(|e| e.phone == phone && e.purpose == purpose).collect();
        Ok(matching.last().map(|e| (e.code.clone(), e.id.clone(), if e.used { 1 } else { 0 }, e.expires_at.clone())))
    }

    async fn get_sms_code_sent_within(&self, _phone: &str, _purpose: &str, _seconds: i64) -> StoreResult<bool> {
        Ok(false)
    }

    async fn mark_sms_code_used(&self, id: &str) -> StoreResult<()> {
        if let Some(entry) = self.sms_codes.lock().await.get_mut(id) {
            entry.used = true;
        }
        Ok(())
    }

    async fn count_sms_by_ip(&self, _ip: &str, _hours: i64) -> StoreResult<i64> {
        Ok(0)
    }

    async fn count_sms_by_phone_today(&self, _phone: &str) -> StoreResult<i64> {
        Ok(0)
    }
}

#[async_trait]
impl SettingsStore for MockIngjooDb {
    async fn get_setting(&self, key: &str) -> StoreResult<Option<String>> {
        let settings = self.settings.lock().await;
        Ok(settings.get(key).cloned())
    }

    async fn get_all_settings(&self) -> StoreResult<HashMap<String, String>> {
        let settings = self.settings.lock().await;
        Ok(settings.clone())
    }

    async fn set_setting(&self, key: &str, value: &str) -> StoreResult<()> {
        self.settings.lock().await.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn set_settings(&self, pairs: &[(String, String)]) -> StoreResult<()> {
        let mut settings = self.settings.lock().await;
        for (k, v) in pairs {
            settings.insert(k.clone(), v.clone());
        }
        Ok(())
    }

    async fn seed_settings(&self, pairs: &[(String, String)]) -> StoreResult<()> {
        self.set_settings(pairs).await
    }
}

#[async_trait]
impl PreferenceStore for MockIngjooDb {
    async fn get_user_preferences(&self, user_id: &UserId) -> StoreResult<UserPreferences> {
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
    ) -> StoreResult<UserPreferences> {
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
impl AttachmentStore for MockIngjooDb {
    async fn create_attachment(
        &self,
        id: &str,
        user_id: &UserId,
        input: &CreateAttachment,
        storage_path: &str,
    ) -> StoreResult<Attachment> {
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
        self.attachments.lock().await.insert(id.to_string(), attachment.clone());
        Ok(attachment)
    }

    async fn list_attachments(
        &self,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> StoreResult<PaginatedResult<Attachment>> {
        use ingjoo_core::PaginatedResult;
        let attachments = self.attachments.lock().await;
        let mut list: Vec<Attachment> = attachments
            .values()
            .filter(|a| {
                entity_type.is_none_or(|t| a.entity_type.as_deref() == Some(t))
                    && entity_id.is_none_or(|t| a.entity_id.as_deref() == Some(t))
            })
            .cloned()
            .collect();
        let total = list.len() as i64;
        list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let items = list.into_iter().skip(offset as usize).take(limit as usize).collect();
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    async fn get_attachment(&self, id: &str) -> StoreResult<Option<Attachment>> {
        let attachments = self.attachments.lock().await;
        Ok(attachments.get(id).cloned())
    }

    async fn delete_attachment(&self, id: &str) -> StoreResult<bool> {
        Ok(self.attachments.lock().await.remove(id).is_some())
    }

    async fn list_all_attachments(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<Attachment>> {
        use ingjoo_core::PaginatedResult;
        let attachments = self.attachments.lock().await;
        let total = attachments.len() as i64;
        let mut list: Vec<Attachment> = attachments.values().cloned().collect();
        list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let items = list.into_iter().skip(offset as usize).take(limit as usize).collect();
        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    async fn update_attachment_storage(&self, id: &str, storage_path: &str, storage_type: &str) -> StoreResult<bool> {
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
impl ModuleSettingStore for MockIngjooDb {
    async fn list_module_settings(
        &self,
        scope: &str,
        scope_id: Option<&str>,
        module: Option<&str>,
    ) -> StoreResult<Vec<ModuleSetting>> {
        let settings = self.module_settings.lock().await;
        let list: Vec<ModuleSetting> = settings
            .values()
            .filter(|s| {
                s.scope == scope
                    && scope_id.is_none_or(|v| s.scope_id.as_deref() == Some(v))
                    && module.is_none_or(|v| s.module == v)
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
    ) -> StoreResult<Option<String>> {
        let settings = self.module_settings.lock().await;
        Ok(settings
            .values()
            .find(|s| {
                s.scope == scope
                    && scope_id.is_none_or(|v| s.scope_id.as_deref() == Some(v))
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
    ) -> StoreResult<ModuleSetting> {
        let setting = ModuleSetting {
            id: id.to_string(),
            scope: scope.to_string(),
            scope_id: scope_id.map(|s| s.to_string()),
            module: module.to_string(),
            key: key.to_string(),
            value: value.to_string(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        self.module_settings.lock().await.insert(id.to_string(), setting.clone());
        Ok(setting)
    }

    async fn delete_module_setting(&self, id: &str) -> StoreResult<bool> {
        Ok(self.module_settings.lock().await.remove(id).is_some())
    }

    async fn get_effective_setting(
        &self,
        module: &str,
        key: &str,
        collection_id: Option<&str>,
    ) -> StoreResult<Option<String>> {
        if let Some(cid) = collection_id {
            let collection_val = self.get_module_setting("collection", Some(cid), module, key).await?;
            if collection_val.is_some() {
                return Ok(collection_val);
            }
        }
        self.get_module_setting("global", None, module, key).await
    }
}

#[async_trait]
impl GroupStore for MockIngjooDb {
    async fn create_group(&self, group: &Group) -> StoreResult<Group> {
        let mut groups = self.groups.lock().await;
        if groups.contains_key(&group.id.0) {
            return Err(StoreError::UniqueViolation { table: "groups".to_string(), column: "id".to_string() });
        }
        let g = group.clone();
        groups.insert(group.id.0.clone(), g.clone());
        Ok(g)
    }

    async fn get_group(&self, id: &GroupId) -> StoreResult<Option<Group>> {
        let groups = self.groups.lock().await;
        Ok(groups.get(&id.0).cloned())
    }

    async fn get_group_by_name(&self, name: &str) -> StoreResult<Option<Group>> {
        let groups = self.groups.lock().await;
        Ok(groups.values().find(|g| g.name == name).cloned())
    }

    async fn list_groups(&self) -> StoreResult<Vec<Group>> {
        let groups = self.groups.lock().await;
        let mut result: Vec<Group> = groups.values().cloned().collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    async fn update_group(
        &self,
        id: &GroupId,
        display_name: Option<&str>,
        comment: Option<&str>,
    ) -> StoreResult<Option<Group>> {
        let mut groups = self.groups.lock().await;
        if let Some(g) = groups.get_mut(&id.0) {
            if let Some(dn) = display_name {
                g.display_name = Some(dn.to_string());
            }
            if let Some(c) = comment {
                g.comment = Some(c.to_string());
            }
            Ok(Some(g.clone()))
        } else {
            Ok(None)
        }
    }

    async fn delete_group(&self, id: &GroupId) -> StoreResult<bool> {
        let mut groups = self.groups.lock().await;
        Ok(groups.remove(&id.0).is_some())
    }

    async fn set_implied_groups(&self, group_id: &GroupId, implied_ids: &[GroupId]) -> StoreResult<()> {
        let mut implied = self.group_implied.lock().await;
        implied.retain(|k, _| k.0 != group_id.0);
        for iid in implied_ids {
            implied.insert(
                (group_id.0.clone(), iid.0.clone()),
                GroupImplied { group_id: group_id.clone(), implied_group_id: iid.clone() },
            );
        }
        Ok(())
    }

    async fn get_implied_groups(&self, group_id: &GroupId) -> StoreResult<Vec<GroupImplied>> {
        let implied = self.group_implied.lock().await;
        Ok(implied.values().filter(|gi| gi.group_id == *group_id).cloned().collect())
    }

    async fn add_user_to_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()> {
        let mut ug = self.user_groups.lock().await;
        ug.entry(user_id.0.clone()).or_insert_with(HashSet::new).insert(group_id.0.clone());
        Ok(())
    }

    async fn remove_user_from_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()> {
        let mut ug = self.user_groups.lock().await;
        if let Some(set) = ug.get_mut(&user_id.0) {
            set.remove(&group_id.0);
        }
        Ok(())
    }

    async fn get_user_groups(&self, user_id: &UserId) -> StoreResult<Vec<Group>> {
        let ug = self.user_groups.lock().await;
        let groups = self.groups.lock().await;
        let mut result = Vec::new();
        if let Some(gids) = ug.get(&user_id.0) {
            for gid in gids {
                if let Some(g) = groups.get(gid) {
                    result.push(g.clone());
                }
            }
        }
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    async fn resolve_all_groups(&self, user_id: &UserId) -> StoreResult<Vec<String>> {
        let direct = self.get_user_groups(user_id).await?;
        let mut names = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        for g in &direct {
            names.insert(g.name.clone());
            queue.push_back(g.id.clone());
        }
        while let Some(gid) = queue.pop_front() {
            let implied = self.get_implied_groups(&gid).await?;
            for imp in implied {
                if let Some(grp) = self.get_group(&imp.implied_group_id).await? {
                    if names.insert(grp.name.clone()) {
                        queue.push_back(grp.id.clone());
                    }
                }
            }
        }
        let mut sorted: Vec<String> = names.into_iter().collect();
        sorted.sort();
        Ok(sorted)
    }

    async fn set_user_groups(&self, user_id: &UserId, group_ids: &[GroupId]) -> StoreResult<()> {
        let mut ug = self.user_groups.lock().await;
        let set: HashSet<String> = group_ids.iter().map(|g| g.0.clone()).collect();
        ug.insert(user_id.0.clone(), set);
        Ok(())
    }
}

#[async_trait]
impl AccessStore for MockIngjooDb {
    async fn create_model_access(&self, access: &ModelAccessRow) -> StoreResult<ModelAccessRow> {
        let mut ma = self.model_accesses.lock().await;
        let a = access.clone();
        ma.insert(access.id.clone(), a.clone());
        Ok(a)
    }

    async fn get_model_access(&self, id: &str) -> StoreResult<Option<ModelAccessRow>> {
        let ma = self.model_accesses.lock().await;
        Ok(ma.get(id).cloned())
    }

    async fn list_model_accesses(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<ModelAccessRow>> {
        let ma = self.model_accesses.lock().await;
        let result: Vec<ModelAccessRow> =
            ma.values().filter(|a| group_id.is_none_or(|gid| a.group_id == *gid)).cloned().collect();
        Ok(result)
    }

    async fn update_model_access(&self, id: &str, access: &ModelAccessRow) -> StoreResult<Option<ModelAccessRow>> {
        let mut ma = self.model_accesses.lock().await;
        let a = access.clone();
        ma.insert(id.to_string(), a.clone());
        Ok(Some(a))
    }

    async fn delete_model_access(&self, id: &str) -> StoreResult<bool> {
        let mut ma = self.model_accesses.lock().await;
        Ok(ma.remove(id).is_some())
    }

    async fn create_record_rule(&self, rule: &RecordRuleRow) -> StoreResult<RecordRuleRow> {
        let mut rr = self.record_rules.lock().await;
        let r = rule.clone();
        rr.insert(rule.id.clone(), r.clone());
        Ok(r)
    }

    async fn get_record_rule(&self, id: &str) -> StoreResult<Option<RecordRuleRow>> {
        let rr = self.record_rules.lock().await;
        Ok(rr.get(id).cloned())
    }

    async fn list_record_rules(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<RecordRuleRow>> {
        let rr = self.record_rules.lock().await;
        let result: Vec<RecordRuleRow> =
            rr.values().filter(|r| group_id.is_none_or(|gid| r.group_id == *gid)).cloned().collect();
        Ok(result)
    }

    async fn update_record_rule(&self, id: &str, rule: &RecordRuleRow) -> StoreResult<Option<RecordRuleRow>> {
        let mut rr = self.record_rules.lock().await;
        let r = rule.clone();
        rr.insert(id.to_string(), r.clone());
        Ok(Some(r))
    }

    async fn delete_record_rule(&self, id: &str) -> StoreResult<bool> {
        let mut rr = self.record_rules.lock().await;
        Ok(rr.remove(id).is_some())
    }

    async fn get_model_accesses_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<ModelAccessRow>> {
        let ma = self.model_accesses.lock().await;
        let groups = self.groups.lock().await;
        let gids: HashSet<String> =
            groups.values().filter(|g| group_names.contains(&g.name)).map(|g| g.id.0.clone()).collect();
        let result: Vec<ModelAccessRow> = ma.values().filter(|a| gids.contains(&a.group_id.0)).cloned().collect();
        Ok(result)
    }

    async fn get_record_rules_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<RecordRuleRow>> {
        let rr = self.record_rules.lock().await;
        let groups = self.groups.lock().await;
        let gids: HashSet<String> =
            groups.values().filter(|g| group_names.contains(&g.name)).map(|g| g.id.0.clone()).collect();
        let result: Vec<RecordRuleRow> = rr.values().filter(|r| gids.contains(&r.group_id.0)).cloned().collect();
        Ok(result)
    }
}

#[async_trait]
impl AuditStore for MockIngjooDb {
    async fn create_audit_log(
        &self,
        user_id: Option<&str>,
        action: &str,
        resource: &str,
        resource_id: Option<&str>,
        detail: Option<serde_json::Value>,
        ip: Option<&str>,
    ) -> Result<AuditEntry, anyhow::Error> {
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.map(|s| s.to_string()),
            action: action.to_string(),
            resource: resource.to_string(),
            resource_id: resource_id.map(|s| s.to_string()),
            detail,
            ip: ip.map(|s| s.to_string()),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.audit_logs.lock().await.push(entry.clone());
        Ok(entry)
    }

    async fn list_audit_logs(&self, query: AuditQuery) -> Result<Vec<AuditEntry>, anyhow::Error> {
        let logs = self.audit_logs.lock().await;
        let mut result: Vec<AuditEntry> = logs
            .iter()
            .filter(|e| {
                query.user_id.as_ref().is_none_or(|v| e.user_id.as_ref() == Some(v))
                    && query.action.as_ref().is_none_or(|v| &e.action == v)
                    && query.resource.as_ref().is_none_or(|v| &e.resource == v)
                    && query.resource_id.as_ref().is_none_or(|v| e.resource_id.as_ref() == Some(v))
                    && query.ip.as_ref().is_none_or(|v| e.ip.as_ref() == Some(v))
            })
            .cloned()
            .collect();
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let offset = query.offset.unwrap_or(0) as usize;
        let limit = query.limit.unwrap_or(100) as usize;
        let items: Vec<AuditEntry> = result.into_iter().skip(offset).take(limit).collect();
        Ok(items)
    }

    async fn get_audit_log(&self, id: &str) -> Result<Option<AuditEntry>, anyhow::Error> {
        let logs = self.audit_logs.lock().await;
        Ok(logs.iter().find(|e| e.id == id).cloned())
    }

    async fn delete_logs_before(&self, before: &str) -> Result<u64, anyhow::Error> {
        let mut logs = self.audit_logs.lock().await;
        let before_count = logs.len();
        logs.retain(|e| e.created_at.as_str() >= before);
        Ok((before_count - logs.len()) as u64)
    }
}

#[async_trait]
impl NotificationStore for MockIngjooDb {
    async fn create_message(
        &self,
        author_id: Option<&str>,
        subject: &str,
        body: &str,
        message_type: &str,
    ) -> Result<MailMessage, anyhow::Error> {
        let msg = MailMessage {
            id: uuid::Uuid::new_v4().to_string(),
            author_id: author_id.map(|s| s.to_string()),
            subject: subject.to_string(),
            body: body.to_string(),
            message_type: message_type.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.mail_messages.lock().await.push(msg.clone());
        Ok(msg)
    }

    async fn create_notification(&self, message_id: &str, user_id: &str) -> Result<MailNotification, anyhow::Error> {
        let n = MailNotification {
            id: uuid::Uuid::new_v4().to_string(),
            message_id: message_id.to_string(),
            user_id: user_id.to_string(),
            is_read: false,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.mail_notifications.lock().await.push(n.clone());
        Ok(n)
    }

    async fn list_notifications(
        &self,
        user_id: &str,
        unread_only: bool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<NotificationItem>, anyhow::Error> {
        let notifs = self.mail_notifications.lock().await;
        let messages = self.mail_messages.lock().await;
        let mut items: Vec<NotificationItem> = notifs
            .iter()
            .filter(|n| n.user_id == user_id && (!unread_only || !n.is_read))
            .filter_map(|n| {
                messages
                    .iter()
                    .find(|m| m.id == n.message_id)
                    .map(|m| NotificationItem { notification: n.clone(), message: m.clone() })
            })
            .collect();
        items.sort_by(|a, b| b.notification.created_at.cmp(&a.notification.created_at));
        let result: Vec<NotificationItem> = items.into_iter().skip(offset as usize).take(limit as usize).collect();
        Ok(result)
    }

    async fn get_unread_count(&self, user_id: &str) -> Result<i64, anyhow::Error> {
        let notifs = self.mail_notifications.lock().await;
        Ok(notifs.iter().filter(|n| n.user_id == user_id && !n.is_read).count() as i64)
    }

    async fn mark_read(&self, notification_id: &str, user_id: &str) -> Result<bool, anyhow::Error> {
        let mut notifs = self.mail_notifications.lock().await;
        for n in notifs.iter_mut() {
            if n.id == notification_id && n.user_id == user_id && !n.is_read {
                n.is_read = true;
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn mark_all_read(&self, user_id: &str) -> Result<u64, anyhow::Error> {
        let mut notifs = self.mail_notifications.lock().await;
        let count = notifs.iter().filter(|n| n.user_id == user_id && !n.is_read).count();
        for n in notifs.iter_mut() {
            if n.user_id == user_id && !n.is_read {
                n.is_read = true;
            }
        }
        Ok(count as u64)
    }
}

#[async_trait]
impl super::traits::IngjooTransaction for MockIngjooDb {
    async fn run_in_transaction<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce() -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>> + Send,
        T: Send,
    {
        f().await
    }
}
