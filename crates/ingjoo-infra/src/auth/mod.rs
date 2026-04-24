use anyhow::{anyhow, Result};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

pub struct AuthConfig {
    pub secret: String,
    pub access_ttl: i64,
    pub refresh_ttl: i64,
}

impl AuthConfig {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            access_ttl: 900,
            refresh_ttl: 604800,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub groups: Vec<String>,
    pub exp: usize,
    pub iat: usize,
}

pub trait AuthProvider: Send + Sync {
    fn hash_password(&self, password: &str) -> Result<String>;
    fn verify_password(&self, password: &str, hash: &str) -> Result<bool>;
    fn create_access_token(&self, user_id: &str, role: &str, groups: &[String]) -> Result<String>;
    fn create_refresh_token(&self) -> String;
    fn refresh_token_hash(&self, token: &str) -> String;
    fn refresh_expires_at(&self) -> String;
    fn verify_access_token(&self, token: &str) -> Result<TokenClaims>;
}

pub struct JwtAuthProvider {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_ttl: i64,
    refresh_ttl: i64,
}

impl JwtAuthProvider {
    pub fn new(config: &AuthConfig) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(config.secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(config.secret.as_bytes()),
            access_ttl: config.access_ttl,
            refresh_ttl: config.refresh_ttl,
        }
    }
}

impl AuthProvider for JwtAuthProvider {
    fn hash_password(&self, password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("密码哈希失败: {}", e))?;
        Ok(hash.to_string())
    }

    fn verify_password(&self, password: &str, hash: &str) -> Result<bool> {
        let parsed = PasswordHash::new(hash).map_err(|e| anyhow!("哈希格式错误: {}", e))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    fn create_access_token(&self, user_id: &str, role: &str, groups: &[String]) -> Result<String> {
        let now = chrono::Utc::now().timestamp();
        let claims = TokenClaims {
            sub: user_id.to_string(),
            role: role.to_string(),
            groups: groups.to_vec(),
            exp: (now + self.access_ttl) as usize,
            iat: now as usize,
        };
        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| anyhow!("签发 access_token 失败: {}", e))
    }

    fn create_refresh_token(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn refresh_token_hash(&self, token: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn refresh_expires_at(&self) -> String {
        let exp = chrono::Utc::now() + chrono::Duration::seconds(self.refresh_ttl);
        exp.to_rfc3339()
    }

    fn verify_access_token(&self, token: &str) -> Result<TokenClaims> {
        let data = decode::<TokenClaims>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| anyhow!("token 无效: {}", e))?;
        Ok(data.claims)
    }
}

pub type AuthUtil = JwtAuthProvider;

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_auth() -> JwtAuthProvider {
        JwtAuthProvider::new(&AuthConfig::new("test-secret-key-for-unit-tests"))
    }

    // ==================== 密码哈希 ====================

    #[test]
    fn test_password_hash_and_verify_correct() {
        let auth = setup_auth();
        let hash = auth.hash_password("my-secret-password").unwrap();
        assert!(auth.verify_password("my-secret-password", &hash).unwrap());
    }

    #[test]
    fn test_password_verify_wrong_password() {
        let auth = setup_auth();
        let hash = auth.hash_password("correct-password").unwrap();
        assert!(!auth.verify_password("wrong-password", &hash).unwrap());
    }

    #[test]
    fn test_password_hash_is_different_each_time() {
        let auth = setup_auth();
        let hash1 = auth.hash_password("same-password").unwrap();
        let hash2 = auth.hash_password("same-password").unwrap();
        assert_ne!(hash1, hash2);
        assert!(auth.verify_password("same-password", &hash1).unwrap());
        assert!(auth.verify_password("same-password", &hash2).unwrap());
    }

    #[test]
    fn test_password_verify_invalid_hash_format() {
        let auth = setup_auth();
        let result = auth.verify_password("password", "not-a-valid-hash");
        assert!(result.is_err());
    }

    // ==================== Access Token ====================

    #[test]
    fn test_access_token_roundtrip() {
        let auth = setup_auth();
        let token = auth.create_access_token("user-123", "admin", &[]).unwrap();
        let claims = auth.verify_access_token(&token).unwrap();
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_access_token_role_field_roundtrip() {
        let auth = setup_auth();
        let token = auth.create_access_token("user-456", "editor", &[]).unwrap();
        let claims = auth.verify_access_token(&token).unwrap();
        assert_eq!(claims.role, "editor");
    }

    #[test]
    fn test_access_token_contains_iat_and_exp() {
        let auth = setup_auth();
        let token = auth.create_access_token("user-789", "user", &[]).unwrap();
        let claims = auth.verify_access_token(&token).unwrap();
        assert!(claims.exp > 0);
        assert!(claims.iat > 0);
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_access_token_tampered_fails() {
        let auth = setup_auth();
        let token = auth.create_access_token("user-123", "user", &[]).unwrap();
        let tampered = format!("{}tampered", &token[..token.len() - 10]);
        let result = auth.verify_access_token(&tampered);
        assert!(result.is_err());
    }

    #[test]
    fn test_access_token_wrong_secret_fails() {
        let auth1 = JwtAuthProvider::new(&AuthConfig::new("secret-1"));
        let auth2 = JwtAuthProvider::new(&AuthConfig::new("secret-2"));
        let token = auth1.create_access_token("user-123", "user", &[]).unwrap();
        let result = auth2.verify_access_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_access_token_empty_string_fails() {
        let auth = setup_auth();
        let result = auth.verify_access_token("");
        assert!(result.is_err());
    }

    #[test]
    fn test_access_token_garbage_fails() {
        let auth = setup_auth();
        let result = auth.verify_access_token("this.is.not.jwt");
        assert!(result.is_err());
    }

    // ==================== Refresh Token ====================

    #[test]
    fn test_refresh_token_uniqueness() {
        let auth = setup_auth();
        let token1 = auth.create_refresh_token();
        let token2 = auth.create_refresh_token();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_refresh_token_hash_deterministic() {
        let auth = setup_auth();
        let token = "some-refresh-token-value";
        let hash1 = auth.refresh_token_hash(token);
        let hash2 = auth.refresh_token_hash(token);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_refresh_token_hash_different_tokens() {
        let auth = setup_auth();
        let hash1 = auth.refresh_token_hash("token-a");
        let hash2 = auth.refresh_token_hash("token-b");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_refresh_expires_at_future() {
        let auth = setup_auth();
        let expires = auth.refresh_expires_at();
        let expires_time = chrono::DateTime::parse_from_rfc3339(&expires);
        assert!(expires_time.is_ok(), "应该是有效的 RFC3339 格式");
        let now = chrono::Utc::now();
        assert!(expires_time.unwrap() > now, "过期时间应该在未来");
    }

    #[test]
    fn test_refresh_expires_at_seven_days() {
        let auth = setup_auth();
        let expires = auth.refresh_expires_at();
        let expires_time = chrono::DateTime::parse_from_rfc3339(&expires).unwrap();
        let now = chrono::Utc::now();
        let diff = expires_time.timestamp() - now.timestamp();
        assert!((diff - 604800).abs() < 5, "refresh TTL should be ~7 days");
    }

    #[test]
    fn test_custom_ttl_config() {
        let config = AuthConfig {
            secret: "test".to_string(),
            access_ttl: 60,
            refresh_ttl: 3600,
        };
        let auth = JwtAuthProvider::new(&config);
        let token = auth.create_access_token("user-1", "user", &[]).unwrap();
        let claims = auth.verify_access_token(&token).unwrap();
        let ttl = (claims.exp - claims.iat) as i64;
        assert!((ttl - 60).abs() <= 2, "access TTL should match config");
    }
}
