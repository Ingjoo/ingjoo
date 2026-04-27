//! 扩展 trait 集合 — 可插拔的基础设施抽象
//!
//! 每个 trait 定义一种独立的扩展能力（审计、状态机、事件总线、搜索、支付等），
//! 由 `ingjoo-infra` 或外部 crate 提供具体实现。

pub mod audit;
pub mod content_filter;
pub mod document;
pub mod event_bus;
pub mod id_generator;
pub mod llm;
pub mod lock;
pub mod masking;
pub mod notification;
pub mod payment;
pub mod prompt;
pub mod relations;
pub mod sanitize;
pub mod search;
pub mod signature;
pub mod state_machine;
pub mod translation;
pub mod vector;

pub use audit::{AuditEntry, AuditQuery, AuditStore};
pub use content_filter::{ContentFilter, FilterResult};
pub use document::{Document, DocumentLoader, TextSplitter};
pub use event_bus::{Event, EventBus, EventHandler};
pub use id_generator::IdGenerator;
pub use llm::{ChatMessage, ChatOptions, ChatResponse, ChatUsage, LlmProvider};
pub use lock::{Lock, LockGuard};
pub use masking::{DataMask, MaskType};
pub use notification::NotificationStore;
pub use payment::{PaymentIntent, PaymentProvider, PaymentResult, PaymentStatus, RefundResult};
pub use prompt::{PromptManager, PromptTemplate};
pub use relations::{Many2Many, RelationLoader};
pub use sanitize::InputSanitizer;
pub use search::{SearchEngine, SearchHighlight, SearchQuery, SearchResult};
pub use signature::{HttpRequest, SignatureVerifier};
pub use state_machine::{StateMachine, StateTransition, TransitionError};
pub use translation::{Translation, TranslationStore};
pub use vector::{VectorResult, VectorStore};
