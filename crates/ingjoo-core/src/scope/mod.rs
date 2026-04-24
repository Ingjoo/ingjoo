pub mod guard;

pub use guard::{ScopeGuard, ScopeError, role_gte};

pub const SCOPE_HIERARCHY_4: &[&str] = &["owner", "admin", "member", "viewer"];
pub const SCOPE_HIERARCHY_3: &[&str] = &["owner", "member", "viewer"];
