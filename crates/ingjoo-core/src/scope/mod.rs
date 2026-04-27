//! 作用域权限 — 角色层级与访问守卫

pub mod guard;

pub use guard::{role_gte, ScopeError, ScopeGuard};

/// 四级角色层级（owner > admin > member > viewer）
pub const SCOPE_HIERARCHY_4: &[&str] = &["owner", "admin", "member", "viewer"];

/// 三级角色层级（owner > member > viewer，无 admin）
pub const SCOPE_HIERARCHY_3: &[&str] = &["owner", "member", "viewer"];
