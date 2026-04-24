use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,       // "system" | "user" | "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub model: Option<String>,
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<ChatUsage>,
    pub finish_reason: Option<String>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 聊天补全 — 发送消息序列，获取模型回复
    async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        options: Option<ChatOptions>,
    ) -> Result<ChatResponse, anyhow::Error>;

    /// 文本向量化 — 将文本转换为嵌入向量
    async fn embed(
        &self,
        texts: &[String],
        model: Option<&str>,
    ) -> Result<Vec<Vec<f32>>, anyhow::Error>;

    /// 估算 token 数量（同步方法，不调用 API）
    fn token_count(&self, text: &str) -> u32;
}
