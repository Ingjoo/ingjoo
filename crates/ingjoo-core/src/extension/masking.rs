use async_trait::async_trait;

/// 脱敏类型
#[derive(Debug, Clone, Copy)]
pub enum MaskType {
    /// 手机号: 138****1234
    Phone,
    /// 身份证: 110***********1234
    IdCard,
    /// 邮箱: a****@example.com
    Email,
    /// 银行卡: **** **** **** 1234
    BankCard,
    /// 自定义（保留首尾各 n 字符）
    Custom { head: usize, tail: usize },
}

/// 数据脱敏与加密存储
#[async_trait]
pub trait DataMask: Send + Sync {
    /// 加密 — 对明文进行加密
    async fn encrypt(&self, plain: &str) -> Result<String, anyhow::Error>;

    /// 解密 — 对密文进行解密
    async fn decrypt(&self, cipher: &str) -> Result<String, anyhow::Error>;

    /// 脱敏展示 — 生成前端展示用的脱敏字符串（如 138****1234）
    fn mask_display(&self, value: &str, mask_type: MaskType) -> String;
}
