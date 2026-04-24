use async_trait::async_trait;

/// HTTP 请求签名（用于 API 验证）
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub timestamp: i64,
    pub body: Option<String>,
    pub headers: Vec<(String, String)>,
}

#[async_trait]
pub trait SignatureVerifier: Send + Sync {
    /// 验证请求签名 — 校验 HMAC-SHA256 签名和时间戳防重放
    /// tolerance: 允许的时间偏差（秒）
    async fn verify(
        &self,
        request: &HttpRequest,
        secret: &str,
        tolerance: std::time::Duration,
    ) -> Result<bool, anyhow::Error>;
}
