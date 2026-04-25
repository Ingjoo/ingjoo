use serde::{Deserialize, Serialize};

use super::ids::{GroupId, UserId};

/// 用户完整信息（含密码哈希等敏感字段，仅内部使用）
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: UserId,
    pub email: String,
    pub name: String,
    /// Argon2 密码哈希
    pub password_hash: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub role: String,
    /// OAuth 提供商名称（如 "google"、"github"）
    pub oauth_provider: Option<String>,
    /// OAuth 提供商侧的用户 ID
    pub oauth_id: Option<String>,
    pub phone: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 认证令牌（JWT access + refresh）
#[derive(Debug, Serialize)]
pub struct AuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserPublic,
}

/// 用户公开信息（脱敏后可返回前端）
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserPublic {
    pub id: UserId,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub role: String,
    pub phone: Option<String>,
}

impl From<&User> for UserPublic {
    fn from(u: &User) -> Self {
        Self {
            id: u.id.clone(),
            email: u.email.clone(),
            name: u.name.clone(),
            avatar_url: u.avatar_url.clone(),
            bio: u.bio.clone(),
            role: u.role.clone(),
            phone: u.phone.clone(),
        }
    }
}

/// 注册请求
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub name: String,
    pub password: String,
}

/// 登录请求
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub database: Option<String>,
}

/// 更新用户资料请求（所有字段可选，仅更新非 None 的字段）
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
}

/// 用户偏好设置（主题、编辑器行为、语言等）
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserPreferences {
    pub user_id: UserId,
    pub theme: String,
    pub inline_edit: i64,
    pub remember_pos: i64,
    pub line_numbers: i64,
    pub last_collection: Option<String>,
    pub last_entry: Option<String>,
    pub language: String,
    pub notification_channels: Option<String>,
    pub updated_at: String,
}

/// 更新用户偏好请求
#[derive(Debug, Deserialize)]
pub struct UpdatePreferences {
    pub theme: Option<String>,
    pub inline_edit: Option<bool>,
    pub remember_pos: Option<bool>,
    pub line_numbers: Option<bool>,
    pub last_collection: Option<String>,
    pub last_entry: Option<String>,
    pub language: Option<String>,
    pub notification_channels: Option<String>,
}

/// 文件附件
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Attachment {
    pub id: String,
    pub user_id: UserId,
    pub filename: String,
    pub mime_type: String,
    /// 文件字节数
    pub size: i64,
    pub storage_path: String,
    /// 关联实体类型（如 "document"、"entry"）
    pub entity_type: Option<String>,
    /// 关联实体 ID
    pub entity_id: Option<String>,
    /// 存储类型（"local"、"s3" 等）
    pub storage_type: String,
    pub created_at: String,
}

/// 创建附件请求
#[derive(Debug, Deserialize)]
pub struct CreateAttachment {
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
}

/// 模块配置项（支持级联：system → collection → document）
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModuleSetting {
    pub id: String,
    /// 配置作用域（"system"、"document_collection" 等）
    pub scope: String,
    /// 作用域内具体 ID（如集合 ID）
    pub scope_id: Option<String>,
    pub module: String,
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

/// 写入模块配置请求
#[derive(Debug, Deserialize)]
pub struct SetModuleSetting {
    pub scope: String,
    pub scope_id: Option<String>,
    pub module: String,
    pub key: String,
    pub value: String,
}

/// 用户组
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub display_name: Option<String>,
    pub comment: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 组间隐含关系（A 隐含 B，则拥有 A 组权限的用户自动拥有 B 组权限）
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GroupImplied {
    pub group_id: GroupId,
    pub implied_group_id: GroupId,
}

/// 用户-组关联
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserGroup {
    pub user_id: UserId,
    pub group_id: GroupId,
}

/// 创建或更新用户组请求
#[derive(Debug, Deserialize)]
pub struct UpsertGroupRequest {
    pub name: String,
    pub display_name: Option<String>,
    pub comment: Option<String>,
    pub implied_group_ids: Vec<GroupId>,
}

/// 模型级权限规则（Layer 1 — CRUD 矩阵）
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelAccessRow {
    pub id: String,
    pub group_id: GroupId,
    pub model: String,
    pub perm_read: bool,
    pub perm_write: bool,
    pub perm_create: bool,
    pub perm_delete: bool,
    pub perm_import: bool,
    pub perm_export: bool,
}

/// 写入模型级权限请求
#[derive(Debug, Deserialize)]
pub struct UpsertModelAccessRequest {
    pub group_id: GroupId,
    pub model: String,
    pub perm_read: bool,
    pub perm_write: bool,
    pub perm_create: bool,
    pub perm_delete: bool,
    #[serde(default)]
    pub perm_import: bool,
    #[serde(default)]
    pub perm_export: bool,
}

/// 记录级权限规则（Layer 2 — Domain 过滤自动注入 WHERE）
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecordRuleRow {
    pub id: String,
    pub name: String,
    pub group_id: GroupId,
    pub model: String,
    /// Domain DSL JSON 表达式
    pub domain: String,
    pub perm_read: bool,
    pub perm_write: bool,
    pub perm_create: bool,
    pub perm_delete: bool,
}

/// 审计日志
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: String,
    pub user_id: Option<String>,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub detail: Option<String>,
    pub ip: Option<String>,
    pub created_at: String,
}

/// 写入记录级权限规则请求
#[derive(Debug, Deserialize)]
pub struct UpsertRecordRuleRequest {
    pub name: String,
    pub group_id: GroupId,
    pub model: String,
    pub domain: String,
    pub perm_read: bool,
    pub perm_write: bool,
    pub perm_create: bool,
    pub perm_delete: bool,
}
