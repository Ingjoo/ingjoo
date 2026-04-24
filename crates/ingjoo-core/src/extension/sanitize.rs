use async_trait::async_trait;

/// 输入净化（XSS 防护）
#[async_trait]
pub trait InputSanitizer: Send + Sync {
    /// 净化 HTML — 移除非白名单标签，保留安全内容
    async fn sanitize_html(&self, html: &str) -> Result<String, anyhow::Error>;

    /// 净化纯文本 — 转义 HTML 实体（如 &lt; &gt; &amp;）
    async fn sanitize_plain_text(&self, text: &str) -> Result<String, anyhow::Error>;
}
