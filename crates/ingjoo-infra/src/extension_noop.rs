//! 扩展 trait 的 Noop 实现 — 功能未启用时返回空结果或错误
//!
//! 包含:
//! - `NoopLlmProvider` — LLM 服务未启用
//! - `NoopVectorStore` — 向量存储未启用
//! - `NoopDocumentLoader` — 文档加载未启用
//! - `NoopTextSplitter` — 文档分割（空操作，返回原文）
//! - `NoopDataMask` — 数据脱敏（脱敏展示实际工作，加密返回错误）
//! - `NoopContentFilter` — 内容过滤未启用
//! - `NoopInputSanitizer` — 输入净化（实际工作，HTML 实体转义）
//! - `NoopSignatureVerifier` — 签名验证未启用
//! - `NoopAuditStore` — 审计日志未启用（静默丢弃，不报错）

use async_trait::async_trait;
use ingjoo_core::extension::audit::{
    AuditEntry, AuditQuery, AuditStore,
};
use ingjoo_core::extension::content_filter::{ContentFilter, FilterResult};
use ingjoo_core::extension::document::{Document, DocumentLoader, TextSplitter};
use ingjoo_core::extension::llm::{
    ChatMessage, ChatOptions, ChatResponse, LlmProvider,
};
use ingjoo_core::extension::masking::{DataMask, MaskType};
use ingjoo_core::extension::sanitize::InputSanitizer;
use ingjoo_core::extension::signature::{HttpRequest, SignatureVerifier};
use ingjoo_core::extension::vector::{VectorResult, VectorStore};

// ---------------------------------------------------------------------------
// LlmProvider
// ---------------------------------------------------------------------------

/// LLM 服务的空实现 — 所有方法返回"未启用"错误
pub struct NoopLlmProvider;

#[async_trait]
impl LlmProvider for NoopLlmProvider {
    async fn chat_completion(
        &self,
        _messages: &[ChatMessage],
        _options: Option<ChatOptions>,
    ) -> Result<ChatResponse, anyhow::Error> {
        Err(anyhow::anyhow!("LLM 服务未启用"))
    }

    async fn embed(
        &self,
        _texts: &[String],
        _model: Option<&str>,
    ) -> Result<Vec<Vec<f32>>, anyhow::Error> {
        Err(anyhow::anyhow!("嵌入服务未启用"))
    }

    fn token_count(&self, _text: &str) -> u32 {
        0
    }
}

// ---------------------------------------------------------------------------
// VectorStore
// ---------------------------------------------------------------------------

/// 向量存储的空实现 — 所有方法返回"未启用"错误
pub struct NoopVectorStore;

#[async_trait]
impl VectorStore for NoopVectorStore {
    async fn store(
        &self,
        _collection: &str,
        _id: &str,
        _vector: &[f32],
        _metadata: Option<serde_json::Value>,
    ) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("向量存储未启用"))
    }

    async fn search(
        &self,
        _collection: &str,
        _query: &[f32],
        _top_k: usize,
        _filter: Option<serde_json::Value>,
    ) -> Result<Vec<VectorResult>, anyhow::Error> {
        Err(anyhow::anyhow!("向量存储未启用"))
    }

    async fn delete(
        &self,
        _collection: &str,
        _id: &str,
    ) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("向量存储未启用"))
    }
}

// ---------------------------------------------------------------------------
// DocumentLoader
// ---------------------------------------------------------------------------

/// 文档加载器的空实现 — 返回"未启用"错误
pub struct NoopDocumentLoader;

#[async_trait]
impl DocumentLoader for NoopDocumentLoader {
    async fn load(&self, _source: &str) -> Result<Vec<Document>, anyhow::Error> {
        Err(anyhow::anyhow!("文档加载服务未启用"))
    }
}

// ---------------------------------------------------------------------------
// TextSplitter
// ---------------------------------------------------------------------------

/// 文档分割器的空实现 — 直接返回原文不分割
pub struct NoopTextSplitter;

impl TextSplitter for NoopTextSplitter {
    fn split(&self, documents: &[Document]) -> Vec<Document> {
        documents.to_vec()
    }
}

// ---------------------------------------------------------------------------
// DataMask
// ---------------------------------------------------------------------------

/// 数据脱敏的空实现 — `mask_display` 实际工作，加密/解密返回错误
pub struct NoopDataMask;

#[async_trait]
impl DataMask for NoopDataMask {
    async fn encrypt(&self, _plain: &str) -> Result<String, anyhow::Error> {
        Err(anyhow::anyhow!("数据加密服务未启用"))
    }

    async fn decrypt(&self, _cipher: &str) -> Result<String, anyhow::Error> {
        Err(anyhow::anyhow!("数据解密服务未启用"))
    }

    fn mask_display(&self, value: &str, mask_type: MaskType) -> String {
        match mask_type {
            MaskType::Phone => {
                let chars: Vec<char> = value.chars().collect();
                if chars.len() >= 7 {
                    let mut result = String::new();
                    for (i, &c) in chars.iter().enumerate() {
                        if i < 3 || i >= chars.len() - 4 {
                            result.push(c);
                        } else {
                            result.push('*');
                        }
                    }
                    result
                } else {
                    value.to_string()
                }
            }
            MaskType::IdCard => {
                let chars: Vec<char> = value.chars().collect();
                if chars.len() >= 7 {
                    let mut result = String::new();
                    for (i, &c) in chars.iter().enumerate() {
                        if i < 3 || i >= chars.len() - 4 {
                            result.push(c);
                        } else {
                            result.push('*');
                        }
                    }
                    result
                } else {
                    value.to_string()
                }
            }
            MaskType::Email => {
                if let Some(at_pos) = value.find('@') {
                    let local = &value[..at_pos];
                    let domain = &value[at_pos..];
                    if local.len() > 1 {
                        format!("{}****{}", &local[..1], domain)
                    } else {
                        format!("*{}", domain)
                    }
                } else {
                    value.to_string()
                }
            }
            MaskType::BankCard => {
                let chars: Vec<char> = value.chars().collect();
                let digits: String = chars.iter().collect();
                if digits.len() > 4 {
                    let last4: String = chars.iter().rev().take(4).rev().collect();
                    format!("****{}", last4)
                } else {
                    value.to_string()
                }
            }
            MaskType::Custom { head, tail } => {
                let chars: Vec<char> = value.chars().collect();
                if chars.len() <= head + tail {
                    value.to_string()
                } else {
                    let mut result = String::new();
                    for (i, &c) in chars.iter().enumerate() {
                        if i < head || i >= chars.len() - tail {
                            result.push(c);
                        } else {
                            result.push('*');
                        }
                    }
                    result
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ContentFilter
// ---------------------------------------------------------------------------

/// 内容过滤的空实现 — 默认放行
pub struct NoopContentFilter;

#[async_trait]
impl ContentFilter for NoopContentFilter {
    async fn check_text(&self, _text: &str) -> Result<FilterResult, anyhow::Error> {
        Ok(FilterResult {
            passed: true,
            reason: None,
            category: None,
        })
    }

    async fn check_file(
        &self,
        _data: &[u8],
        _file_type: &str,
    ) -> Result<FilterResult, anyhow::Error> {
        Ok(FilterResult {
            passed: true,
            reason: None,
            category: None,
        })
    }
}

// ---------------------------------------------------------------------------
// InputSanitizer
// ---------------------------------------------------------------------------

/// 输入净化的实际实现 — HTML 实体转义
pub struct NoopInputSanitizer;

#[async_trait]
impl InputSanitizer for NoopInputSanitizer {
    async fn sanitize_html(&self, html: &str) -> Result<String, anyhow::Error> {
        // 简化版: 移除所有 HTML 标签，只保留文本内容
        let mut result = String::with_capacity(html.len());
        let mut in_tag = false;
        for ch in html.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(ch),
                _ => {}
            }
        }
        Ok(result)
    }

    async fn sanitize_plain_text(&self, text: &str) -> Result<String, anyhow::Error> {
        Ok(text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;"))
    }
}

// ---------------------------------------------------------------------------
// SignatureVerifier
// ---------------------------------------------------------------------------

/// 签名验证的空实现 — 返回"未启用"错误
pub struct NoopSignatureVerifier;

#[async_trait]
impl SignatureVerifier for NoopSignatureVerifier {
    async fn verify(
        &self,
        _request: &HttpRequest,
        _secret: &str,
        _tolerance: std::time::Duration,
    ) -> Result<bool, anyhow::Error> {
        Err(anyhow::anyhow!("签名验证服务未启用"))
    }
}

// ---------------------------------------------------------------------------
// AuditStore
// ---------------------------------------------------------------------------

/// 审计日志的空实现 — 静默丢弃，不报错
///
/// 审计失败不应阻断业务流程，因此返回空的成功结果
pub struct NoopAuditStore;

#[async_trait]
impl AuditStore for NoopAuditStore {
    async fn create_audit_log(
        &self,
        _user_id: Option<&str>,
        _action: &str,
        _resource: &str,
        _resource_id: Option<&str>,
        _detail: Option<serde_json::Value>,
        _ip: Option<&str>,
    ) -> Result<AuditEntry, anyhow::Error> {
        // 审计未启用，静默丢弃
        Ok(AuditEntry {
            id: String::new(),
            user_id: None,
            action: String::new(),
            resource: String::new(),
            resource_id: None,
            detail: None,
            ip: None,
            created_at: String::new(),
        })
    }

    async fn list_audit_logs(
        &self,
        _query: AuditQuery,
    ) -> Result<Vec<AuditEntry>, anyhow::Error> {
        Ok(vec![])
    }

    async fn get_audit_log(
        &self,
        _id: &str,
    ) -> Result<Option<AuditEntry>, anyhow::Error> {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_llm_provider_returns_error() {
        let provider = NoopLlmProvider;
        let result = provider
            .chat_completion(&[], None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("LLM 服务未启用"));

        let embed_result = provider.embed(&[], None).await;
        assert!(embed_result.is_err());

        assert_eq!(provider.token_count("hello"), 0);
    }

    #[tokio::test]
    async fn test_noop_vector_store_returns_error() {
        let store = NoopVectorStore;
        assert!(store.store("c", "id", &[], None).await.is_err());
        assert!(store.search("c", &[], 1, None).await.is_err());
        assert!(store.delete("c", "id").await.is_err());
    }

    #[tokio::test]
    async fn test_noop_document_loader_returns_error() {
        let loader = NoopDocumentLoader;
        assert!(loader.load("source").await.is_err());
    }

    #[test]
    fn test_noop_text_splitter_returns_original() {
        let splitter = NoopTextSplitter;
        let docs = vec![Document {
            id: "1".to_string(),
            content: "hello".to_string(),
            metadata: serde_json::Value::Null,
        }];
        let result = splitter.split(&docs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello");
    }

    #[test]
    fn test_noop_data_mask_phone() {
        let masker = NoopDataMask;
        let result = masker.mask_display("13812345678", MaskType::Phone);
        assert_eq!(result, "138****5678");
    }

    #[test]
    fn test_noop_data_mask_email() {
        let masker = NoopDataMask;
        let result = masker.mask_display("user@example.com", MaskType::Email);
        assert_eq!(result, "u****@example.com");
    }

    #[test]
    fn test_noop_data_mask_bank_card() {
        let masker = NoopDataMask;
        let result = masker.mask_display("6222021234561234", MaskType::BankCard);
        assert_eq!(result, "****1234");
    }

    #[test]
    fn test_noop_data_mask_custom() {
        let masker = NoopDataMask;
        let result = masker.mask_display("ABCDEFGHIJ", MaskType::Custom { head: 2, tail: 3 });
        assert_eq!(result, "AB*****HIJ");
    }

    #[tokio::test]
    async fn test_noop_content_filter_passes() {
        let filter = NoopContentFilter;
        let result = filter.check_text("anything").await.unwrap();
        assert!(result.passed);
        let file_result = filter.check_file(&[], "png").await.unwrap();
        assert!(file_result.passed);
    }

    #[tokio::test]
    async fn test_noop_input_sanitizer_escapes_html() {
        let sanitizer = NoopInputSanitizer;
        let result = sanitizer.sanitize_plain_text("<script>alert('xss')</script>").await.unwrap();
        assert_eq!(result, "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;");
    }

    #[tokio::test]
    async fn test_noop_input_sanitizer_strips_tags() {
        let sanitizer = NoopInputSanitizer;
        let result = sanitizer.sanitize_html("<b>hello</b> world").await.unwrap();
        assert_eq!(result, "hello world");
    }

    #[tokio::test]
    async fn test_noop_signature_verifier_returns_error() {
        let verifier = NoopSignatureVerifier;
        let req = HttpRequest {
            method: "GET".to_string(),
            path: "/test".to_string(),
            timestamp: 0,
            body: None,
            headers: vec![],
        };
        assert!(verifier.verify(&req, "secret", std::time::Duration::from_secs(60)).await.is_err());
    }

    #[tokio::test]
    async fn test_noop_audit_store_silent_discard() {
        let store = NoopAuditStore;
        let entry = store.create_audit_log(None, "test", "res", None, None, None).await.unwrap();
        assert!(entry.id.is_empty());

        let logs = store.list_audit_logs(AuditQuery {
            user_id: None,
            action: None,
            resource: None,
            resource_id: None,
            ip: None,
            limit: None,
            offset: None,
        }).await.unwrap();
        assert!(logs.is_empty());

        let log = store.get_audit_log("id").await.unwrap();
        assert!(log.is_none());
    }
}
