use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 提示词模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub key: String,
    pub template: String,
    pub description: Option<String>,
}

/// 提示词模板管理器 — 基于 module_settings 的级联配置存储模板
/// 键格式: prompt:<template_key>，值为带 {{variable}} 占位符的文本
pub struct PromptManager {
    // 注意：这里不用 Arc<dyn ModuleSettingStore> 因为 core 不直接依赖具体的 store
    // 实际实现会在 ingjoo-infra 中注入
}

impl PromptManager {
    pub fn new() -> Self {
        Self {}
    }

    /// 渲染模板 — 将变量替换到模板中
    pub fn render(
        &self,
        template: &str,
        vars: &HashMap<String, String>,
    ) -> Result<String, anyhow::Error> {
        let mut result = template.to_string();
        for (key, value) in vars {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        Ok(result)
    }
}

impl Default for PromptManager {
    fn default() -> Self {
        Self::new()
    }
}
