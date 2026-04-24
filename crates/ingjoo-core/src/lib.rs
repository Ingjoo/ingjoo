pub mod config;
pub mod db;
pub mod dialect;
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
pub use module::{ModuleRoutes, RouteDescriptor};

pub use db::ids::UserId;
pub use db::models::{
    User, UserPublic, AuthToken, RegisterRequest, LoginRequest, UpdateProfileRequest,
    UserPreferences, UpdatePreferences, Attachment, CreateAttachment,
    ModuleSetting, SetModuleSetting,
};
pub use db::traits::{
    UserStore, TokenStore, CaptchaStore, SmsCodeStore, SettingsStore,
    PreferenceStore, AttachmentStore, ModuleSettingStore, ScaffStore, ScaffTransaction,
};
