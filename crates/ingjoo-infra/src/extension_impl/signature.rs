//! HMAC-SHA256 请求签名验证 — 含时间戳防重放

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use ingjoo_core::extension::signature::{HttpRequest, SignatureVerifier};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// 基于 HMAC-SHA256 的请求签名验证器
///
/// 签名消息格式: `{method}\n{path}\n{timestamp}\n{body}`
/// 验证步骤:
/// 1. 检查时间戳是否在 tolerance 范围内（防重放）
/// 2. 重新计算 HMAC-SHA256 签名
/// 3. 常量时间比较签名（防时序攻击）
/// HMAC-SHA256 签名验证器
pub struct HmacSignatureVerifier;

impl HmacSignatureVerifier {
    pub fn new() -> Self {
        Self
    }

    /// 构造待签名字符串
    fn signing_string(request: &HttpRequest) -> String {
        let body = request.body.as_deref().unwrap_or("");
        format!(
            "{}\n{}\n{}\n{}",
            request.method, request.path, request.timestamp, body
        )
    }

    /// 计算 HMAC-SHA256 签名
    fn compute_signature(secret: &str, message: &str) -> Result<Vec<u8>, anyhow::Error> {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|_| anyhow::anyhow!("HMAC 密钥初始化失败"))?;
        mac.update(message.as_bytes());
        Ok(mac.finalize().into_bytes().to_vec())
    }

    /// 从请求头中提取签名（查找 X-Signature 头）
    fn extract_signature(request: &HttpRequest) -> Option<String> {
        for (name, value) in &request.headers {
            if name.eq_ignore_ascii_case("X-Signature") {
                return Some(value.clone());
            }
        }
        None
    }
}

impl Default for HmacSignatureVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SignatureVerifier for HmacSignatureVerifier {
    async fn verify(
        &self,
        request: &HttpRequest,
        secret: &str,
        tolerance: std::time::Duration,
    ) -> Result<bool, anyhow::Error> {
        // 防重放: 检查时间戳
        let now = chrono::Utc::now().timestamp();
        let diff = (now - request.timestamp).abs();
        if diff > tolerance.as_secs() as i64 {
            return Ok(false);
        }

        let signature = Self::extract_signature(request)
            .ok_or_else(|| anyhow::anyhow!("缺少 X-Signature 请求头"))?;

        let message = Self::signing_string(request);
        let expected = Self::compute_signature(secret, &message)?;

        let sig_bytes = hex::decode(&signature)
            .map_err(|_| anyhow::anyhow!("签名格式无效，预期十六进制编码"))?;

        // 常量时间比较
        if sig_bytes.len() != expected.len() {
            return Ok(false);
        }
        let mut diff = 0u8;
        for (a, b) in sig_bytes.iter().zip(expected.iter()) {
            diff |= a ^ b;
        }
        Ok(diff == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(timestamp: i64, body: Option<&str>) -> HttpRequest {
        let mut headers = vec![];
        let message = format!("POST\n/api/data\n{}\n{}", timestamp, body.unwrap_or(""));
        let mut mac = HmacSha256::new_from_slice(b"secret_key").unwrap();
        mac.update(message.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());
        headers.push(("X-Signature".to_string(), sig));

        HttpRequest {
            method: "POST".to_string(),
            path: "/api/data".to_string(),
            timestamp,
            body: body.map(String::from),
            headers,
        }
    }

    #[tokio::test]
    async fn test_valid_signature_passes() {
        let verifier = HmacSignatureVerifier::new();
        let now = chrono::Utc::now().timestamp();
        let req = make_request(now, Some("hello"));
        let result = verifier
            .verify(&req, "secret_key", std::time::Duration::from_secs(300))
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_wrong_secret_fails() {
        let verifier = HmacSignatureVerifier::new();
        let now = chrono::Utc::now().timestamp();
        let req = make_request(now, Some("hello"));
        let result = verifier
            .verify(&req, "wrong_secret", std::time::Duration::from_secs(300))
            .await
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_expired_timestamp_fails() {
        let verifier = HmacSignatureVerifier::new();
        let one_hour_ago = chrono::Utc::now().timestamp() - 3600;
        let req = make_request(one_hour_ago, Some("hello"));
        let result = verifier
            .verify(&req, "secret_key", std::time::Duration::from_secs(300))
            .await
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_missing_signature_header_fails() {
        let verifier = HmacSignatureVerifier::new();
        let now = chrono::Utc::now().timestamp();
        let req = HttpRequest {
            method: "POST".to_string(),
            path: "/api".to_string(),
            timestamp: now,
            body: None,
            headers: vec![],
        };
        let result = verifier
            .verify(&req, "secret_key", std::time::Duration::from_secs(300))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_signing_string_format() {
        let req = HttpRequest {
            method: "GET".to_string(),
            path: "/test".to_string(),
            timestamp: 12345,
            body: Some("body".to_string()),
            headers: vec![],
        };
        let s = HmacSignatureVerifier::signing_string(&req);
        assert_eq!(s, "GET\n/test\n12345\nbody");
    }

    #[test]
    fn test_signing_string_no_body() {
        let req = HttpRequest {
            method: "GET".to_string(),
            path: "/test".to_string(),
            timestamp: 12345,
            body: None,
            headers: vec![],
        };
        let s = HmacSignatureVerifier::signing_string(&req);
        assert_eq!(s, "GET\n/test\n12345\n");
    }

    #[test]
    fn test_compute_signature_deterministic() {
        let s1 = HmacSignatureVerifier::compute_signature("key", "message").unwrap();
        let s2 = HmacSignatureVerifier::compute_signature("key", "message").unwrap();
        assert_eq!(s1, s2);
    }
}
