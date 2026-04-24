//! 存储层 trait 定义 — 每个存储能力一个独立 trait，由 `IngjooStore` 聚合

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

/// 用户存储 — 用户 CRUD 和搜索
#[async_trait]
pub trait UserStore: Send + Sync {
    /// 创建用户
    async fn create_user(&self, user: &User) -> StoreResult<User>;
    /// 按邮箱查找用户
    async fn get_user_by_email(&self, email: &str) -> StoreResult<Option<User>>;
    /// 按 ID 查找用户
    async fn get_user_by_id(&self, id: &UserId) -> StoreResult<Option<User>>;
    /// 按 OAuth 提供商 + ID 查找用户
    async fn get_user_by_oauth(&self, provider: &str, oauth_id: &str)
        -> StoreResult<Option<User>>;
    /// 按手机号查找用户
    async fn get_user_by_phone(&self, phone: &str) -> StoreResult<Option<User>>;
    /// 更新用户资料（仅更新非 None 的字段）
    async fn update_user(
        &self,
        id: &UserId,
        name: Option<&str>,
        avatar_url: Option<&str>,
        bio: Option<&str>,
    ) -> StoreResult<Option<User>>;
    /// 更新用户角色
    async fn update_user_role(&self, id: &UserId, role: &str) -> StoreResult<Option<User>>;
    /// 更新用户密码哈希
    async fn update_user_password(
        &self,
        id: &UserId,
        password_hash: &str,
    ) -> StoreResult<Option<User>>;
    /// 分页列出用户
    async fn list_users(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<User>>;
    /// 按关键词搜索用户（返回公开信息）
    async fn search_users(&self, query: &str, limit: i64) -> StoreResult<Vec<UserPublic>>;
}

/// 令牌存储 — refresh token 和密码重置 token
#[async_trait]
pub trait TokenStore: Send + Sync {
    /// 存储 refresh token
    async fn create_refresh_token(
        &self,
        id: &str,
        user_id: &UserId,
        token_hash: &str,
        expires_at: &str,
    ) -> StoreResult<()>;
    /// 查找 refresh token，返回 (id, user_id)
    async fn get_refresh_token(&self, token_hash: &str) -> StoreResult<Option<(String, String)>>;
    /// 删除 refresh token（登出时调用）
    async fn delete_refresh_token(&self, token_hash: &str) -> StoreResult<()>;
    /// 创建密码重置 token
    async fn create_password_reset_token(
        &self,
        id: &str,
        user_id: &UserId,
        token: &str,
        expires_at: &str,
    ) -> StoreResult<()>;
    /// 查找密码重置 token，返回 (id, user_id, used, expires_at)
    async fn get_password_reset_token(
        &self,
        token: &str,
    ) -> StoreResult<Option<(String, String, i64, String)>>;
    /// 标记密码重置 token 已使用
    async fn mark_password_reset_used(&self, id: &str) -> StoreResult<()>;
}

/// 验证码存储 — 图形验证码
#[async_trait]
pub trait CaptchaStore: Send + Sync {
    /// 创建验证码
    async fn create_captcha(&self, id: &str, answer: &str, expires_at: &str) -> StoreResult<()>;
    /// 查找验证码，返回 (answer, used, expires_at)
    async fn get_captcha(&self, id: &str) -> StoreResult<Option<(String, i64, String)>>;
    /// 标记验证码已使用
    async fn mark_captcha_used(&self, id: &str) -> StoreResult<()>;
}

/// 短信验证码存储
#[async_trait]
pub trait SmsCodeStore: Send + Sync {
    /// 创建短信验证码
    async fn create_sms_code(
        &self,
        id: &str,
        phone: &str,
        code: &str,
        purpose: &str,
        ip_address: Option<&str>,
        expires_at: &str,
    ) -> StoreResult<()>;
    /// 获取最新的短信验证码，返回 (id, code, used, expires_at)
    async fn get_latest_sms_code(
        &self,
        phone: &str,
        purpose: &str,
    ) -> StoreResult<Option<(String, String, i64, String)>>;
    /// 检查指定秒数内是否已发送过验证码
    async fn get_sms_code_sent_within(
        &self,
        phone: &str,
        purpose: &str,
        seconds: i64,
    ) -> StoreResult<bool>;
    /// 标记短信验证码已使用
    async fn mark_sms_code_used(&self, id: &str) -> StoreResult<()>;
    /// 统计指定 IP 在指定小时数内发送的短信数量
    async fn count_sms_by_ip(&self, ip: &str, hours: i64) -> StoreResult<i64>;
    /// 统计指定手机号今日发送数量
    async fn count_sms_by_phone_today(&self, phone: &str) -> StoreResult<i64>;
}

/// 系统设置存储 — 全局键值对
#[async_trait]
pub trait SettingsStore: Send + Sync {
    /// 获取单个设置值
    async fn get_setting(&self, key: &str) -> StoreResult<Option<String>>;
    /// 获取所有设置
    async fn get_all_settings(&self) -> StoreResult<HashMap<String, String>>;
    /// 设置单个键值
    async fn set_setting(&self, key: &str, value: &str) -> StoreResult<()>;
    /// 批量设置
    async fn set_settings(&self, pairs: &[(String, String)]) -> StoreResult<()>;
    /// 种子设置（仅在键不存在时写入）
    async fn seed_settings(&self, pairs: &[(String, String)]) -> StoreResult<()>;
}

/// 用户偏好存储
#[async_trait]
pub trait PreferenceStore: Send + Sync {
    /// 获取用户偏好（不存在则返回默认值）
    async fn get_user_preferences(&self, user_id: &UserId) -> StoreResult<UserPreferences>;
    /// 更新用户偏好（upsert）
    async fn upsert_user_preferences(
        &self,
        user_id: &UserId,
        input: &UpdatePreferences,
    ) -> StoreResult<UserPreferences>;
}

/// 附件存储
#[async_trait]
pub trait AttachmentStore: Send + Sync {
    /// 创建附件记录
    async fn create_attachment(
        &self,
        id: &str,
        user_id: &UserId,
        input: &CreateAttachment,
        storage_path: &str,
    ) -> StoreResult<Attachment>;
    /// 按关联实体分页列出附件
    async fn list_attachments(
        &self,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> StoreResult<PaginatedResult<Attachment>>;
    /// 按 ID 获取附件
    async fn get_attachment(&self, id: &str) -> StoreResult<Option<Attachment>>;
    /// 删除附件，返回是否实际删除
    async fn delete_attachment(&self, id: &str) -> StoreResult<bool>;
    /// 分页列出所有附件
    async fn list_all_attachments(&self, limit: i64, offset: i64) -> StoreResult<PaginatedResult<Attachment>>;
    /// 更新附件存储路径和类型（迁移时使用）
    async fn update_attachment_storage(
        &self,
        id: &str,
        storage_path: &str,
        storage_type: &str,
    ) -> StoreResult<bool>;
}

/// 模块配置存储 — 支持级联作用域
#[async_trait]
pub trait ModuleSettingStore: Send + Sync {
    /// 列出模块配置项
    async fn list_module_settings(
        &self,
        scope: &str,
        scope_id: Option<&str>,
        module: Option<&str>,
    ) -> StoreResult<Vec<ModuleSetting>>;
    /// 获取单个模块配置值
    async fn get_module_setting(
        &self,
        scope: &str,
        scope_id: Option<&str>,
        module: &str,
        key: &str,
    ) -> StoreResult<Option<String>>;
    /// 写入模块配置（upsert）
    async fn set_module_setting(
        &self,
        id: &str,
        scope: &str,
        scope_id: Option<&str>,
        module: &str,
        key: &str,
        value: &str,
    ) -> StoreResult<ModuleSetting>;
    /// 删除模块配置项
    async fn delete_module_setting(&self, id: &str) -> StoreResult<bool>;
    /// 获取级联合并后的生效配置值（collection → system）
    async fn get_effective_setting(
        &self,
        module: &str,
        key: &str,
        collection_id: Option<&str>,
    ) -> StoreResult<Option<String>>;
}

/// 用户组存储 — 组 CRUD、隐含关系、用户-组关联
#[async_trait]
pub trait GroupStore: Send + Sync {
    /// 创建用户组
    async fn create_group(&self, group: &Group) -> StoreResult<Group>;
    /// 按 ID 获取组
    async fn get_group(&self, id: &GroupId) -> StoreResult<Option<Group>>;
    /// 按名称查找组
    async fn get_group_by_name(&self, name: &str) -> StoreResult<Option<Group>>;
    /// 列出所有组
    async fn list_groups(&self) -> StoreResult<Vec<Group>>;
    /// 更新组信息
    async fn update_group(&self, id: &GroupId, display_name: Option<&str>, comment: Option<&str>) -> StoreResult<Option<Group>>;
    /// 删除组
    async fn delete_group(&self, id: &GroupId) -> StoreResult<bool>;
    /// 设置组的隐含关系（全量替换）
    async fn set_implied_groups(&self, group_id: &GroupId, implied_ids: &[GroupId]) -> StoreResult<()>;
    /// 获取组的直接隐含关系
    async fn get_implied_groups(&self, group_id: &GroupId) -> StoreResult<Vec<GroupImplied>>;
    /// 将用户加入组
    async fn add_user_to_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()>;
    /// 将用户移出组
    async fn remove_user_from_group(&self, user_id: &UserId, group_id: &GroupId) -> StoreResult<()>;
    /// 获取用户直接所属的组
    async fn get_user_groups(&self, user_id: &UserId) -> StoreResult<Vec<Group>>;
    /// 解析用户所有组（含递归隐含），返回组名列表
    async fn resolve_all_groups(&self, user_id: &UserId) -> StoreResult<Vec<String>>;
    /// 设置用户所属组（全量替换）
    async fn set_user_groups(&self, user_id: &UserId, group_ids: &[GroupId]) -> StoreResult<()>;
}

/// 权限存储 — 模型级权限（Layer 1）和记录级规则（Layer 2）
#[async_trait]
pub trait AccessStore: Send + Sync {
    /// 创建模型级权限规则
    async fn create_model_access(&self, access: &ModelAccessRow) -> StoreResult<ModelAccessRow>;
    /// 按 ID 获取模型级权限
    async fn get_model_access(&self, id: &str) -> StoreResult<Option<ModelAccessRow>>;
    /// 列出模型级权限，可按组筛选
    async fn list_model_accesses(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<ModelAccessRow>>;
    /// 更新模型级权限
    async fn update_model_access(&self, id: &str, access: &ModelAccessRow) -> StoreResult<Option<ModelAccessRow>>;
    /// 删除模型级权限
    async fn delete_model_access(&self, id: &str) -> StoreResult<bool>;
    /// 创建记录级规则
    async fn create_record_rule(&self, rule: &RecordRuleRow) -> StoreResult<RecordRuleRow>;
    /// 按 ID 获取记录级规则
    async fn get_record_rule(&self, id: &str) -> StoreResult<Option<RecordRuleRow>>;
    /// 列出记录级规则，可按组筛选
    async fn list_record_rules(&self, group_id: Option<&GroupId>) -> StoreResult<Vec<RecordRuleRow>>;
    /// 更新记录级规则
    async fn update_record_rule(&self, id: &str, rule: &RecordRuleRow) -> StoreResult<Option<RecordRuleRow>>;
    /// 删除记录级规则
    async fn delete_record_rule(&self, id: &str) -> StoreResult<bool>;
    /// 批量获取指定组列表的模型级权限
    async fn get_model_accesses_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<ModelAccessRow>>;
    /// 批量获取指定组列表的记录级规则
    async fn get_record_rules_for_groups(&self, group_names: &[String]) -> StoreResult<Vec<RecordRuleRow>>;
}

/// 聚合存储 trait — 组合所有存储能力，以 `Arc<dyn IngjooStore>` 形式使用
#[async_trait]
pub trait IngjooStore: UserStore + TokenStore + CaptchaStore + SmsCodeStore
    + SettingsStore + PreferenceStore + AttachmentStore + ModuleSettingStore
    + GroupStore + AccessStore + AuditStore
    + Send + Sync
{
}

/// 事务支持 — 在事务内执行闭包
#[async_trait]
pub trait IngjooTransaction: IngjooStore {
    /// 在数据库事务中执行异步闭包
    async fn run_in_transaction<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce() -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>> + Send,
        T: Send;
}

/// 自动为满足所有约束的类型实现 `IngjooStore`
impl<T> IngjooStore for T
where
    T: UserStore + TokenStore + CaptchaStore + SmsCodeStore
        + SettingsStore + PreferenceStore + AttachmentStore + ModuleSettingStore
        + GroupStore + AccessStore + AuditStore
        + Send + Sync,
{
}
