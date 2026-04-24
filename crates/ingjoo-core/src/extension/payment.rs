//! 支付提供商 — 创建支付意图、确认、取消和退款

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 支付状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PaymentStatus {
    Pending,
    Processing,
    Succeeded,
    Failed,
    Cancelled,
    Refunded,
}

/// 支付意图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentIntent {
    pub id: String,
    pub amount: i64,
    pub currency: String,
    pub status: PaymentStatus,
    pub metadata: serde_json::Value,
}

/// 支付结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResult {
    pub intent_id: String,
    pub status: PaymentStatus,
    pub transaction_id: Option<String>,
    pub client_secret: Option<String>,
}

/// 退款结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundResult {
    pub refund_id: String,
    pub amount: i64,
    pub status: PaymentStatus,
}

/// 支付提供商 — 统一的支付接口
#[async_trait]
pub trait PaymentProvider: Send + Sync {
    /// 创建支付意图
    async fn create_intent(
        &self,
        amount: i64,
        currency: &str,
        metadata: serde_json::Value,
    ) -> Result<PaymentResult, anyhow::Error>;

    /// 确认支付
    async fn confirm(&self, intent_id: &str) -> Result<PaymentResult, anyhow::Error>;

    /// 取消支付
    async fn cancel(&self, intent_id: &str) -> Result<PaymentResult, anyhow::Error>;

    /// 退款（部分或全额）
    async fn refund(&self, intent_id: &str, amount: Option<i64>) -> Result<RefundResult, anyhow::Error>;

    /// 查询支付状态
    async fn get_status(&self, intent_id: &str) -> Result<PaymentStatus, anyhow::Error>;
}
