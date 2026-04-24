use std::collections::HashMap;
use std::path::PathBuf;

/// 应用全局配置，从环境变量或 settings 表加载
pub struct IngjooConfig {
    /// JWT 签名密钥
    pub jwt_secret: String,
    /// 文件存储根目录
    pub storage_path: PathBuf,
    #[cfg(feature = "sms")]
    pub sms: Option<crate::sms::SmsConfig>,
    #[cfg(feature = "email")]
    pub email: Option<crate::email::EmailConfig>,
}

impl IngjooConfig {
    /// 从 settings 键值表构建配置
    #[allow(unused_variables)]
    pub fn from_settings(
        jwt_secret: String,
        storage_path: PathBuf,
        settings: &HashMap<String, String>,
    ) -> Self {
        Self {
            jwt_secret,
            storage_path,
            #[cfg(feature = "sms")]
            sms: crate::sms::SmsConfig::from_settings(settings),
            #[cfg(feature = "email")]
            email: crate::email::EmailConfig::from_settings(settings),
        }
    }
}
