//! API middleware — rate limiting, API key auth, CORS, and logging.

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::info;

/// Rate limiter state.
#[derive(Clone)]
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Check if a request should be allowed.
    async fn check(&self, key: &str) -> bool {
        let mut requests = self.requests.lock().await;
        let now = Instant::now();

        let entry = requests.entry(key.to_string()).or_default();

        // Remove expired entries
        entry.retain(|t| now.duration_since(*t) < self.window);

        if entry.len() >= self.max_requests {
            false
        } else {
            entry.push(now);
            true
        }
    }
}

/// Rate limiting middleware.
pub async fn rate_limit_middleware(
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Use IP or API key as rate limit key
    let key = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    // Simple in-memory rate limiter (60 req/min)
    static LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
    let limiter = LIMITER.get_or_init(|| RateLimiter::new(60, 60));

    if !limiter.check(&key).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(serde_json::json!({
                "error": "Rate limit exceeded. Max 60 requests per minute."
            })),
        )
            .into_response();
    }

    next.run(request).await
}

/// API key authentication middleware.
pub async fn auth_middleware(
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Check for API key in config
    let expected_key = std::env::var("ZENCLAW_API_KEY").ok();

    // If no API key set, allow all requests
    if expected_key.is_none() {
        return next.run(request).await;
    }

    let expected = expected_key.unwrap();

    // Check Authorization header
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = if let Some(bearer) = auth.strip_prefix("Bearer ") {
        bearer
    } else {
        // Also check X-API-Key header
        headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    };

    if token != expected {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "error": "Invalid or missing API key. Set Authorization: Bearer <key> or X-API-Key: <key>"
            })),
        )
            .into_response();
    }

    next.run(request).await
}

/// Request logging middleware.
pub async fn logging_middleware(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = Instant::now();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    info!(
        "{} {} → {} ({:.1}ms)",
        method,
        uri,
        status.as_u16(),
        duration.as_secs_f64() * 1000.0
    );

    response
}
