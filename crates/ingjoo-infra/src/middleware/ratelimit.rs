use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_per_sec: f64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(max_tokens: u32, refill_per_sec: f64) -> Self {
        Self { tokens: max_tokens as f64, max_tokens: max_tokens as f64, refill_per_sec, last_refill: Instant::now() }
    }

    pub fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.max_tokens);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    max_tokens: u32,
    refill_per_sec: f64,
}

impl RateLimiter {
    pub fn new(max_tokens: u32, refill_per_sec: f64) -> Self {
        Self { buckets: Arc::new(Mutex::new(HashMap::new())), max_tokens, refill_per_sec }
    }

    pub async fn check(&self, key: &str) -> bool {
        let mut buckets = self.buckets.lock().await;
        let bucket =
            buckets.entry(key.to_string()).or_insert_with(|| TokenBucket::new(self.max_tokens, self.refill_per_sec));
        bucket.try_consume()
    }
}

pub async fn rate_limit_middleware(
    axum::Extension(limiter): axum::Extension<Arc<RateLimiter>>,
    request: Request,
    next: Next,
) -> Response {
    let key = request
        .headers()
        .get("x-forwarded-for")
        .or_else(|| request.headers().get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    if !limiter.check(&key).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": "请求过于频繁，请稍后再试",
                "code": "RATE_LIMITED",
                "status": 429,
            })),
        )
            .into_response();
    }

    next.run(request).await
}
