use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    #[error("无效的状态转换: {0}")]
    InvalidTransition(String),
    #[error("状态机未找到: {0}")]
    MachineNotFound(String),
    #[error("条件不满足: {0}")]
    ConditionNotMet(String),
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

#[async_trait]
pub trait StateMachine: Send + Sync {
    async fn get_current_state(&self, model: &str, record_id: &str) -> Result<String, TransitionError>;

    async fn get_available_transitions(
        &self,
        model: &str,
        record_id: &str,
    ) -> Result<Vec<StateTransition>, TransitionError>;

    async fn transition(
        &self,
        model: &str,
        record_id: &str,
        target_state: &str,
        context: HashMap<String, serde_json::Value>,
    ) -> Result<String, TransitionError>;

    async fn register_machine(
        &self,
        model: &str,
        states: Vec<String>,
        transitions: Vec<StateTransition>,
        initial_state: String,
    ) -> Result<(), TransitionError>;
}
