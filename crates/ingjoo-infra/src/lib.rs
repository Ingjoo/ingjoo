//! 莺竹框架基础设施层
//!
//! 提供数据库访问、认证、路由、中间件、扩展实现等开箱即用的基础设施。
//! 应用通过 [`AppState`] 组合各组件，由 [`base_router`] 挂载 HTTP 路由。

pub mod config;
pub mod extension_impl;
pub mod extension_noop;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod plugin;
pub mod router;
pub mod state;

#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "captcha")]
pub mod captcha;
#[cfg(feature = "db")]
pub mod db;
#[cfg(feature = "email")]
pub mod email;
#[cfg(feature = "sms")]
pub mod sms;
pub mod storage;

pub use ingjoo_core::PaginatedResult;

pub use extension_impl::InMemoryVectorStore;

#[cfg(feature = "auth")]
pub use auth::{AuthConfig, AuthProvider, AuthUtil, JwtAuthProvider, TokenClaims};
pub use config::IngjooConfig;
#[cfg(feature = "db")]
pub use db::database_manager::DatabaseManager;
#[cfg(feature = "db")]
pub use db::ids::UserId;
#[cfg(all(feature = "db", feature = "mock"))]
pub use db::mock::MockIngjooDb;
#[cfg(feature = "db")]
pub use db::models::{
    Attachment, AuthToken, CreateAttachment, LoginRequest, ModuleSetting, RegisterRequest, SetModuleSetting,
    UpdatePreferences, UpdateProfileRequest, User, UserPreferences, UserPublic,
};
#[cfg(feature = "db")]
pub use db::traits::{
    AttachmentStore, CaptchaStore, IngjooStore, IngjooTransaction, ModuleSettingStore, PreferenceStore, SettingsStore,
    SmsCodeStore, StoreError, StoreResult, TokenStore, UserStore,
};
#[cfg(feature = "db")]
pub use db::Db as IngjooDb;
#[cfg(feature = "email")]
pub use email::{EmailConfig, EmailProvider, SmtpEmailProvider};
pub use middleware::error::AppError;
pub use plugin::PluginManager;
#[cfg(feature = "sms")]
pub use sms::{SmsConfig, SmsProvider, TencentSmsProvider};
pub use state::AppState;
pub use state::RateLimitConfig;
#[cfg(feature = "s3")]
pub use storage::S3Storage;
pub use storage::{FileStorage, LocalStorage};
