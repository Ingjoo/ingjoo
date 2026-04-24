use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Translation {
    pub lang: String,
    pub model: String,
    pub field: String,
    pub record_id: String,
    pub value: String,
}

#[async_trait]
pub trait TranslationStore: Send + Sync {
    async fn get(
        &self,
        lang: &str,
        model: &str,
        field: &str,
        record_id: &str,
    ) -> Result<Option<String>, anyhow::Error>;

    async fn set(
        &self,
        translation: Translation,
    ) -> Result<(), anyhow::Error>;

    async fn get_batch(
        &self,
        lang: &str,
        model: &str,
        field: &str,
        record_ids: &[String],
    ) -> Result<HashMap<String, String>, anyhow::Error>;

    async fn remove(
        &self,
        lang: &str,
        model: &str,
        field: &str,
        record_id: &str,
    ) -> Result<(), anyhow::Error>;

    async fn list_languages(&self, model: &str) -> Result<Vec<String>, anyhow::Error>;
}
