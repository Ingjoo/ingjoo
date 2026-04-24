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
    pub exp: usize,
    pub iat: usize,
}

pub trait AuthProvider: Send + Sync {
    fn hash_password(&self, password: &str) -> Result<String>;
    fn verify_password(&self, password: &str, hash: &str) -> Result<bool>;
    fn create_access_token(&self, user_id: &str) -> Result<String>;
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

    fn create_access_token(&self, user_id: &str) -> Result<String> {
        let now = chrono::Utc::now().timestamp();
        let claims = TokenClaims {
            sub: user_id.to_string(),
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
