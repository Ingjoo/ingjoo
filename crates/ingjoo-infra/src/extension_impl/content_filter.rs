//! 关键词内容过滤器 — 基于关键词列表的文本/文件安全过滤

use async_trait::async_trait;
use ingjoo_core::extension::content_filter::{ContentFilter, FilterResult};
use std::collections::HashMap;

/// 基于关键词列表的内容过滤器
///
/// 关键词按类别分组，每个类别可独立配置。匹配时对文本进行扫描，
/// 若命中任何关键词则返回 `passed=false` 并附上命中的类别和原因。
/// 基于关键词的内容过滤实现
pub struct KeywordContentFilter {
    categories: HashMap<String, CategoryConfig>,
}

/// 单个过滤类别的配置
/// 内容分类配置（关键词列表 + 严重级别）
pub struct CategoryConfig {
    pub keywords: Vec<String>,
    pub description: String,
    pub case_sensitive: bool,
}

impl KeywordContentFilter {
    pub fn new() -> Self {
        Self {
            categories: HashMap::new(),
        }
    }

    /// 添加过滤类别
    pub fn add_category(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        keywords: Vec<String>,
        case_sensitive: bool,
    ) {
        self.categories.insert(
            name.into(),
            CategoryConfig {
                keywords,
                description: description.into(),
                case_sensitive,
            },
        );
    }

    /// 创建带默认敏感词的过滤器
    pub fn with_defaults() -> Self {
        let mut filter = Self::new();
        filter.add_category(
            "profanity",
            "粗俗语言",
            vec!["damn".to_string(), "hell".to_string()],
            false,
        );
        filter.add_category(
            "dangerous",
            "危险内容",
            vec![
                "炸弹制作".to_string(),
                "毒品合成".to_string(),
                "hack password".to_string(),
            ],
            false,
        );
        filter
    }

    fn check_text_inner(&self, text: &str) -> FilterResult {
        for (category, config) in &self.categories {
            for keyword in &config.keywords {
                let hit = if config.case_sensitive {
                    text.contains(keyword.as_str())
                } else {
                    text.to_lowercase()
                        .contains(&keyword.to_lowercase())
                };
                if hit {
                    return FilterResult {
                        passed: false,
                        reason: Some(format!("命中关键词: {}", keyword)),
                        category: Some(category.clone()),
                    };
                }
            }
        }
        FilterResult {
            passed: true,
            reason: None,
            category: None,
        }
    }
}

impl Default for KeywordContentFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentFilter for KeywordContentFilter {
    async fn check_text(&self, text: &str) -> Result<FilterResult, anyhow::Error> {
        Ok(self.check_text_inner(text))
    }

    async fn check_file(
        &self,
        data: &[u8],
        file_type: &str,
    ) -> Result<FilterResult, anyhow::Error> {
        let text_content = match file_type {
            "txt" | "csv" | "json" | "xml" | "html" | "md" => {
                String::from_utf8_lossy(data).into_owned()
            }
            _ => {
                return Ok(FilterResult {
                    passed: true,
                    reason: Some(format!("二进制文件类型 {} 不执行文本过滤", file_type)),
                    category: None,
                });
            }
        };
        Ok(self.check_text_inner(&text_content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clean_text_passes() {
        let filter = KeywordContentFilter::with_defaults();
        let result = filter.check_text("今天天气不错").await.unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_profanity_detected() {
        let filter = KeywordContentFilter::with_defaults();
        let result = filter.check_text("What the damn hell").await.unwrap();
        assert!(!result.passed);
        assert_eq!(result.category.as_deref(), Some("profanity"));
        assert!(result.reason.unwrap().contains("damn"));
    }

    #[tokio::test]
    async fn test_case_insensitive_matching() {
        let filter = KeywordContentFilter::with_defaults();
        let result = filter.check_text("DAMN this is bad").await.unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_custom_category() {
        let mut filter = KeywordContentFilter::new();
        filter.add_category(
            "pii",
            "个人隐私信息",
            vec!["身份证号".to_string(), "银行卡号".to_string()],
            false,
        );
        let result = filter
            .check_text("请提供您的身份证号")
            .await
            .unwrap();
        assert!(!result.passed);
        assert_eq!(result.category.as_deref(), Some("pii"));
    }

    #[tokio::test]
    async fn test_binary_file_skipped() {
        let filter = KeywordContentFilter::with_defaults();
        let result = filter.check_file(&[0xFF, 0xD8, 0xFF], "png").await.unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_text_file_scanned() {
        let filter = KeywordContentFilter::with_defaults();
        let data = b"What the damn is this";
        let result = filter.check_file(data, "txt").await.unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_empty_filter_passes_all() {
        let filter = KeywordContentFilter::new();
        let result = filter.check_text_inner("anything goes");
        assert!(result.passed);
    }
}
