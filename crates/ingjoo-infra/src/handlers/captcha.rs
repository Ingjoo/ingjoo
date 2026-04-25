use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::captcha::CaptchaGenerator;
use crate::middleware::error::AppError;
use crate::AppState;

#[derive(Serialize)]
pub struct CaptchaResponse {
    pub id: String,
    pub image: String,
}

#[derive(Deserialize)]
pub struct VerifyCaptchaRequest {
    pub id: String,
    pub answer: String,
}

#[derive(Serialize)]
pub struct VerifyCaptchaResponse {
    pub valid: bool,
}

/// 计算过期时间（当前时间 + 5 分钟），返回 RFC 3339 格式字符串
fn expires_in_5_minutes() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() + 5 * 60;
    format_timestamp(secs as i64)
}

/// 将 Unix 时间戳格式化为类似 RFC 3339 的字符串
fn format_timestamp(secs: i64) -> String {
    // 简单的 Unix 时间戳 → "YYYY-MM-DDTHH:MM:SSZ" 格式
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;

    // 从 1970-01-01 计算年月日
    let mut year = 1970i64;
    let mut remaining_days = days;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    let month_days: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &md in &month_days {
        if remaining_days < md {
            break;
        }
        remaining_days -= md;
        month += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// 解析 RFC 3339 时间戳为 Unix 时间戳（秒）
fn parse_timestamp(s: &str) -> Option<i64> {
    // 格式: "YYYY-MM-DDTHH:MM:SSZ" 或 "YYYY-MM-DDTHH:MM:SS+XX:XX"
    let s = s.trim_end_matches('Z');
    let parts: Vec<&str> = s.split('T').collect();
    if parts.len() != 2 {
        return None;
    }
    let date_parts: Vec<i64> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
    let time_str = parts[1].split('+').next().unwrap_or("");
    let time_parts: Vec<i64> = time_str.split(':').filter_map(|p| p.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }
    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (hour, minute, second) = (time_parts[0], time_parts[1], time_parts[2]);

    let month_days: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut total_days: i64 = 0;
    for y in 1970..year {
        total_days += if is_leap_year(y) { 366 } else { 365 };
    }
    for days in &month_days[..(month - 1) as usize] {
        total_days += days;
    }
    total_days += day - 1;

    Some(total_days * 86400 + hour * 3600 + minute * 60 + second)
}

/// GET /api/captcha — 生成验证码
pub async fn generate_captcha(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<CaptchaResponse>), AppError> {
    let (answer, data_uri) = CaptchaGenerator::generate()
        .map_err(AppError::Internal)?;

    let id = uuid::Uuid::new_v4().to_string();
    let expires_at = expires_in_5_minutes();

    state
        .store
        .create_captcha(&id, &answer, &expires_at)
        .await?;

    Ok((
        StatusCode::OK,
        Json(CaptchaResponse { id, image: data_uri }),
    ))
}

/// POST /api/captcha — 验证验证码
pub async fn verify_captcha(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyCaptchaRequest>,
) -> Result<Json<VerifyCaptchaResponse>, AppError> {
    let (answer, used, expires_at) = state
        .store
        .get_captcha(&req.id)
        .await?
        .ok_or_else(|| AppError::BadRequest("验证码不存在或已过期".into()))?;

    if used == 1 {
        return Err(AppError::BadRequest("验证码已被使用".into()));
    }

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let expires_secs = parse_timestamp(&expires_at)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("验证码时间格式错误")))?;
    if now_secs > expires_secs {
        return Err(AppError::BadRequest("验证码已过期".into()));
    }

    let valid = CaptchaGenerator::verify(&answer, &req.answer);
    state.store.mark_captcha_used(&req.id).await?;

    Ok(Json(VerifyCaptchaResponse { valid }))
}
