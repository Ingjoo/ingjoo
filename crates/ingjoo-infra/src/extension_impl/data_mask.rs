//! AES-256-GCM 数据加密与脱敏

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use async_trait::async_trait;
use base64::Engine;
use ingjoo_core::extension::masking::{DataMask, MaskType};
use rand::RngCore;

const NONCE_SIZE: usize = 12;

/// 基于 AES-256-GCM 的数据加密脱敏实现
///
/// 加密格式: `base64(nonce[12字节] || ciphertext)`
/// 解密时从头部提取 12 字节 nonce，剩余部分为密文。
/// AES-256-GCM 数据脱敏实现
pub struct AesDataMask {
    cipher: Aes256Gcm,
}

impl AesDataMask {
    /// 使用 32 字节密钥创建实例
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(key.into());
        Self { cipher }
    }

    /// 生成随机 256 位密钥
    pub fn generate_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }
}

fn apply_mask(value: &str, mask_type: MaskType) -> String {
    match mask_type {
        MaskType::Phone => {
            let chars: Vec<char> = value.chars().collect();
            if chars.len() < 7 {
                return value.to_string();
            }
            let mut result = String::new();
            for (i, &c) in chars.iter().enumerate() {
                if i < 3 || i >= chars.len() - 4 {
                    result.push(c);
                } else {
                    result.push('*');
                }
            }
            result
        }
        MaskType::IdCard => {
            let chars: Vec<char> = value.chars().collect();
            if chars.len() < 7 {
                return value.to_string();
            }
            let mut result = String::new();
            for (i, &c) in chars.iter().enumerate() {
                if i < 3 || i >= chars.len() - 4 {
                    result.push(c);
                } else {
                    result.push('*');
                }
            }
            result
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
            if chars.len() > 4 {
                let last4: String = chars.iter().rev().take(4).rev().collect();
                format!("****{}", last4)
            } else {
                value.to_string()
            }
        }
        MaskType::Custom { head, tail } => {
            let chars: Vec<char> = value.chars().collect();
            if chars.len() <= head + tail {
                return value.to_string();
            }
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

#[async_trait]
impl DataMask for AesDataMask {
    async fn encrypt(&self, plain: &str) -> Result<String, anyhow::Error> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plain.as_bytes())
            .map_err(|e| anyhow::anyhow!("加密失败: {}", e))?;
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(base64::engine::general_purpose::STANDARD.encode(&combined))
    }

    async fn decrypt(&self, encoded: &str) -> Result<String, anyhow::Error> {
        let combined = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| anyhow::anyhow!("密文解码失败: {}", e))?;
        if combined.len() < NONCE_SIZE {
            return Err(anyhow::anyhow!("密文格式无效: 长度不足"));
        }
        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("解密失败: 密钥不匹配或密文损坏"))?;
        Ok(String::from_utf8(plaintext)?)
    }

    fn mask_display(&self, value: &str, mask_type: MaskType) -> String {
        apply_mask(value, mask_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encrypt_decrypt_roundtrip() {
        let key = AesDataMask::generate_key();
        let mask = AesDataMask::new(&key);
        let original = "这是一条敏感信息，包含身份证号";
        let encrypted = mask.encrypt(original).await.unwrap();
        assert_ne!(encrypted, original);
        let decrypted = mask.decrypt(&encrypted).await.unwrap();
        assert_eq!(decrypted, original);
    }

    #[tokio::test]
    async fn test_different_ciphertext_each_time() {
        let key = AesDataMask::generate_key();
        let mask = AesDataMask::new(&key);
        let plain = "同一段明文";
        let c1 = mask.encrypt(plain).await.unwrap();
        let c2 = mask.encrypt(plain).await.unwrap();
        assert_ne!(c1, c2, "每次加密应产生不同密文（随机 nonce）");
    }

    #[tokio::test]
    async fn test_wrong_key_fails() {
        let key1 = AesDataMask::generate_key();
        let key2 = AesDataMask::generate_key();
        let mask1 = AesDataMask::new(&key1);
        let mask2 = AesDataMask::new(&key2);
        let encrypted = mask1.encrypt("secret").await.unwrap();
        assert!(mask2.decrypt(&encrypted).await.is_err());
    }

    #[tokio::test]
    async fn test_invalid_base64_fails() {
        let key = AesDataMask::generate_key();
        let mask = AesDataMask::new(&key);
        assert!(mask.decrypt("not-valid-base64!!!").await.is_err());
    }

    #[tokio::test]
    async fn test_too_short_ciphertext_fails() {
        let key = AesDataMask::generate_key();
        let mask = AesDataMask::new(&key);
        let short = base64::engine::general_purpose::STANDARD.encode(b"short");
        assert!(mask.decrypt(&short).await.is_err());
    }

    #[test]
    fn test_mask_phone() {
        let key = AesDataMask::generate_key();
        let mask = AesDataMask::new(&key);
        assert_eq!(mask.mask_display("13812345678", MaskType::Phone), "138****5678");
    }

    #[test]
    fn test_mask_email() {
        let key = AesDataMask::generate_key();
        let mask = AesDataMask::new(&key);
        assert_eq!(
            mask.mask_display("user@example.com", MaskType::Email),
            "u****@example.com"
        );
    }

    #[test]
    fn test_mask_bank_card() {
        let key = AesDataMask::generate_key();
        let mask = AesDataMask::new(&key);
        assert_eq!(
            mask.mask_display("6222021234561234", MaskType::BankCard),
            "****1234"
        );
    }

    #[test]
    fn test_mask_custom() {
        let key = AesDataMask::generate_key();
        let mask = AesDataMask::new(&key);
        assert_eq!(
            mask.mask_display("ABCDEFGHIJ", MaskType::Custom { head: 2, tail: 3 }),
            "AB*****HIJ"
        );
    }

    #[test]
    fn test_generate_key_produces_different_keys() {
        let k1 = AesDataMask::generate_key();
        let k2 = AesDataMask::generate_key();
        assert_ne!(k1, k2);
    }
}
