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
    let key = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .or_else(|| {
            // Fallback to strict X-Forwarded-For parsing (take the first valid IP)
            headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next()) // Take first IP
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

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
    let expected = match expected_key {
        Some(key) => key,
        None => return next.run(request).await,
    };

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
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get, Router, http::{Request, StatusCode}};
    use tower::ServiceExt;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    async fn dummy_handler() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn test_auth_middleware_no_key_configured() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // Clear environment key
        unsafe { std::env::remove_var("ZENCLAW_API_KEY"); }

        let app = Router::new()
            .route("/", get(dummy_handler))
            .layer(axum::middleware::from_fn(auth_middleware));

        let res = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // Should allow request without token because ZENCLAW_API_KEY is unset
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_middleware_with_key() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // Set environment key
        unsafe { std::env::set_var("ZENCLAW_API_KEY", "test-secret-key"); }

        let app = Router::new()
            .route("/", get(dummy_handler))
            .layer(axum::middleware::from_fn(auth_middleware));

        // 1. No auth header should fail
        let res1 = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res1.status(), StatusCode::UNAUTHORIZED);

        // 2. Wrong Bearer token should fail
        let res2 = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("authorization", "Bearer wrong-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res2.status(), StatusCode::UNAUTHORIZED);

        // 3. Correct Bearer token should succeed
        let res3 = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("authorization", "Bearer test-secret-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res3.status(), StatusCode::OK);

        // 4. Correct X-API-Key should succeed
        let res4 = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("x-api-key", "test-secret-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res4.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rate_limit_middleware() {
        let app = Router::new()
            .route("/", get(dummy_handler))
            .layer(axum::middleware::from_fn(rate_limit_middleware));

        let unique_ip = "192.168.0.99";

        // Send max_requests (60)
        for _ in 0..60 {
            let res = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/")
                        // Use unique IP via X-Forwarded-For so it doesn't collide with other tests
                        .header("x-forwarded-for", unique_ip)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
        }

        // The 61st request should be rate-limited
        let res_limited = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("x-forwarded-for", unique_ip)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_limited.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
