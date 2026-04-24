use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 审计日志查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQuery {
    pub user_id: Option<String>,
    pub action: Option<String>,
    pub resource: Option<String>,
    pub resource_id: Option<String>,
    pub ip: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// 审计日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub user_id: Option<String>,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub detail: Option<serde_json::Value>,
    pub ip: Option<String>,
    pub created_at: String,
}

#[async_trait]
pub trait AuditStore: Send + Sync {
    /// 记录审计日志
    async fn create_audit_log(
        &self,
        user_id: Option<&str>,
        action: &str,
        resource: &str,
        resource_id: Option<&str>,
        detail: Option<serde_json::Value>,
        ip: Option<&str>,
    ) -> Result<AuditEntry, anyhow::Error>;

    /// 查询审计日志
    async fn list_audit_logs(&self, query: AuditQuery) -> Result<Vec<AuditEntry>, anyhow::Error>;

    /// 获取单条审计日志
    async fn get_audit_log(&self, id: &str) -> Result<Option<AuditEntry>, anyhow::Error>;
}
