pub mod config;
pub mod db;
pub mod dialect;
pub mod extension;
pub mod module;
pub mod pool;
pub mod query;
pub mod scope;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl<T> PaginatedResult<T> {
    pub fn new(items: Vec<T>, total: i64, limit: i64, offset: i64) -> Self {
        Self { items, total, limit, offset }
    }
}

pub use dialect::Dialect;
pub use query::domain::{Domain, DomainOp, DomainValue, SqlCondition};
pub use scope::{ScopeGuard, ScopeError, role_gte, SCOPE_HIERARCHY_4, SCOPE_HIERARCHY_3};
pub use config::cascade::CascadeConfig;
pub use module::{ModuleRoutes, RouteDescriptor, ModelRegistry, ModelDescriptor, FieldDescriptor, FieldType, IdType, ViewType, ActionType, MenuDescriptor, ViewDescriptor, ActionDescriptor};

pub use db::ids::UserId;
pub use db::ids::GroupId;
pub use db::error::{StoreError, StoreResult};
pub use db::models::{
    User, UserPublic, AuthToken, RegisterRequest, LoginRequest, UpdateProfileRequest,
    UserPreferences, UpdatePreferences, Attachment, CreateAttachment,
    ModuleSetting, SetModuleSetting,
    Group, GroupImplied, UserGroup, UpsertGroupRequest,
    ModelAccessRow, UpsertModelAccessRequest,
    RecordRuleRow, UpsertRecordRuleRequest,
};
pub use db::traits::{
    UserStore, TokenStore, CaptchaStore, SmsCodeStore, SettingsStore,
    PreferenceStore, AttachmentStore, ModuleSettingStore, GroupStore, AccessStore,
    IngjooStore, IngjooTransaction,
};
