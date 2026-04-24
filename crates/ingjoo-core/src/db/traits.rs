use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;

use super::ids::UserId;
use crate::PaginatedResult;
use super::models::{
    Attachment, CreateAttachment, ModuleSetting, UpdatePreferences, User, UserPreferences, UserPublic,
};

#[async_trait]
pub trait UserStore: Send + Sync {
    async fn create_user(&self, user: &User) -> Result<User>;
    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>>;
    async fn get_user_by_id(&self, id: &UserId) -> Result<Option<User>>;
    async fn get_user_by_oauth(&self, provider: &str, oauth_id: &str)
        -> Result<Option<User>>;
    async fn get_user_by_phone(&self, phone: &str) -> Result<Option<User>>;
    async fn update_user(
        &self,
        id: &UserId,
        name: Option<&str>,
        avatar_url: Option<&str>,
        bio: Option<&str>,
    ) -> Result<Option<User>>;
    async fn update_user_role(&self, id: &UserId, role: &str) -> Result<Option<User>>;
    async fn update_user_password(
        &self,
        id: &UserId,
        password_hash: &str,
    ) -> Result<Option<User>>;
    async fn list_users(&self, limit: i64, offset: i64) -> Result<PaginatedResult<User>>;
    async fn search_users(&self, query: &str, limit: i64) -> Result<Vec<UserPublic>>;
}

#[async_trait]
pub trait TokenStore: Send + Sync {
    async fn create_refresh_token(
        &self,
        id: &str,
        user_id: &UserId,
        token_hash: &str,
        expires_at: &str,
    ) -> Result<()>;
    async fn get_refresh_token(&self, token_hash: &str) -> Result<Option<(String, String)>>;
    async fn delete_refresh_token(&self, token_hash: &str) -> Result<()>;
    async fn create_password_reset_token(
        &self,
        id: &str,
        user_id: &UserId,
        token: &str,
        expires_at: &str,
    ) -> Result<()>;
    async fn get_password_reset_token(
        &self,
        token: &str,
    ) -> Result<Option<(String, String, i64, String)>>;
    async fn mark_password_reset_used(&self, id: &str) -> Result<()>;
}

#[async_trait]
pub trait CaptchaStore: Send + Sync {
    async fn create_captcha(&self, id: &str, answer: &str, expires_at: &str) -> Result<()>;
    async fn get_captcha(&self, id: &str) -> Result<Option<(String, i64, String)>>;
    async fn mark_captcha_used(&self, id: &str) -> Result<()>;
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
    ) -> Result<()>;
    async fn get_latest_sms_code(
        &self,
        phone: &str,
        purpose: &str,
    ) -> Result<Option<(String, String, i64, String)>>;
    async fn get_sms_code_sent_within(
        &self,
        phone: &str,
        purpose: &str,
        seconds: i64,
    ) -> Result<bool>;
    async fn mark_sms_code_used(&self, id: &str) -> Result<()>;
    async fn count_sms_by_ip(&self, ip: &str, hours: i64) -> Result<i64>;
    async fn count_sms_by_phone_today(&self, phone: &str) -> Result<i64>;
}

#[async_trait]
pub trait SettingsStore: Send + Sync {
    async fn get_setting(&self, key: &str) -> Result<Option<String>>;
    async fn get_all_settings(&self) -> Result<HashMap<String, String>>;
    async fn set_setting(&self, key: &str, value: &str) -> Result<()>;
    async fn set_settings(&self, pairs: &[(String, String)]) -> Result<()>;
    async fn seed_settings(&self, pairs: &[(String, String)]) -> Result<()>;
}

#[async_trait]
pub trait PreferenceStore: Send + Sync {
    async fn get_user_preferences(&self, user_id: &UserId) -> Result<UserPreferences>;
    async fn upsert_user_preferences(
        &self,
        user_id: &UserId,
        input: &UpdatePreferences,
    ) -> Result<UserPreferences>;
}

#[async_trait]
pub trait AttachmentStore: Send + Sync {
    async fn create_attachment(
        &self,
        id: &str,
        user_id: &UserId,
        input: &CreateAttachment,
        storage_path: &str,
    ) -> Result<Attachment>;
    async fn list_attachments(
        &self,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<PaginatedResult<Attachment>>;
    async fn get_attachment(&self, id: &str) -> Result<Option<Attachment>>;
    async fn delete_attachment(&self, id: &str) -> Result<bool>;
    async fn list_all_attachments(&self, limit: i64, offset: i64) -> Result<PaginatedResult<Attachment>>;
    async fn update_attachment_storage(
        &self,
        id: &str,
        storage_path: &str,
        storage_type: &str,
    ) -> Result<bool>;
}

#[async_trait]
pub trait ModuleSettingStore: Send + Sync {
    async fn list_module_settings(
        &self,
        scope: &str,
        scope_id: Option<&str>,
        module: Option<&str>,
    ) -> Result<Vec<ModuleSetting>>;
    async fn get_module_setting(
        &self,
        scope: &str,
        scope_id: Option<&str>,
        module: &str,
        key: &str,
    ) -> Result<Option<String>>;
    async fn set_module_setting(
        &self,
        id: &str,
        scope: &str,
        scope_id: Option<&str>,
        module: &str,
        key: &str,
        value: &str,
    ) -> Result<ModuleSetting>;
    async fn delete_module_setting(&self, id: &str) -> Result<bool>;
    async fn get_effective_setting(
        &self,
        module: &str,
        key: &str,
        collection_id: Option<&str>,
    ) -> Result<Option<String>>;
}

#[async_trait]
pub trait ScaffStore: UserStore + TokenStore + CaptchaStore + SmsCodeStore
    + SettingsStore + PreferenceStore + AttachmentStore + ModuleSettingStore
    + Send + Sync
{
}

#[async_trait]
pub trait ScaffTransaction: ScaffStore {
    async fn run_in_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>> + Send,
        T: Send;
}

impl<T> ScaffStore for T
where
    T: UserStore + TokenStore + CaptchaStore + SmsCodeStore
        + SettingsStore + PreferenceStore + AttachmentStore + ModuleSettingStore
        + Send + Sync,
{
}
