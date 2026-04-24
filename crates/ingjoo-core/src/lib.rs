//! # ingjoo-core
//!
//! 莺竹框架核心抽象层，定义了所有 trait、领域模型和类型别名。
//!
//! 本 crate 不包含任何具体实现，仅提供：
//! - **数据库存储 trait**（`UserStore`、`TokenStore` 等）和聚合 trait `IngjooStore`
//! - **领域模型**（`User`、`Group`、`ModelAccessRow` 等）
//! - **扩展 trait**（`StateMachine`、`EventBus`、`SearchEngine` 等）
//! - **Domain DSL** 查询语言（Odoo 风格声明式过滤表达式）
//! - **SQL 方言适配**（SQLite / PostgreSQL 双方言支持）
//! - **动态模型注册**（`ModelRegistry`、`ModelDescriptor`）
//! - **权限与作用域**（`ScopeGuard`、角色层级）

pub mod config;
pub mod db;
pub mod dialect;
pub mod extension;
pub mod module;
pub mod pool;
pub mod query;
pub mod scope;

use serde::{Deserialize, Serialize};

/// 分页查询结果，携带数据列表和分页元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    /// 当前页数据
    pub items: Vec<T>,
    /// 满足条件的总记录数
    pub total: i64,
    /// 每页条数
    pub limit: i64,
    /// 偏移量（跳过的记录数）
    pub offset: i64,
}

impl<T> PaginatedResult<T> {
    /// 创建分页结果
    pub fn new(items: Vec<T>, total: i64, limit: i64, offset: i64) -> Self {
        Self { items, total, limit, offset }
    }
}

pub use dialect::Dialect;
pub use query::domain::{Domain, DomainOp, DomainValue, SqlCondition};
pub use scope::{ScopeGuard, ScopeError, role_gte, SCOPE_HIERARCHY_4, SCOPE_HIERARCHY_3};
pub use config::cascade::CascadeConfig;
pub use module::{ModuleRoutes, RouteDescriptor, ModelRegistry, ModelDescriptor, FieldDescriptor, FieldType, IdType, ViewType, ActionType, MenuDescriptor, ViewDescriptor, ActionDescriptor, PluginManifest, PluginInfo, PluginState};

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
