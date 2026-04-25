//! 状态机 — 基于 ir_state_machine / ir_state_transition 表的状态管理

use async_trait::async_trait;
use ingjoo_core::extension::state_machine::{StateMachine, StateTransition, TransitionError};
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use sqlx::Row as _;
use std::collections::HashMap;

pub struct DbStateMachine {
    pool: Pool,
    dialect: Dialect,
}

impl DbStateMachine {
    pub fn new(pool: Pool, dialect: Dialect) -> Self {
        Self { pool, dialect }
    }

    fn sql(&self, query: &str) -> String {
        self.dialect.prepare(query)
    }
}

#[async_trait]
impl StateMachine for DbStateMachine {
    async fn get_current_state(
        &self,
        model: &str,
        record_id: &str,
    ) -> Result<String, TransitionError> {
        let row = sqlx::query(&self.sql(
            "SELECT current_state FROM ir_state_record WHERE model = ? AND record_id = ?"
        ))
        .bind(model)
        .bind(record_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TransitionError::Internal(e.into()))?;

        match row {
            Some(r) => Ok(r.get("current_state")),
            None => {
                let machine_row = sqlx::query(&self.sql(
                    "SELECT initial_state FROM ir_state_machine WHERE model = ?"
                ))
                .bind(model)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| TransitionError::Internal(e.into()))?;

                match machine_row {
                    Some(r) => {
                        let initial: String = r.get("initial_state");
                        sqlx::query(&self.sql(
                            "INSERT INTO ir_state_record (model, record_id, current_state) VALUES (?, ?, ?)"
                        ))
                        .bind(model)
                        .bind(record_id)
                        .bind(&initial)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| TransitionError::Internal(e.into()))?;
                        Ok(initial)
                    }
                    None => Err(TransitionError::MachineNotFound(model.to_string())),
                }
            }
        }
    }

    async fn get_available_transitions(
        &self,
        model: &str,
        record_id: &str,
    ) -> Result<Vec<StateTransition>, TransitionError> {
        let current = self.get_current_state(model, record_id).await?;
        let rows = sqlx::query(&self.sql(
            "SELECT from_state, to_state, label FROM ir_state_transition WHERE model = ? AND from_state = ?"
        ))
        .bind(model)
        .bind(&current)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TransitionError::Internal(e.into()))?;

        Ok(rows
            .iter()
            .map(|r| StateTransition {
                from: r.get("from_state"),
                to: r.get("to_state"),
                label: r.get("label"),
            })
            .collect())
    }

    async fn transition(
        &self,
        model: &str,
        record_id: &str,
        target_state: &str,
        _context: HashMap<String, serde_json::Value>,
    ) -> Result<String, TransitionError> {
        let current = self.get_current_state(model, record_id).await?;

        let row = sqlx::query(&self.sql(
            "SELECT COUNT(*) as cnt FROM ir_state_transition WHERE model = ? AND from_state = ? AND to_state = ?"
        ))
        .bind(model)
        .bind(&current)
        .bind(target_state)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| TransitionError::Internal(e.into()))?;

        let count: i64 = row.get("cnt");
        if count == 0 {
            return Err(TransitionError::InvalidTransition(format!(
                "{} -> {} (model: {})",
                current, target_state, model
            )));
        }

        sqlx::query(&self.sql(
            "UPDATE ir_state_record SET current_state = ? WHERE model = ? AND record_id = ?"
        ))
        .bind(target_state)
        .bind(model)
        .bind(record_id)
        .execute(&self.pool)
        .await
        .map_err(|e| TransitionError::Internal(e.into()))?;

        Ok(target_state.to_string())
    }

    async fn register_machine(
        &self,
        model: &str,
        states: Vec<String>,
        transitions: Vec<StateTransition>,
        initial_state: String,
    ) -> Result<(), TransitionError> {
        let states_json = serde_json::to_string(&states).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(&self.sql(
            "INSERT INTO ir_state_machine (model, states, initial_state) \
             VALUES (?, ?, ?) \
             ON CONFLICT(model) DO UPDATE SET states = excluded.states, initial_state = excluded.initial_state"
        ))
        .bind(model)
        .bind(&states_json)
        .bind(&initial_state)
        .execute(&self.pool)
        .await
        .map_err(|e| TransitionError::Internal(e.into()))?;

        sqlx::query(&self.sql(
            "DELETE FROM ir_state_transition WHERE model = ?"
        ))
        .bind(model)
        .execute(&self.pool)
        .await
        .map_err(|e| TransitionError::Internal(e.into()))?;

        for t in &transitions {
            sqlx::query(&self.sql(
                "INSERT INTO ir_state_transition (model, from_state, to_state, label) VALUES (?, ?, ?, ?)"
            ))
            .bind(model)
            .bind(&t.from)
            .bind(&t.to)
            .bind(&t.label)
            .execute(&self.pool)
            .await
            .map_err(|e| TransitionError::Internal(e.into()))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_state_machine_constructs() {
        let pool = sqlx::AnyPool::connect_lazy("sqlite::memory:").unwrap();
        let _sm = DbStateMachine::new(pool.into(), Dialect::Sqlite);
    }
}
