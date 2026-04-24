//! 状态机 — 模型状态转换的注册、查询和执行

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 状态转换定义
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub label: String,
}

/// 状态转换错误
#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    /// 转换不合法（从当前状态无法到达目标状态）
    #[error("无效的状态转换: {0}")]
    InvalidTransition(String),
    /// 找不到对应模型的状态机定义
    #[error("状态机未找到: {0}")]
    MachineNotFound(String),
    /// 转换前置条件不满足
    #[error("条件不满足: {0}")]
    ConditionNotMet(String),
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

/// 状态机 — 管理模型记录的生命周期状态和转换规则
#[async_trait]
pub trait StateMachine: Send + Sync {
    /// 获取记录当前状态
    async fn get_current_state(&self, model: &str, record_id: &str) -> Result<String, TransitionError>;

    /// 获取记录可用的转换列表
    async fn get_available_transitions(
        &self,
        model: &str,
        record_id: &str,
    ) -> Result<Vec<StateTransition>, TransitionError>;

    /// 执行状态转换，返回转换后的新状态
    async fn transition(
        &self,
        model: &str,
        record_id: &str,
        target_state: &str,
        context: HashMap<String, serde_json::Value>,
    ) -> Result<String, TransitionError>;

    /// 注册模型的状态机定义（状态列表、转换规则、初始状态）
    async fn register_machine(
        &self,
        model: &str,
        states: Vec<String>,
        transitions: Vec<StateTransition>,
        initial_state: String,
    ) -> Result<(), TransitionError>;
}
