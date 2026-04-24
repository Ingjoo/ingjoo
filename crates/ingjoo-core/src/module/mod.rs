//! 动态模型模块 — 模型注册表、路由与视图描述

pub mod metadata;
pub mod plugin;
pub mod registry;

pub use registry::{ModelDescriptor, FieldDescriptor, FieldType, IdType, ModelRegistry};
pub use metadata::{ViewType, ActionType, MenuDescriptor, ViewDescriptor, ActionDescriptor};
pub use plugin::{PluginManifest, PluginInfo, PluginState};

/// 模块路由注册 — 每个业务模块实现此 trait 以声明 HTTP 路由
pub trait ModuleRoutes: Send + Sync {
    /// 模块名称
    fn name(&self) -> &str;

    /// 返回模块所有路由描述
    fn route_descriptions(&self) -> Vec<RouteDescriptor>;
}

/// 路由描述 — 记录单个 HTTP 端点的元信息
pub struct RouteDescriptor {
    /// HTTP 方法（GET / POST / PUT / DELETE）
    pub method: &'static str,
    /// URL 路径
    pub path: &'static str,
    /// 处理函数标识
    pub handler: &'static str,
    /// 最低角色要求
    pub min_role: &'static str,
}
