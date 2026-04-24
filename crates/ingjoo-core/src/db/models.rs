use serde::{Deserialize, Serialize};

use super::ids::{GroupId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: UserId,
    pub email: String,
    pub name: String,
    pub password_hash: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub role: String,
    pub oauth_provider: Option<String>,
    pub oauth_id: Option<String>,
    pub phone: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct AuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserPublic,
}

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

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Attachment {
    pub id: String,
    pub user_id: UserId,
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    pub storage_path: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub storage_type: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAttachment {
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModuleSetting {
    pub id: String,
    pub scope: String,
    pub scope_id: Option<String>,
    pub module: String,
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SetModuleSetting {
    pub scope: String,
    pub scope_id: Option<String>,
    pub module: String,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub display_name: Option<String>,
    pub comment: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GroupImplied {
    pub group_id: GroupId,
    pub implied_group_id: GroupId,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserGroup {
    pub user_id: UserId,
    pub group_id: GroupId,
}

#[derive(Debug, Deserialize)]
pub struct UpsertGroupRequest {
    pub name: String,
    pub display_name: Option<String>,
    pub comment: Option<String>,
    pub implied_group_ids: Vec<GroupId>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecordRuleRow {
    pub id: String,
    pub name: String,
    pub group_id: GroupId,
    pub model: String,
    pub domain: String,
    pub perm_read: bool,
    pub perm_write: bool,
    pub perm_create: bool,
    pub perm_delete: bool,
}

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
