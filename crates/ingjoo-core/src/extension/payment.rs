use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PaymentStatus {
    Pending,
    Processing,
    Succeeded,
    Failed,
    Cancelled,
    Refunded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentIntent {
    pub id: String,
    pub amount: i64,
    pub currency: String,
    pub status: PaymentStatus,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResult {
    pub intent_id: String,
    pub status: PaymentStatus,
    pub transaction_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundResult {
    pub refund_id: String,
    pub amount: i64,
    pub status: PaymentStatus,
}

#[async_trait]
pub trait PaymentProvider: Send + Sync {
    async fn create_intent(
        &self,
        amount: i64,
        currency: &str,
        metadata: serde_json::Value,
    ) -> Result<PaymentResult, anyhow::Error>;

    async fn confirm(&self, intent_id: &str) -> Result<PaymentResult, anyhow::Error>;

    async fn cancel(&self, intent_id: &str) -> Result<PaymentResult, anyhow::Error>;

    async fn refund(&self, intent_id: &str, amount: Option<i64>) -> Result<RefundResult, anyhow::Error>;

    async fn get_status(&self, intent_id: &str) -> Result<PaymentStatus, anyhow::Error>;
}
