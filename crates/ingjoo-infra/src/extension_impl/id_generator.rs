use async_trait::async_trait;
use ingjoo_core::extension::IdGenerator;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const EPOCH_MS: u64 = 1700000000000;
const MACHINE_ID: u64 = 1;

/// 基于 UUID v4 的默认 ID 生成器
pub struct DefaultIdGenerator {
    snowflake_counter: AtomicU64,
}

impl DefaultIdGenerator {
    pub fn new() -> Self {
        Self {
            snowflake_counter: AtomicU64::new(0),
        }
    }

    pub fn generate_uuid_v4() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    pub fn generate_snowflake_id(counter: &AtomicU64) -> String {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let seq = counter.fetch_add(1, Ordering::Relaxed) & 0xFFF;
        let id = ((ts - EPOCH_MS) << 22) | (MACHINE_ID << 12) | seq;
        id.to_string()
    }
}

impl Default for DefaultIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IdGenerator for DefaultIdGenerator {
    fn generate(&self, prefix: &str) -> String {
        let id = Self::generate_snowflake_id(&self.snowflake_counter);
        format!("{}_{}", prefix, id)
    }

    fn generate_uuid(&self) -> String {
        Self::generate_uuid_v4()
    }

    fn generate_short_id(&self, length: usize) -> String {
        let uuid = Self::generate_uuid_v4();
        let hash = uuid.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
        hash[..hash.len().min(length)].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_v4_format() {
        let id = DefaultIdGenerator::generate_uuid_v4();
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn snowflake_id_is_numeric() {
        let gen = DefaultIdGenerator::new();
        let id = gen.generate("order");
        assert!(id.starts_with("order_"));
        let parts: Vec<&str> = id.splitn(2, '_').collect();
        assert!(parts[1].parse::<u64>().is_ok());
    }

    #[test]
    fn snowflake_ids_increase() {
        let gen = DefaultIdGenerator::new();
        let id1 = gen.generate("t");
        let id2 = gen.generate("t");
        let n1: u64 = id1.split('_').next_back().unwrap().parse().unwrap();
        let n2: u64 = id2.split('_').next_back().unwrap().parse().unwrap();
        assert!(n2 > n1);
    }

    #[test]
    fn generate_uuid_returns_unique() {
        let gen = DefaultIdGenerator::new();
        let a = gen.generate_uuid();
        let b = gen.generate_uuid();
        assert_ne!(a, b);
    }

    #[test]
    fn short_id_length() {
        let gen = DefaultIdGenerator::new();
        let id = gen.generate_short_id(8);
        assert_eq!(id.len(), 8);
    }
}
