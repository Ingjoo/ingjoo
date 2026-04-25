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
    use ingjoo_core::extension::state_machine::StateMachine;

    async fn setup() -> DbStateMachine {
        let tmp = tempfile::Builder::new()
            .prefix("state_machine_test_")
            .suffix(".db")
            .tempfile()
            .unwrap();
        let db_path = tmp.path().to_str().unwrap().to_string();
        std::mem::forget(tmp);

        ingjoo_core::pool::install_drivers();
        let db_url = format!("sqlite://{}?mode=rwc", db_path);
        let (pool, dialect) = ingjoo_core::pool::connect_pool(&db_url).await.unwrap();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS ir_state_machine (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                model TEXT NOT NULL UNIQUE,
                states TEXT NOT NULL DEFAULT '[]',
                initial_state TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS ir_state_transition (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                model TEXT NOT NULL,
                from_state TEXT NOT NULL,
                to_state TEXT NOT NULL,
                label TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS ir_state_record (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                model TEXT NOT NULL,
                record_id TEXT NOT NULL,
                current_state TEXT NOT NULL,
                UNIQUE(model, record_id)
            )"
        )
        .execute(&pool)
        .await
        .unwrap();
        DbStateMachine::new(pool, dialect)
    }

    #[tokio::test]
    async fn test_db_state_machine_constructs() {
        let pool = sqlx::AnyPool::connect_lazy("sqlite::memory:").unwrap();
        let _sm = DbStateMachine::new(pool, Dialect::Sqlite);
    }

    #[tokio::test]
    async fn state_machine_register_and_get_initial() {
        let sm = setup().await;
        sm.register_machine(
            "order",
            vec!["draft".into(), "confirmed".into(), "done".into()],
            vec![StateTransition {
                from: "draft".into(), to: "confirmed".into(), label: "确认".into(),
            }],
            "draft".into(),
        ).await.unwrap();

        let state = sm.get_current_state("order", "O001").await.unwrap();
        assert_eq!(state, "draft");
    }

    #[tokio::test]
    async fn state_machine_transition_flow() {
        let sm = setup().await;
        sm.register_machine(
            "order",
            vec!["draft".into(), "confirmed".into(), "done".into(), "cancelled".into()],
            vec![
                StateTransition { from: "draft".into(), to: "confirmed".into(), label: "确认".into() },
                StateTransition { from: "confirmed".into(), to: "done".into(), label: "完成".into() },
                StateTransition { from: "draft".into(), to: "cancelled".into(), label: "取消".into() },
            ],
            "draft".into(),
        ).await.unwrap();

        let new = sm.transition("order", "O001", "confirmed", HashMap::new()).await.unwrap();
        assert_eq!(new, "confirmed");

        let current = sm.get_current_state("order", "O001").await.unwrap();
        assert_eq!(current, "confirmed");

        let new2 = sm.transition("order", "O001", "done", HashMap::new()).await.unwrap();
        assert_eq!(new2, "done");
    }

    #[tokio::test]
    async fn state_machine_invalid_transition_rejected() {
        let sm = setup().await;
        sm.register_machine(
            "order",
            vec!["draft".into(), "done".into()],
            vec![StateTransition { from: "draft".into(), to: "done".into(), label: "完成".into() }],
            "draft".into(),
        ).await.unwrap();

        sm.get_current_state("order", "O001").await.unwrap();
        let result = sm.transition("order", "O001", "cancelled", HashMap::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn state_machine_available_transitions() {
        let sm = setup().await;
        sm.register_machine(
            "order",
            vec!["draft".into(), "confirmed".into(), "cancelled".into()],
            vec![
                StateTransition { from: "draft".into(), to: "confirmed".into(), label: "确认".into() },
                StateTransition { from: "draft".into(), to: "cancelled".into(), label: "取消".into() },
            ],
            "draft".into(),
        ).await.unwrap();

        sm.get_current_state("order", "O001").await.unwrap();
        let transitions = sm.get_available_transitions("order", "O001").await.unwrap();
        assert_eq!(transitions.len(), 2);

        let labels: Vec<&str> = transitions.iter().map(|t| t.label.as_str()).collect();
        assert!(labels.contains(&"确认"));
        assert!(labels.contains(&"取消"));
    }

    #[tokio::test]
    async fn state_machine_machine_not_found() {
        let sm = setup().await;
        let result = sm.get_current_state("nonexistent", "O001").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn state_machine_register_idempotent() {
        let sm = setup().await;
        sm.register_machine(
            "order",
            vec!["draft".into(), "confirmed".into()],
            vec![StateTransition { from: "draft".into(), to: "confirmed".into(), label: "确认".into() }],
            "draft".into(),
        ).await.unwrap();

        sm.register_machine(
            "order",
            vec!["new".into(), "processed".into()],
            vec![StateTransition { from: "new".into(), to: "processed".into(), label: "处理".into() }],
            "new".into(),
        ).await.unwrap();

        let state = sm.get_current_state("order", "O001").await.unwrap();
        assert_eq!(state, "new");
    }
}
