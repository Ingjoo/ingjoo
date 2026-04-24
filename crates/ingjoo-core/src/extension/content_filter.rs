use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 内容过滤结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterResult {
    pub passed: bool,
    pub reason: Option<String>,
    pub category: Option<String>,
}

#[async_trait]
pub trait ContentFilter: Send + Sync {
    /// 检查文本内容 — 对文本进行安全过滤
    async fn check_text(&self, text: &str) -> Result<FilterResult, anyhow::Error>;

    /// 检查文件内容 — 对二进制文件进行安全过滤
    async fn check_file(&self, data: &[u8], file_type: &str) -> Result<FilterResult, anyhow::Error>;
}
