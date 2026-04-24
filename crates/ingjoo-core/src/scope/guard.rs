use anyhow::Result;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScopeError {
    #[error("resource not found")]
    NotFound,
    #[error("access denied: insufficient role")]
    Forbidden,
}

pub fn role_gte(actual: &str, required: &str, hierarchy: &[&str]) -> bool {
    let actual_rank = hierarchy.iter().position(|r| *r == actual);
    let required_rank = hierarchy.iter().position(|r| *r == required);
    match (actual_rank, required_rank) {
        (Some(a), Some(r)) => a <= r,
        _ => false,
    }
}

#[async_trait]
pub trait ScopeGuard: Send + Sync {
    type Member: Send + Sync + Clone;

    async fn check_access(
        &self,
        scope_id: &str,
        user_id: &str,
        min_role: &str,
    ) -> Result<Self::Member, ScopeError>;

    async fn invalidate(&self, scope_id: &str, user_id: Option<&str>);
}

#[cfg(test)]
mod tests {
    use super::*;

    const H4: &[&str] = &["owner", "admin", "member", "viewer"];
    const H3: &[&str] = &["owner", "member", "viewer"];

    #[test]
    fn owner_beats_all() {
        assert!(role_gte("owner", "admin", H4));
        assert!(role_gte("owner", "member", H4));
        assert!(role_gte("owner", "viewer", H4));
    }

    #[test]
    fn admin_beats_member_viewer() {
        assert!(!role_gte("admin", "owner", H4));
        assert!(role_gte("admin", "member", H4));
        assert!(role_gte("admin", "viewer", H4));
    }

    #[test]
    fn viewer_cannot_write() {
        assert!(!role_gte("viewer", "member", H4));
        assert!(!role_gte("viewer", "admin", H4));
    }

    #[test]
    fn equal_role_passes() {
        assert!(role_gte("member", "member", H4));
        assert!(role_gte("viewer", "viewer", H4));
    }

    #[test]
    fn unknown_role_fails() {
        assert!(!role_gte("guest", "viewer", H4));
        assert!(!role_gte("owner", "guest", H4));
    }

    #[test]
    fn hierarchy_3_no_admin() {
        assert!(role_gte("owner", "member", H3));
        assert!(role_gte("member", "viewer", H3));
        assert!(!role_gte("admin", "member", H3));
    }
}
