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
use std::collections::HashMap;

use ingjoo_core::extension::audit::{AuditEntry, AuditQuery, AuditStore};
use ingjoo_core::extension::content_filter::{ContentFilter, FilterResult};
use ingjoo_core::extension::document::{Document, DocumentLoader, TextSplitter};
use ingjoo_core::extension::llm::{ChatMessage, ChatOptions, ChatResponse, LlmProvider};
use ingjoo_core::extension::masking::{DataMask, MaskType};
use ingjoo_core::extension::notification::NotificationStore;
use ingjoo_core::extension::payment::{PaymentProvider, PaymentResult, PaymentStatus, RefundResult};
use ingjoo_core::extension::relations::RelationLoader;
use ingjoo_core::extension::sanitize::InputSanitizer;
use ingjoo_core::extension::search::{SearchEngine, SearchQuery, SearchResult};
use ingjoo_core::extension::signature::{HttpRequest, SignatureVerifier};
use ingjoo_core::extension::state_machine::{StateMachine, StateTransition, TransitionError};
use ingjoo_core::extension::translation::{Translation, TranslationStore};
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

    async fn embed(&self, _texts: &[String], _model: Option<&str>) -> Result<Vec<Vec<f32>>, anyhow::Error> {
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

    async fn delete(&self, _collection: &str, _id: &str) -> Result<(), anyhow::Error> {
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
        Ok(FilterResult { passed: true, reason: None, category: None })
    }

    async fn check_file(&self, _data: &[u8], _file_type: &str) -> Result<FilterResult, anyhow::Error> {
        Ok(FilterResult { passed: true, reason: None, category: None })
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

    async fn list_audit_logs(&self, _query: AuditQuery) -> Result<Vec<AuditEntry>, anyhow::Error> {
        Ok(vec![])
    }

    async fn get_audit_log(&self, _id: &str) -> Result<Option<AuditEntry>, anyhow::Error> {
        Ok(None)
    }

    async fn delete_logs_before(&self, _before: &str) -> Result<u64, anyhow::Error> {
        Ok(0)
    }
}

// ---------------------------------------------------------------------------
// StateMachine
// ---------------------------------------------------------------------------

/// 状态机的空实现 — 返回空状态，转换返回错误
pub struct NoopStateMachine;

#[async_trait]
impl StateMachine for NoopStateMachine {
    async fn get_current_state(&self, _model: &str, _record_id: &str) -> Result<String, TransitionError> {
        Ok(String::new())
    }

    async fn get_available_transitions(
        &self,
        _model: &str,
        _record_id: &str,
    ) -> Result<Vec<StateTransition>, TransitionError> {
        Ok(vec![])
    }

    async fn transition(
        &self,
        model: &str,
        _record_id: &str,
        target_state: &str,
        _context: HashMap<String, serde_json::Value>,
    ) -> Result<String, TransitionError> {
        Err(TransitionError::InvalidTransition(format!(
            "无法从当前状态转换到 '{}': 状态机服务未启用 (model={})",
            target_state, model
        )))
    }

    async fn register_machine(
        &self,
        _model: &str,
        _states: Vec<String>,
        _transitions: Vec<StateTransition>,
        _initial_state: String,
    ) -> Result<(), TransitionError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SearchEngine
// ---------------------------------------------------------------------------

/// 搜索引擎的空实现 — 所有方法返回"未启用"错误
pub struct NoopSearchEngine;

#[async_trait]
impl SearchEngine for NoopSearchEngine {
    async fn index_record(
        &self,
        _model: &str,
        _record_id: &str,
        _data: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("搜索服务未启用"))
    }

    async fn remove_record(&self, _model: &str, _record_id: &str) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("搜索服务未启用"))
    }

    async fn search(&self, _query: SearchQuery) -> Result<Vec<SearchResult>, anyhow::Error> {
        Err(anyhow::anyhow!("搜索服务未启用"))
    }

    async fn rebuild_index(&self, _model: &str) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("搜索服务未启用"))
    }
}

// ---------------------------------------------------------------------------
// PaymentProvider
// ---------------------------------------------------------------------------

/// 支付提供商的空实现 — 所有方法返回"未启用"错误
pub struct NoopPaymentProvider;

#[async_trait]
impl PaymentProvider for NoopPaymentProvider {
    async fn create_intent(
        &self,
        _amount: i64,
        _currency: &str,
        _metadata: serde_json::Value,
    ) -> Result<PaymentResult, anyhow::Error> {
        Err(anyhow::anyhow!("支付服务未启用"))
    }

    async fn confirm(&self, _intent_id: &str) -> Result<PaymentResult, anyhow::Error> {
        Err(anyhow::anyhow!("支付服务未启用"))
    }

    async fn cancel(&self, _intent_id: &str) -> Result<PaymentResult, anyhow::Error> {
        Err(anyhow::anyhow!("支付服务未启用"))
    }

    async fn refund(&self, _intent_id: &str, _amount: Option<i64>) -> Result<RefundResult, anyhow::Error> {
        Err(anyhow::anyhow!("支付服务未启用"))
    }

    async fn get_status(&self, _intent_id: &str) -> Result<PaymentStatus, anyhow::Error> {
        Err(anyhow::anyhow!("支付服务未启用"))
    }
}

// ---------------------------------------------------------------------------
// RelationLoader
// ---------------------------------------------------------------------------

/// 关系加载器的空实现 — 返回空的 HashMap
pub struct NoopRelationLoader;

#[async_trait]
impl RelationLoader for NoopRelationLoader {
    async fn load_one2many(
        &self,
        _model: &str,
        _field: &str,
        _ids: &[String],
    ) -> Result<HashMap<String, Vec<serde_json::Value>>, anyhow::Error> {
        Ok(HashMap::new())
    }

    async fn load_many2one(
        &self,
        _model: &str,
        _field: &str,
        _ids: &[String],
    ) -> Result<HashMap<String, Option<serde_json::Value>>, anyhow::Error> {
        Ok(HashMap::new())
    }
}

// ---------------------------------------------------------------------------
// TranslationStore
// ---------------------------------------------------------------------------

/// 翻译存储的空实现 — 读取返回空，写入静默丢弃
pub struct NoopTranslationStore;

#[async_trait]
impl TranslationStore for NoopTranslationStore {
    async fn get(
        &self,
        _lang: &str,
        _model: &str,
        _field: &str,
        _record_id: &str,
    ) -> Result<Option<String>, anyhow::Error> {
        Ok(None)
    }

    async fn set(&self, _translation: Translation) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn get_batch(
        &self,
        _lang: &str,
        _model: &str,
        _field: &str,
        _record_ids: &[String],
    ) -> Result<HashMap<String, String>, anyhow::Error> {
        Ok(HashMap::new())
    }

    async fn remove(&self, _lang: &str, _model: &str, _field: &str, _record_id: &str) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn list_languages(&self, _model: &str) -> Result<Vec<String>, anyhow::Error> {
        Ok(vec![])
    }
}

// ---------------------------------------------------------------------------
// EmailProvider
// ---------------------------------------------------------------------------

/// 邮件服务的空实现 — 返回"未启用"错误
#[cfg(feature = "email")]
pub struct NoopEmailProvider;

#[cfg(feature = "email")]
#[async_trait]
impl crate::email::EmailProvider for NoopEmailProvider {
    async fn send(&self, _to: &str, _subject: &str, _html_body: &str) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("邮件服务未启用"))
    }
}

// ---------------------------------------------------------------------------
// SmsProvider
// ---------------------------------------------------------------------------

/// 短信服务的空实现 — 返回"未启用"错误
#[cfg(feature = "sms")]
pub struct NoopSmsProvider;

#[cfg(feature = "sms")]
#[async_trait]
impl crate::sms::SmsProvider for NoopSmsProvider {
    async fn send_verification_code(&self, _phone: &str, _code: &str) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("短信服务未启用"))
    }
}

// ---------------------------------------------------------------------------
// NotificationStore
// ---------------------------------------------------------------------------

/// 通知存储的空实现 — 返回空结果
pub struct NoopNotificationStore;

#[async_trait]
impl NotificationStore for NoopNotificationStore {
    async fn create_message(
        &self,
        _author_id: Option<&str>,
        _subject: &str,
        _body: &str,
        _message_type: &str,
    ) -> Result<ingjoo_core::db::models::MailMessage, anyhow::Error> {
        Ok(ingjoo_core::db::models::MailMessage {
            id: String::new(),
            author_id: None,
            subject: String::new(),
            body: String::new(),
            message_type: String::new(),
            created_at: String::new(),
        })
    }

    async fn create_notification(
        &self,
        _message_id: &str,
        _user_id: &str,
    ) -> Result<ingjoo_core::db::models::MailNotification, anyhow::Error> {
        Ok(ingjoo_core::db::models::MailNotification {
            id: String::new(),
            message_id: String::new(),
            user_id: String::new(),
            is_read: false,
            created_at: String::new(),
        })
    }

    async fn list_notifications(
        &self,
        _user_id: &str,
        _unread_only: bool,
        _limit: i64,
        _offset: i64,
    ) -> Result<Vec<ingjoo_core::db::models::NotificationItem>, anyhow::Error> {
        Ok(vec![])
    }

    async fn get_unread_count(&self, _user_id: &str) -> Result<i64, anyhow::Error> {
        Ok(0)
    }

    async fn mark_read(&self, _notification_id: &str, _user_id: &str) -> Result<bool, anyhow::Error> {
        Ok(false)
    }

    async fn mark_all_read(&self, _user_id: &str) -> Result<u64, anyhow::Error> {
        Ok(0)
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
        let result = provider.chat_completion(&[], None).await;
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
        let docs =
            vec![Document { id: "1".to_string(), content: "hello".to_string(), metadata: serde_json::Value::Null }];
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

        let logs = store
            .list_audit_logs(AuditQuery {
                user_id: None,
                action: None,
                resource: None,
                resource_id: None,
                ip: None,
                limit: None,
                offset: None,
            })
            .await
            .unwrap();
        assert!(logs.is_empty());

        let log = store.get_audit_log("id").await.unwrap();
        assert!(log.is_none());
    }

    #[tokio::test]
    async fn test_noop_state_machine() {
        let sm = NoopStateMachine;

        let state = sm.get_current_state("order", "1").await.unwrap();
        assert!(state.is_empty());

        let transitions = sm.get_available_transitions("order", "1").await.unwrap();
        assert!(transitions.is_empty());

        let err = sm.transition("order", "1", "confirmed", HashMap::new()).await.unwrap_err();
        match err {
            TransitionError::InvalidTransition(msg) => {
                assert!(msg.contains("confirmed"));
                assert!(msg.contains("order"));
            }
            other => panic!("预期 InvalidTransition，得到: {:?}", other),
        }

        sm.register_machine("order", vec![], vec![], "draft".to_string()).await.unwrap();
    }

    #[tokio::test]
    async fn test_noop_search_engine_returns_error() {
        let engine = NoopSearchEngine;
        assert!(engine.index_record("model", "1", &serde_json::Value::Null).await.is_err());
        assert!(engine.remove_record("model", "1").await.is_err());
        assert!(engine
            .search(SearchQuery { text: "test".to_string(), models: vec![], limit: None, offset: None, filters: None })
            .await
            .is_err());
        assert!(engine.rebuild_index("model").await.is_err());
    }

    #[tokio::test]
    async fn test_noop_payment_provider_returns_error() {
        let provider = NoopPaymentProvider;
        assert!(provider.create_intent(100, "CNY", serde_json::Value::Null).await.is_err());
        assert!(provider.confirm("id").await.is_err());
        assert!(provider.cancel("id").await.is_err());
        assert!(provider.refund("id", None).await.is_err());
        assert!(provider.get_status("id").await.is_err());
    }

    #[tokio::test]
    async fn test_noop_relation_loader_returns_empty() {
        let loader = NoopRelationLoader;
        let o2m = loader.load_one2many("order", "lines", &["1".to_string()]).await.unwrap();
        assert!(o2m.is_empty());

        let m2o = loader.load_many2one("order", "partner", &["1".to_string()]).await.unwrap();
        assert!(m2o.is_empty());
    }

    #[cfg(feature = "email")]
    #[tokio::test]
    async fn test_noop_email_provider_returns_error() {
        use crate::email::EmailProvider;
        let provider = NoopEmailProvider;
        let result = provider.send("test@example.com", "subject", "<p>body</p>").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("邮件服务未启用"));
    }

    #[cfg(feature = "sms")]
    #[tokio::test]
    async fn test_noop_sms_provider_returns_error() {
        use crate::sms::SmsProvider;
        let provider = NoopSmsProvider;
        let result = provider.send_verification_code("13812345678", "123456").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("短信服务未启用"));
    }

    #[tokio::test]
    async fn test_noop_translation_store_returns_none() {
        let store = NoopTranslationStore;
        let result = store.get("zh", "order", "name", "1").await.unwrap();
        assert!(result.is_none());

        store
            .set(Translation {
                lang: "zh".to_string(),
                model: "order".to_string(),
                field: "name".to_string(),
                record_id: "1".to_string(),
                value: "测试".to_string(),
            })
            .await
            .unwrap();

        let batch = store.get_batch("zh", "order", "name", &["1".to_string()]).await.unwrap();
        assert!(batch.is_empty());

        store.remove("zh", "order", "name", "1").await.unwrap();

        let langs = store.list_languages("order").await.unwrap();
        assert!(langs.is_empty());
    }
}
