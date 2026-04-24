use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;

use super::error::StoreResult;
use super::ids::{GroupId, UserId};
use crate::PaginatedResult;
use crate::extension::audit::AuditStore;
use super::models::{
    Attachment, CreateAttachment, Group, GroupImplied, ModelAccessRow, ModuleSetting,
    RecordRuleRow, UpdatePreferences, User, UserPreferences, UserPublic,
};

#[async_trait]
pub trait UserStore: Send + Sync {
    async fn create_user(&self, user: &User) -> StoreResult<User>;
    async fn get_user_by_email(&self, email: &str) -> StoreResult<Option<User>>;
    async fn get_user_by_id(&self, id: &UserId) -> StoreResult<Option<User>>;
    async fn get_user_by_oauth(&self, provider: &str, oauth_id: &str)
        -> StoreResult<Option<User>>;
    async fn get_user_by_phone(&self, phone: &str) -> StoreResult<Option<User>>;
    async fn update_user(
        &self,
        id: &UserId,
        name: Option<&str>,
        avatar_url: Option<&str>,
        bio: Option<&str>,
    ) -> StoreResult<Option<User>>;
    async fn update_user_role(&self, id: &UserId, role: &str) -> StoreResult<Option<User>>;
    async fn update_user_password(
        &self,
        id: &UserId,
        password_hash: &str,
    ) -> StoreResult<Option<User>>;
    async fn list_users(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<User>>;
    async fn search_users(&self, query: &str, limit: i64) -> StoreResult<Vec<UserPublic>>;
}

#[async_trait]
pub trait TokenStore: Send + Sync {
    async fn create_refresh_token(
        &self,
        id: &str,
        user_id: &UserId,
        token_hash: &str,
        expires_at: &str,
    ) -> StoreResult<()>;
    async fn get_refresh_token(&self, token_hash: &str) -> StoreResult<Option<(String, String)>>;
    async fn delete_refresh_token(&self, token_hash: &str) -> StoreResult<()>;
    async fn create_password_reset_token(
        &self,
        id: &str,
        user_id: &UserId,
        token: &str,
        expires_at: &str,
    ) -> StoreResult<()>;
    async fn get_password_reset_token(
        &self,
        token: &str,
    ) -> StoreResult<Option<(String, String, i64, String)>>;
    async fn mark_password_reset_used(&self, id: &str) -> StoreResult<()>;
}

#[async_trait]
pub trait CaptchaStore: Send + Sync {
    async fn create_captcha(&self, id: &str, answer: &str, expires_at: &str) -> StoreResult<()>;
    async fn get_captcha(&self, id: &str) -> StoreResult<Option<(String, i64, String)>>;
    async fn mark_captcha_used(&self, id: &str) -> StoreResult<()>;
}

#[async_trait]
pub trait SmsCodeStore: Send + Sync {
    async fn create_sms_code(
        &self,
        id: &str,
        phone: &str,
        code: &str,
        purpose: &str,
        ip_address: Option<&str>,
        expires_at: &str,
    ) -> StoreResult<()>;
    async fn get_latest_sms_code(
        &self,
        phone: &str,
        purpose: &str,
    ) -> StoreResult<Option<(String, String, i64, String)>>;
    async fn get_sms_code_sent_within(
        &self,
        phone: &str,
        purpose: &str,
        seconds: i64,
    ) -> StoreResult<bool>;
    async fn mark_sms_code_used(&self, id: &str) -> StoreResult<()>;
    async fn count_sms_by_ip(&self, ip: &str, hours: i64) -> StoreResult<i64>;
    async fn count_sms_by_phone_today(&self, phone: &str) -> StoreResult<i64>;
}

#[async_trait]
pub trait SettingsStore: Send + Sync {
    async fn get_setting(&self, key: &str) -> StoreResult<Option<String>>;
    async fn get_all_settings(&self) -> StoreResult<HashMap<String, String>>;
    async fn set_setting(&self, key: &str, value: &str) -> StoreResult<()>;
    async fn set_settings(&self, pairs: &[(String, String)]) -> StoreResult<()>;
    async fn seed_settings(&self, pairs: &[(String, String)]) -> StoreResult<()>;
}

#[async_trait]
pub trait PreferenceStore: Send + Sync {
    async fn get_user_preferences(&self, user_id: &UserId) -> StoreResult<UserPreferences>;
    async fn upsert_user_preferences(
        &self,
        user_id: &UserId,
        input: &UpdatePreferences,
    ) -> StoreResult<UserPreferences>;
}

#[async_trait]
pub trait AttachmentStore: Send + Sync {
    async fn create_attachment(
        &self,
        id: &str,
        user_id: &UserId,
        input: &CreateAttachment,
        storage_path: &str,
    ) -> StoreResult<Attachment>;
    async fn list_attachments(
        &self,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> StoreResult<PaginatedResult<Attachment>>;
    async fn get_attachment(&self, id: &str) -> StoreResult<Option<Attachment>>;
    async fn delete_attachment(&self, id: &str) -> StoreResult<bool>;
    async fn list_all_attachments(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<Attachment>>;
    async fn update_attachment_storage(
        &self,
        id: &str,
        storage_path: &str,
        storage_type: &str,
    ) -> StoreResult<bool>;
}

#[async_trait]
pub trait ModuleSettingStore: Send + Sync {
    async fn list_module_settings(
        &self,
        scope: &str,
        scope_id: Option<&str>,
        module: Option<&str>,
    ) -> StoreResult<Vec<ModuleSetting>>;
    async fn get_module_setting(
        &self,
        scope: &str,
        scope_id: Option<&str>,
        module: &str,
        key: &str,
    ) -> StoreResult<Option<String>>;
    async fn set_module_setting(
        &self,
        id: &str,
        scope: &str,
        scope_id: Option<&str>,
        module: &str,
        key: &str,
        value: &str,
    ) -> StoreResult<ModuleSetting>;
    async fn delete_module_setting(&self, id: &str) -> StoreResult<bool>;
    async fn get_effective_setting(
        &self,
        module: &str,
        key: &str,
        collection_id: Option<&str>,
    ) -> StoreResult<Option<String>>;
}

#[async_trait]
pub trait GroupStore: Send + Sync {
    async fn create_group(&self, group: &Group) -> StoreResult<Group>;
    async fn get_group(&self, id: &GroupId) -> StoreResult<Option<Group>>;
    async fn get_group_by_name(&self, name: &str) -> StoreResult<Option<Group>>;
    async fn list_groups(&self) -> StoreResult<Vec<Group>>;
    async fn update_group(&self, id: &GroupId, display_name: Option<&str>, comment: Option<&str>) -> StoreResult<Option<Group>>;
    async fn delete_group(&self, id: &GroupId) -> StoreResult<bool>;
    async fn set_implied_groups(&self, group_id: &GroupId, implied_ids: &[GroupId]) -> StoreResult<()>;
    async fn get_implied_groups(&self, group_id: &GroupId) -> StoreResult<Vec<GroupImplied>>;
    async fn add_user_to_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()>;
    async fn remove_user_from_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()>;
    async fn get_user_groups(&self, user_id: &UserId) -> StoreResult<Vec<Group>>;
    async fn resolve_all_groups(&self, user_id: &UserId) -> StoreResult<Vec<String>>;
    async fn set_user_groups(&self, user_id: &UserId, group_ids: &[GroupId]) -> StoreResult<()>;
}

#[async_trait]
pub trait AccessStore: Send + Sync {
    async fn create_model_access(&self, access: &ModelAccessRow) -> StoreResult<ModelAccessRow>;
    async fn get_model_access(&self, id: &str) -> StoreResult<Option<ModelAccessRow>>;
    async fn list_model_accesses(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<ModelAccessRow>>;
    async fn update_model_access(&self, id: &str, access: &ModelAccessRow) -> StoreResult<Option<ModelAccessRow>>;
    async fn delete_model_access(&self, id: &str) -> StoreResult<bool>;
    async fn create_record_rule(&self, rule: &RecordRuleRow) -> StoreResult<RecordRuleRow>;
    async fn get_record_rule(&self, id: &str) -> StoreResult<Option<RecordRuleRow>>;
    async fn list_record_rules(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<RecordRuleRow>>;
    async fn update_record_rule(&self, id: &str, rule: &RecordRuleRow) -> StoreResult<Option<RecordRuleRow>>;
    async fn delete_record_rule(&self, id: &str) -> StoreResult<bool>;
    async fn get_model_accesses_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<ModelAccessRow>>;
    async fn get_record_rules_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<RecordRuleRow>>;
}

#[async_trait]
pub trait IngjooStore: UserStore + TokenStore + CaptchaStore + SmsCodeStore
    + SettingsStore + PreferenceStore + AttachmentStore + ModuleSettingStore
    + GroupStore + AccessStore + AuditStore
    + Send + Sync
{
}

#[async_trait]
pub trait IngjooTransaction: IngjooStore {
    async fn run_in_transaction<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce() -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>> + Send,
        T: Send;
}

impl<T> IngjooStore for T
where
    T: UserStore + TokenStore + CaptchaStore + SmsCodeStore
        + SettingsStore + PreferenceStore + AttachmentStore + ModuleSettingStore
        + GroupStore + AccessStore + AuditStore
        + Send + Sync,
{
}
