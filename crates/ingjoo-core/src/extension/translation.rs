//! 多语言翻译存储 — 字段级别的翻译读写

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 翻译条目 — 记录某个模型某个字段在某种语言下的翻译值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Translation {
    pub lang: String,
    pub model: String,
    pub field: String,
    pub record_id: String,
    pub value: String,
}

/// 翻译存储 — 字段级别的多语言翻译读写
#[async_trait]
pub trait TranslationStore: Send + Sync {
    /// 获取单条翻译
    async fn get(
        &self,
        lang: &str,
        model: &str,
        field: &str,
        record_id: &str,
    ) -> Result<Option<String>, anyhow::Error>;

    /// 写入一条翻译（存在则覆盖）
    async fn set(
        &self,
        translation: Translation,
    ) -> Result<(), anyhow::Error>;

    /// 批量获取翻译（返回 record_id → value 映射）
    async fn get_batch(
        &self,
        lang: &str,
        model: &str,
        field: &str,
        record_ids: &[String],
    ) -> Result<HashMap<String, String>, anyhow::Error>;

    /// 删除一条翻译
    async fn remove(
        &self,
        lang: &str,
        model: &str,
        field: &str,
        record_id: &str,
    ) -> Result<(), anyhow::Error>;

    /// 列出指定模型已有翻译的所有语言代码
    async fn list_languages(&self, model: &str) -> Result<Vec<String>, anyhow::Error>;
}
