use async_trait::async_trait;
use ingjoo_core::db::models::{MailMessage, MailNotification, NotificationItem};
use ingjoo_core::extension::notification::NotificationStore;
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;

pub struct DbNotificationStore {
    pool: Pool,
    dialect: Dialect,
}

impl DbNotificationStore {
    pub fn new(pool: Pool, dialect: Dialect) -> Self {
        Self { pool, dialect }
    }

    fn sql(&self, s: &str) -> String {
        self.dialect.prepare(s)
    }
}

#[async_trait]
impl NotificationStore for DbNotificationStore {
    async fn create_message(
        &self,
        author_id: Option<&str>,
        subject: &str,
        body: &str,
        message_type: &str,
    ) -> Result<MailMessage, anyhow::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let row: MailMessage = sqlx::query_as::<_, MailMessage>(&self.sql(
            "INSERT INTO mail_messages (id, author_id, subject, body, message_type) VALUES (?, ?, ?, ?, ?) RETURNING *",
        ))
        .bind(&id)
        .bind(author_id)
        .bind(subject)
        .bind(body)
        .bind(message_type)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn create_notification(&self, message_id: &str, user_id: &str) -> Result<MailNotification, anyhow::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let row: MailNotification = sqlx::query_as::<_, MailNotification>(
            &self.sql("INSERT INTO mail_notifications (id, message_id, user_id) VALUES (?, ?, ?) RETURNING *"),
        )
        .bind(&id)
        .bind(message_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn list_notifications(
        &self,
        user_id: &str,
        unread_only: bool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<NotificationItem>, anyhow::Error> {
        let sql = if unread_only {
            "SELECT n.id as n_id, n.message_id, n.user_id, n.is_read, n.created_at as n_created_at, m.id as m_id, m.author_id, m.subject, m.body, m.message_type, m.created_at as m_created_at FROM mail_notifications n JOIN mail_messages m ON n.message_id = m.id WHERE n.user_id = ? AND n.is_read = 0 ORDER BY n.created_at DESC LIMIT ? OFFSET ?"
        } else {
            "SELECT n.id as n_id, n.message_id, n.user_id, n.is_read, n.created_at as n_created_at, m.id as m_id, m.author_id, m.subject, m.body, m.message_type, m.created_at as m_created_at FROM mail_notifications n JOIN mail_messages m ON n.message_id = m.id WHERE n.user_id = ? ORDER BY n.created_at DESC LIMIT ? OFFSET ?"
        };
        let rows = sqlx::query(&self.sql(sql)).bind(user_id).bind(limit).bind(offset).fetch_all(&self.pool).await?;

        let items = rows
            .iter()
            .map(|row| {
                use sqlx::Row;
                NotificationItem {
                    notification: MailNotification {
                        id: row.get("n_id"),
                        message_id: row.get("message_id"),
                        user_id: row.get("user_id"),
                        is_read: row.get::<i32, _>("is_read") != 0,
                        created_at: row.get("n_created_at"),
                    },
                    message: MailMessage {
                        id: row.get("m_id"),
                        author_id: row.get("author_id"),
                        subject: row.get("subject"),
                        body: row.get("body"),
                        message_type: row.get("message_type"),
                        created_at: row.get("m_created_at"),
                    },
                }
            })
            .collect();
        Ok(items)
    }

    async fn get_unread_count(&self, user_id: &str) -> Result<i64, anyhow::Error> {
        let count: i64 =
            sqlx::query_scalar(&self.sql("SELECT COUNT(*) FROM mail_notifications WHERE user_id = ? AND is_read = 0"))
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    async fn mark_read(&self, notification_id: &str, user_id: &str) -> Result<bool, anyhow::Error> {
        let result = sqlx::query(
            &self.sql("UPDATE mail_notifications SET is_read = 1 WHERE id = ? AND user_id = ? AND is_read = 0"),
        )
        .bind(notification_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn mark_all_read(&self, user_id: &str) -> Result<u64, anyhow::Error> {
        let result =
            sqlx::query(&self.sql("UPDATE mail_notifications SET is_read = 1 WHERE user_id = ? AND is_read = 0"))
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_notification_store_dialect_format() {
        let dialect = Dialect::Sqlite;
        assert_eq!(format!("{:?}", dialect), "Sqlite");
        let dialect = Dialect::Postgres;
        assert_eq!(format!("{:?}", dialect), "Postgres");
    }
}
