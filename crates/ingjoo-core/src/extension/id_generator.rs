use async_trait::async_trait;

#[async_trait]
pub trait IdGenerator: Send + Sync {
    fn generate(&self, prefix: &str) -> String;

    fn generate_uuid(&self) -> String;

    fn generate_short_id(&self, length: usize) -> String;
}
