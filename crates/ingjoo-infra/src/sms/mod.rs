use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::json;
use sha2::Sha256;
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

#[async_trait]
pub trait SmsProvider: Send + Sync {
    async fn send_verification_code(&self, phone: &str, code: &str) -> Result<()>;
}

pub struct SmsConfig {
    pub secret_id: String,
    pub secret_key: String,
    pub sdk_app_id: String,
    pub sign_name: String,
    pub template_id: String,
}

impl SmsConfig {
    pub fn from_settings(settings: &HashMap<String, String>) -> Option<Self> {
        let secret_id = settings.get("tencent_sms_secret_id").cloned().unwrap_or_default();
        let secret_key = settings.get("tencent_sms_secret_key").cloned().unwrap_or_default();
        let sdk_app_id = settings.get("tencent_sms_sdk_app_id").cloned().unwrap_or_default();
        let sign_name = settings.get("tencent_sms_sign_name").cloned().unwrap_or_default();
        let template_id = settings.get("tencent_sms_template_id").cloned().unwrap_or_default();
        if secret_id.is_empty() || secret_key.is_empty() || sdk_app_id.is_empty() {
            return None;
        }
        Some(Self { secret_id, secret_key, sdk_app_id, sign_name, template_id })
    }
}

pub struct TencentSmsProvider {
    config: SmsConfig,
}

impl TencentSmsProvider {
    pub fn new(config: SmsConfig) -> Self {
        Self { config }
    }

    pub fn from_settings(settings: &HashMap<String, String>) -> Option<Self> {
        SmsConfig::from_settings(settings).map(Self::new)
    }
}

#[async_trait]
impl SmsProvider for TencentSmsProvider {
    async fn send_verification_code(&self, phone: &str, code: &str) -> Result<()> {
        let phone_with_prefix = if phone.starts_with('+') { phone.to_string() } else { format!("+86{}", phone) };

        let host = "sms.tencentcloudapi.com";
        let service = "sms";
        let action = "SendSms";
        let timestamp = chrono::Utc::now().timestamp();
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let body = json!({
            "SmsSdkAppId": self.config.sdk_app_id,
            "SignName": self.config.sign_name,
            "TemplateId": self.config.template_id,
            "TemplateParamSet": [code],
            "PhoneNumberSet": [phone_with_prefix],
        });
        let payload = serde_json::to_string(&body)?;

        let canonical_request = format!("POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:{}\nx-tc-action:{}\n\ncontent-type;host;x-tc-action\n{}",
            host, action.to_lowercase(), sha256_hex(payload.as_bytes()));

        let credential_scope = format!("{}/{}/tc3_request", date, service);
        let string_to_sign = format!(
            "TC3-HMAC-SHA256\n{}\n{}\n{}",
            timestamp,
            credential_scope,
            sha256_hex(canonical_request.as_bytes())
        );

        let signature = hmac_sha256_chain(
            &[format!("TC3{}", self.config.secret_key).as_bytes(), date.as_bytes(), service.as_bytes(), b"tc3_request"],
            string_to_sign.as_bytes(),
        );

        let authorization = format!(
            "TC3-HMAC-SHA256 Credential={}/{}, SignedHeaders=content-type;host;x-tc-action, Signature={}",
            self.config.secret_id, credential_scope, signature
        );

        let client = Client::new();
        let resp = client
            .post(format!("https://{}", host))
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Host", host)
            .header("X-TC-Action", action)
            .header("X-TC-Version", "2021-01-11")
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("Authorization", authorization)
            .body(payload)
            .send()
            .await
            .map_err(|e| anyhow!("SMS 请求失败: {}", e))?;

        let resp_json: serde_json::Value = resp.json().await.map_err(|e| anyhow!("SMS 响应解析失败: {}", e))?;

        if let Some(err) = resp_json["Response"]["Error"].as_object() {
            return Err(anyhow!(
                "SMS 发送失败: {} - {}",
                err.get("Code").and_then(|v| v.as_str()).unwrap_or("unknown"),
                err.get("Message").and_then(|v| v.as_str()).unwrap_or("unknown")
            ));
        }

        let status = resp_json["Response"]["SendStatusSet"][0]["Code"].as_str().unwrap_or("unknown");
        if status != "Ok" {
            return Err(anyhow!("SMS 发送失败: {}", status));
        }

        Ok(())
    }
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256_chain(keys: &[&[u8]], data: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(keys[0]).expect("HMAC key length");
    mac.update(data);
    let mut result = mac.finalize().into_bytes().to_vec();

    for key in &keys[1..] {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length");
        mac.update(&result);
        result = mac.finalize().into_bytes().to_vec();
    }

    hex::encode(result)
}
