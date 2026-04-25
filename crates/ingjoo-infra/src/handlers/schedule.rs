use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use ingjoo_queue::{ScheduledJob, ScheduleStatus, ScheduleStore, SqlScheduleStore};

use ingjoo_core::db::traits::IngjooStore;

use crate::extractors::CurrentUser;
use crate::middleware::database_selector::ResolvedDatabase;
use crate::middleware::error::AppError;
use crate::AppState;

async fn require_admin(user: &CurrentUser, store: &Arc<dyn IngjooStore>) -> Result<(), AppError> {
    if !user.is_admin() {
        let _ = store.create_audit_log(
            Some(&user.user_id),
            "admin_required_denied",
            "schedules",
            None,
            None,
            None,
        ).await;
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ScheduleFilter {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateScheduleRequest {
    pub name: String,
    pub cron_expr: String,
    pub queue: Option<String>,
    pub job_name: String,
    pub payload: Option<serde_json::Value>,
    pub max_attempts: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateScheduleRequest {
    pub name: Option<String>,
    pub cron_expr: Option<String>,
    pub queue: Option<String>,
    pub job_name: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub status: Option<String>,
    pub max_attempts: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ScheduleResponse {
    pub id: String,
    pub name: String,
    pub cron_expr: String,
    pub queue: String,
    pub job_name: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub max_attempts: i32,
    pub last_fire_time: Option<String>,
    pub next_fire_time: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ScheduledJob> for ScheduleResponse {
    fn from(job: ScheduledJob) -> Self {
        Self {
            id: job.id,
            name: job.name,
            cron_expr: job.cron_expr,
            queue: job.queue,
            job_name: job.job_name,
            payload: job.payload,
            status: job.status.as_str().to_string(),
            max_attempts: job.max_attempts,
            last_fire_time: job.last_fire_time.map(|t| t.to_rfc3339()),
            next_fire_time: job.next_fire_time.map(|t| t.to_rfc3339()),
            created_at: job.created_at.to_rfc3339(),
            updated_at: job.updated_at.to_rfc3339(),
        }
    }
}

fn make_store(resolved_db: &ResolvedDatabase) -> SqlScheduleStore<'_> {
    SqlScheduleStore::new(&resolved_db.pool, &resolved_db.dialect)
}

pub async fn list_schedules(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<ScheduleFilter>,
) -> Result<Json<Vec<ScheduleResponse>>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let store = make_store(&resolved_db);
    let status = filter.status.as_deref().and_then(ScheduleStatus::try_from_str);
    let jobs = store.list(status).await?;
    Ok(Json(jobs.into_iter().map(ScheduleResponse::from).collect()))
}

pub async fn create_schedule(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateScheduleRequest>,
) -> Result<(StatusCode, Json<ScheduleResponse>), AppError> {
    require_admin(&current_user, &state.store).await?;

    let mut job = ScheduledJob::new(
        &req.name,
        &req.cron_expr,
        req.queue.as_deref().unwrap_or("default"),
        &req.job_name,
        req.payload.unwrap_or(serde_json::Value::Null),
    );

    if let Some(max) = req.max_attempts {
        job = job.with_max_attempts(max);
    }

    // 预计算首次触发时间
    job.next_fire_time = Some(job.compute_next_fire_time()?);

    let store = make_store(&resolved_db);
    store.create(&job).await?;
    Ok((StatusCode::CREATED, Json(ScheduleResponse::from(job))))
}

pub async fn get_schedule(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ScheduleResponse>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let store = make_store(&resolved_db);
    let job = store.get(&id).await?.ok_or_else(|| AppError::NotFound(id))?;
    Ok(Json(ScheduleResponse::from(job)))
}

pub async fn update_schedule(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateScheduleRequest>,
) -> Result<Json<ScheduleResponse>, AppError> {
    require_admin(&current_user, &state.store).await?;
    let store = make_store(&resolved_db);
    let mut job = store.get(&id).await?.ok_or_else(|| AppError::NotFound(id.clone()))?;

    if let Some(name) = req.name {
        job.name = name;
    }
    if let Some(cron_expr) = req.cron_expr {
        job.cron_expr = cron_expr;
        job.next_fire_time = Some(job.compute_next_fire_time()?);
    }
    if let Some(queue) = req.queue {
        job.queue = queue;
    }
    if let Some(job_name) = req.job_name {
        job.job_name = job_name;
    }
    if let Some(payload) = req.payload {
        job.payload = payload;
    }
    if let Some(status) = req.status {
        job.status = ScheduleStatus::try_from_str(&status)
            .ok_or_else(|| AppError::BadRequest(format!("无效状态: {}", status)))?;
    }
    if let Some(max) = req.max_attempts {
        job.max_attempts = max;
    }
    job.updated_at = chrono::Utc::now();

    store.update(&job).await?;
    Ok(Json(ScheduleResponse::from(job)))
}

pub async fn delete_schedule(
    Extension(current_user): Extension<CurrentUser>,
    Extension(resolved_db): Extension<ResolvedDatabase>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_admin(&current_user, &state.store).await?;
    let store = make_store(&resolved_db);
    store.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}
