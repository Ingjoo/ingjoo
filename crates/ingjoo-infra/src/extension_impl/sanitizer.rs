//! 输入净化器 — HTML 标签白名单过滤 + 实体转义

use async_trait::async_trait;
use ingjoo_core::extension::sanitize::InputSanitizer;

/// HTML 标签白名单净化器
///
/// `sanitize_html`: 只保留白名单标签，移除危险属性（onclick/onerror 等）
/// `sanitize_plain_text`: 转义 HTML 实体
pub struct HtmlInputSanitizer {
    allowed_tags: Vec<&'static str>,
}

impl HtmlInputSanitizer {
    pub fn new(allowed_tags: Vec<&'static str>) -> Self {
        Self { allowed_tags }
    }

    pub fn with_defaults() -> Self {
        Self::new(vec![
            "p", "br", "b", "i", "u", "em", "strong", "a", "ul", "ol", "li",
            "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "code", "pre",
            "span", "div", "table", "tr", "td", "th", "thead", "tbody",
        ])
    }

    fn is_allowed(&self, tag: &str) -> bool {
        self.allowed_tags.contains(&tag)
    }
}

impl Default for HtmlInputSanitizer {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[async_trait]
impl InputSanitizer for HtmlInputSanitizer {
    async fn sanitize_html(&self, html: &str) -> Result<String, anyhow::Error> {
        let mut result = String::with_capacity(html.len());
        let chars: Vec<char> = html.chars().collect();
        let len = chars.len();
        let mut i = 0;
        let mut suppress_tag: Option<String> = None;

        while i < len {
            if chars[i] == '<' {
                let tag_end = find_tag_end(&chars, i + 1);
                if tag_end.is_none() {
                    if suppress_tag.is_none() {
                        result.push_str("&lt;");
                    }
                    i += 1;
                    continue;
                }
                let end = tag_end.unwrap();
                let tag_content = slice_chars(&chars, i + 1, end.min(len));

                if let Some(stripped) = tag_content.strip_prefix('/') {
                    let tag_name = extract_tag_name(stripped);
                    if suppress_tag.as_deref() == Some(tag_name) {
                        suppress_tag = None;
                    } else if suppress_tag.is_none() && self.is_allowed(tag_name) {
                        result.push('<');
                        result.push('/');
                        result.push_str(tag_name);
                        result.push('>');
                    }
                } else if tag_content.starts_with('!') {
                } else {
                    let tag_name = extract_tag_name(&tag_content);
                    if self.is_allowed(tag_name) {
                        if suppress_tag.is_none() {
                            let safe_attrs = strip_dangerous_attrs(&tag_content[tag_name.len()..]);
                            result.push('<');
                            result.push_str(tag_name);
                            if !safe_attrs.is_empty() {
                                result.push(' ');
                                result.push_str(&safe_attrs);
                            }
                            result.push('>');
                        }
                    } else {
                        suppress_tag = Some(tag_name.to_string());
                    }
                }
                i = end + 1;
            } else {
                if suppress_tag.is_none() {
                    result.push(chars[i]);
                }
                i += 1;
            }
        }

        Ok(result)
    }

    async fn sanitize_plain_text(&self, text: &str) -> Result<String, anyhow::Error> {
        Ok(text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;"))
    }
}

fn find_tag_end(chars: &[char], start: usize) -> Option<usize> {
    let mut in_attr = false;
    for (i, &ch) in chars.iter().enumerate().skip(start) {
        match ch {
            '"' => in_attr = !in_attr,
            '>' if !in_attr => return Some(i),
            _ => {}
        }
    }
    None
}

fn slice_chars(chars: &[char], start: usize, end: usize) -> String {
    chars[start..end].iter().collect()
}

fn extract_tag_name(content: &str) -> &str {
    content
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or("")
}

fn strip_dangerous_attrs(attr_str: &str) -> String {
    let lower = attr_str.to_lowercase();
    if lower.contains("onclick")
        || lower.contains("onerror")
        || lower.contains("onload")
        || lower.contains("onmouseover")
        || lower.contains("onfocus")
        || lower.contains("onblur")
        || lower.contains("javascript:")
    {
        return String::new();
    }

    if lower.contains("href") && lower.contains("javascript:") {
        return String::new();
    }

    attr_str.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn plain_text_escapes_entities() {
        let s = HtmlInputSanitizer::default();
        let result = s.sanitize_plain_text("<script>alert('xss')</script>").await.unwrap();
        assert_eq!(result, "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;");
    }

    #[tokio::test]
    async fn plain_text_escapes_ampersand() {
        let s = HtmlInputSanitizer::default();
        let result = s.sanitize_plain_text("a & b < c").await.unwrap();
        assert_eq!(result, "a &amp; b &lt; c");
    }

    #[tokio::test]
    async fn html_allows_whitelisted_tags() {
        let s = HtmlInputSanitizer::default();
        let result = s.sanitize_html("<p>Hello <b>world</b></p>").await.unwrap();
        assert_eq!(result, "<p>Hello <b>world</b></p>");
    }

    #[tokio::test]
    async fn html_strips_dangerous_tags() {
        let s = HtmlInputSanitizer::default();
        let result = s.sanitize_html("<script>alert('xss')</script><p>safe</p>").await.unwrap();
        assert_eq!(result, "<p>safe</p>");
    }

    #[tokio::test]
    async fn html_strips_onclick_attrs() {
        let s = HtmlInputSanitizer::default();
        let result = s.sanitize_html("<p onclick=\"evil()\">text</p>").await.unwrap();
        assert_eq!(result, "<p>text</p>");
    }

    #[tokio::test]
    async fn html_strips_unclosed_angle() {
        let s = HtmlInputSanitizer::default();
        let result = s.sanitize_html("a < b > c").await.unwrap();
        assert!(result.contains("&lt;") || !result.contains("< b"));
    }

    #[tokio::test]
    async fn custom_tag_whitelist() {
        let s = HtmlInputSanitizer::new(vec!["p"]);
        let result = s.sanitize_html("<p>ok</p><b>gone</b>").await.unwrap();
        assert!(result.contains("<p>ok</p>"));
        assert!(!result.contains("<b>"));
    }

    #[tokio::test]
    async fn default_sanitizer_constructs() {
        let s = HtmlInputSanitizer::default();
        assert!(!s.allowed_tags.is_empty());
    }
}
