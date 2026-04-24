pub mod config;
pub mod extension_noop;
pub mod extractors;
pub mod handlers;
pub mod middleware;
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

#[cfg(feature = "db")]
pub use db::models::{User, UserPublic, AuthToken, RegisterRequest, LoginRequest, UpdateProfileRequest, UserPreferences, UpdatePreferences, Attachment, CreateAttachment, ModuleSetting, SetModuleSetting};
#[cfg(feature = "db")]
pub use db::ids::UserId;
#[cfg(feature = "db")]
pub use db::traits::{StoreError, StoreResult, UserStore, TokenStore, CaptchaStore, SmsCodeStore, SettingsStore, PreferenceStore, AttachmentStore, ModuleSettingStore, IngjooStore, IngjooTransaction};
#[cfg(feature = "db")]
pub use db::Db as IngjooDb;
#[cfg(all(feature = "db", feature = "mock"))]
pub use db::mock::MockIngjooDb;
#[cfg(feature = "auth")]
pub use auth::{AuthConfig, AuthUtil, AuthProvider, JwtAuthProvider, TokenClaims};
pub use middleware::error::AppError;
#[cfg(feature = "sms")]
pub use sms::{SmsProvider, SmsConfig, TencentSmsProvider};
#[cfg(feature = "email")]
pub use email::{EmailProvider, SmtpEmailProvider, EmailConfig};
pub use config::IngjooConfig;
pub use state::AppState;
pub use storage::{FileStorage, LocalStorage};
#[cfg(feature = "s3")]
pub use storage::S3Storage;
