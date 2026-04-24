use std::collections::HashMap;
use std::path::PathBuf;

pub struct IngjooConfig {
    pub jwt_secret: String,
    pub storage_path: PathBuf,
    #[cfg(feature = "sms")]
    pub sms: Option<crate::sms::SmsConfig>,
    #[cfg(feature = "email")]
    pub email: Option<crate::email::EmailConfig>,
}

impl IngjooConfig {
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
