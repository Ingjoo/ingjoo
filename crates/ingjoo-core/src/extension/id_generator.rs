//! ID 生成器 — UUID、前缀 ID、短 ID 生成

use async_trait::async_trait;

/// ID 生成器 — 生成各种格式的唯一标识符
#[async_trait]
pub trait IdGenerator: Send + Sync {
    /// 生成带前缀的 ID（如 "usr_xxxx"）
    fn generate(&self, prefix: &str) -> String;

    /// 生成标准 UUID v4
    fn generate_uuid(&self) -> String;

    /// 生成指定长度的短 ID（URL 友好的随机字符串）
    fn generate_short_id(&self, length: usize) -> String;
}
