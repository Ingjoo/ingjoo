use anyhow::{anyhow, Result};
use async_trait::async_trait;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::collections::HashMap;

#[async_trait]
pub trait EmailProvider: Send + Sync {
    async fn send(&self, to: &str, subject: &str, html_body: &str) -> Result<()>;
}

pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub smtp_from: String,
}

impl EmailConfig {
    pub fn from_settings(settings: &HashMap<String, String>) -> Option<Self> {
        let host = settings.get("smtp_host").cloned().unwrap_or_default();
        let pass = settings.get("smtp_pass").cloned().unwrap_or_default();
        let from = settings.get("smtp_from").cloned().unwrap_or_default();
        if host.is_empty() || pass.is_empty() || from.is_empty() {
            return None;
        }
        Some(Self {
            smtp_host: host,
            smtp_port: settings.get("smtp_port").cloned().unwrap_or_default().parse().unwrap_or(465),
            smtp_user: settings.get("smtp_user").cloned().unwrap_or_default(),
            smtp_pass: pass,
            smtp_from: from,
        })
    }
}

pub struct SmtpEmailProvider {
    config: EmailConfig,
}

impl SmtpEmailProvider {
    pub fn new(config: EmailConfig) -> Self {
        Self { config }
    }

    pub fn from_settings(settings: &HashMap<String, String>) -> Option<Self> {
        EmailConfig::from_settings(settings).map(Self::new)
    }
}

#[async_trait]
impl EmailProvider for SmtpEmailProvider {
    async fn send(&self, to: &str, subject: &str, html_body: &str) -> Result<()> {
        let email = Message::builder()
            .from(self.config.smtp_from.parse().map_err(|e| anyhow!("发件人地址无效: {}", e))?)
            .to(to.parse().map_err(|e| anyhow!("收件人地址无效: {}", e))?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html_body.to_string())
            .map_err(|e| anyhow!("邮件构建失败: {}", e))?;

        let creds = Credentials::new(self.config.smtp_user.clone(), self.config.smtp_pass.clone());

        let mailer = if self.config.smtp_port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.smtp_host)
                .map_err(|e| anyhow!("SMTP TLS 连接配置失败: {}", e))?
                .credentials(creds)
                .port(self.config.smtp_port)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.smtp_host)
                .map_err(|e| anyhow!("SMTP STARTTLS 连接配置失败: {}", e))?
                .credentials(creds)
                .port(self.config.smtp_port)
                .build()
        };

        mailer.send(email).await.map_err(|e| anyhow!("邮件发送失败: {}", e))?;
        Ok(())
    }
}
