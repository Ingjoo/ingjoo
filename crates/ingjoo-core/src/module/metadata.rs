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

impl ViewDescriptor {
    /// 将 `arch` JSON 解析为结构化的 `ViewArch`
    pub fn parse_arch(&self) -> Result<Option<ViewArch>, serde_json::Error> {
        if self.arch.is_null() {
            return Ok(None);
        }
        match self.view_type {
            ViewType::List => {
                serde_json::from_value::<ListArch>(self.arch.clone())
                    .map(|a| Some(ViewArch::List(a)))
            }
            ViewType::Form => {
                serde_json::from_value::<FormArch>(self.arch.clone())
                    .map(|a| Some(ViewArch::Form(a)))
            }
            ViewType::Kanban => {
                serde_json::from_value::<KanbanArch>(self.arch.clone())
                    .map(|a| Some(ViewArch::Kanban(a)))
            }
            ViewType::Search => {
                serde_json::from_value::<SearchArch>(self.arch.clone())
                    .map(|a| Some(ViewArch::Search(a)))
            }
            _ => Ok(None),
        }
    }
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

// ==================== ViewArch — 视图架构 JSON 结构化类型 ====================

/// 列表视图列定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListColumn {
    pub field: String,
    pub label: String,
    #[serde(default)]
    pub width: Option<String>,
    #[serde(default)]
    pub sortable: Option<bool>,
    #[serde(default)]
    pub widget: Option<String>,
}

/// 列表视图架构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListArch {
    pub columns: Vec<ListColumn>,
    #[serde(default)]
    pub editable: Option<bool>,
}

/// 表单字段定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FormField {
    pub field: String,
    pub label: String,
    #[serde(default)]
    pub widget: Option<String>,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub readonly: Option<bool>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub colspan: Option<u32>,
}

/// 表单分组
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FormGroup {
    #[serde(default)]
    pub string: Option<String>,
    pub fields: Vec<FormField>,
    #[serde(default)]
    pub colspan: Option<u32>,
}

/// 表单按钮
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FormButton {
    pub name: String,
    pub label: String,
    pub action: String,
    #[serde(default, rename = "type")]
    pub button_type: Option<String>,
}

/// 表单底部
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FormFooter {
    pub buttons: Vec<FormButton>,
}

/// 表单视图架构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FormArch {
    pub groups: Vec<FormGroup>,
    #[serde(default)]
    pub footer: Option<FormFooter>,
}

/// 看板卡片字段
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct KanbanCardField {
    pub field: String,
    #[serde(default)]
    pub widget: Option<String>,
}

/// 看板卡片定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct KanbanCard {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    pub fields: Vec<KanbanCardField>,
    #[serde(default)]
    pub color: Option<String>,
}

/// 看板视图架构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct KanbanArch {
    pub group_by: String,
    pub card: KanbanCard,
    #[serde(default)]
    pub quick_create: Option<bool>,
}

/// 搜索字段
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SearchField {
    pub name: String,
    pub label: String,
    pub field_type: String,
    #[serde(default)]
    pub options: Option<Vec<(String, String)>>,
}

/// 搜索过滤器
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SearchFilter {
    pub name: String,
    pub label: String,
    pub domain: Vec<Vec<String>>,
}

/// 搜索分组
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SearchGroupBy {
    pub name: String,
    pub label: String,
}

/// 搜索视图架构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SearchArch {
    pub fields: Vec<SearchField>,
    #[serde(default)]
    pub filters: Vec<SearchFilter>,
    #[serde(default)]
    pub group_by: Vec<SearchGroupBy>,
}

/// 视图架构联合类型 — 根据 view_type 解析不同的 arch 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "arch")]
#[serde(rename_all = "snake_case")]
pub enum ViewArch {
    List(ListArch),
    Form(FormArch),
    Kanban(KanbanArch),
    Search(SearchArch),
}
