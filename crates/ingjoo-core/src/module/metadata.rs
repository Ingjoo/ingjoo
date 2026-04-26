//! 元数据系统 — 菜单、视图、动作描述符
//!
//! 为前端提供动态导航、视图渲染和动作绑定的元数据。

use serde::{Deserialize, Serialize};

// ==================== 枚举 ====================

/// 视图类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewType {
    Form,
    List,
    Kanban,
    Search,
    Graph,
    Calendar,
}

impl ViewType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Form => "form",
            Self::List => "list",
            Self::Kanban => "kanban",
            Self::Search => "search",
            Self::Graph => "graph",
            Self::Calendar => "calendar",
        }
    }
}

/// 动作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    ActWindow,
    ActUrl,
    ActServer,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ActWindow => "act_window",
            Self::ActUrl => "act_url",
            Self::ActServer => "act_server",
        }
    }
}

// ==================== Menu ====================

/// 菜单描述符
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuDescriptor {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default = "default_sequence")]
    pub sequence: i32,
    #[serde(default)]
    pub action_id: Option<String>,
    #[serde(default)]
    pub web_icon: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
    /// 可见性组 — 空数组表示所有人可见
    #[serde(default)]
    pub group_ids: Vec<String>,
    /// 子菜单（运行时填充，不持久化）
    #[serde(default)]
    pub children: Vec<MenuDescriptor>,
}

fn default_sequence() -> i32 {
    10
}
fn default_true() -> bool {
    true
}

// ==================== View ====================

/// 视图描述符
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDescriptor {
    pub id: String,
    pub name: String,
    pub model: String,
    #[serde(rename = "type")]
    pub view_type: ViewType,
    #[serde(default = "default_priority")]
    pub priority: i32,
    /// JSON 架构定义（serde_json::Value）
    pub arch: serde_json::Value,
    #[serde(default)]
    pub inherit_id: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub group_ids: Vec<String>,
}

fn default_priority() -> i32 {
    16
}

// ==================== Action ====================

/// 动作描述符
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDescriptor {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub action_type: ActionType,

    // -- act_window 字段 --
    #[serde(default)]
    pub res_model: Option<String>,
    #[serde(default)]
    pub view_mode: Vec<ViewType>,
    #[serde(default)]
    pub view_ids: Vec<String>,
    /// Domain DSL JSON（与 ingjoo_core::Domain 兼容）
    #[serde(default)]
    pub domain: Option<serde_json::Value>,
    /// JSON 上下文
    #[serde(default)]
    pub context: Option<serde_json::Value>,
    #[serde(default)]
    pub limit: Option<i32>,
    /// "current" | "new" | "fullscreen"
    #[serde(default = "default_target")]
    pub target: Option<String>,
    #[serde(default)]
    pub search_view_id: Option<String>,

    // -- act_url 字段 --
    #[serde(default)]
    pub url: Option<String>,

    // -- 通用字段 --
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub group_ids: Vec<String>,
}

fn default_target() -> Option<String> {
    Some("current".to_string())
}
