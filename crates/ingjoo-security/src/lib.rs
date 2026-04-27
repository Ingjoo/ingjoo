//! 三层权限引擎 — 模型级、记录级、集合级的统一安全策略
//!
//! 灵感源自 Odoo 的 `ir.model.access` + `ir.rule` 体系，简化为三层架构：
//!
//! - **Layer 1 — 模型级**: 角色 × 实体 × 操作（CRUD）矩阵，控制"能不能碰"
//! - **Layer 2 — 记录级**: Domain 过滤自动注入 WHERE 子句，控制"能碰哪些行"
//! - **Layer 3 — 集合级**: 多租户场景下按 `collection_id` 自动隔离
//!
//! 典型用法：通过 [`SecurityBuilder`] 从 JSON 构建 [`SecurityPolicy`]，
//! 然后在 CRUD 处理器中调用各层检查方法。

pub mod policy;

pub use policy::{
    AccessOp, ModelAccess, ModelAccessDef, PolicyDocument, RecordRule, RecordRuleDef, SecurityBuilder, SecurityError,
    SecurityPolicy,
};
