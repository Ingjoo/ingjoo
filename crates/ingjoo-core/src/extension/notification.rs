use crate::db::models::{MailMessage, MailNotification, NotificationItem};
use async_trait::async_trait;

#[async_trait]
pub trait NotificationStore: Send + Sync {
    async fn create_message(
        &self,
        author_id: Option<&str>,
        subject: &str,
        body: &str,
        message_type: &str,
    ) -> Result<MailMessage, anyhow::Error>;

    async fn create_notification(&self, message_id: &str, user_id: &str) -> Result<MailNotification, anyhow::Error>;

    async fn list_notifications(
        &self,
        user_id: &str,
        unread_only: bool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<NotificationItem>, anyhow::Error>;

    async fn get_unread_count(&self, user_id: &str) -> Result<i64, anyhow::Error>;

    async fn mark_read(&self, notification_id: &str, user_id: &str) -> Result<bool, anyhow::Error>;

    async fn mark_all_read(&self, user_id: &str) -> Result<u64, anyhow::Error>;
}
