use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::AppState;
use ingjoo_core::pool::PoolStats;

/// 健康检查端点 — 返回服务状态、数据库连通性、连接池统计、运行时间和已注册模型数
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let uptime_secs = state.start_time.elapsed().as_secs();
    let model_count = state.registry.len();

    let db_ok = sqlx::query("SELECT 1")
        .execute(&*state.pool)
        .await
        .is_ok();

    let pool_stats = PoolStats::from_pool(&state.pool);

    Json(serde_json::json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "database": db_ok,
        "pool": pool_stats,
        "uptime_seconds": uptime_secs,
        "registered_models": model_count,
        "dialect": format!("{:?}", state.dialect),
    }))
}
