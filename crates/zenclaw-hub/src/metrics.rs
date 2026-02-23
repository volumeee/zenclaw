//! Runtime metrics — track request counts, latencies, errors.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Global metrics collector.
#[derive(Debug, Default)]
pub struct Metrics {
    pub requests_total: AtomicU64,
    pub requests_success: AtomicU64,
    pub requests_error: AtomicU64,
    pub tokens_in: AtomicU64,
    pub tokens_out: AtomicU64,
    pub tool_calls: AtomicU64,
    pub rag_queries: AtomicU64,
    pub webhook_events: AtomicU64,
    start_time: Option<Instant>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            ..Default::default()
        }
    }

    pub fn record_request(&self, success: bool) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        if success {
            self.requests_success.fetch_add(1, Ordering::Relaxed);
        } else {
            self.requests_error.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_tool_call(&self) {
        self.tool_calls.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_rag_query(&self) {
        self.rag_queries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_webhook(&self) {
        self.webhook_events.fetch_add(1, Ordering::Relaxed);
    }

    pub fn uptime_secs(&self) -> u64 {
        self.start_time
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0)
    }

    /// Export as JSON.
    pub fn to_json(&self) -> serde_json::Value {
        let uptime = self.uptime_secs();
        let hours = uptime / 3600;
        let minutes = (uptime % 3600) / 60;
        let seconds = uptime % 60;

        serde_json::json!({
            "uptime": format!("{}h {}m {}s", hours, minutes, seconds),
            "uptime_secs": uptime,
            "requests": {
                "total": self.requests_total.load(Ordering::Relaxed),
                "success": self.requests_success.load(Ordering::Relaxed),
                "errors": self.requests_error.load(Ordering::Relaxed),
            },
            "tokens": {
                "in": self.tokens_in.load(Ordering::Relaxed),
                "out": self.tokens_out.load(Ordering::Relaxed),
            },
            "tool_calls": self.tool_calls.load(Ordering::Relaxed),
            "rag_queries": self.rag_queries.load(Ordering::Relaxed),
            "webhook_events": self.webhook_events.load(Ordering::Relaxed),
        })
    }
}

/// Shared metrics instance.
pub type SharedMetrics = Arc<Metrics>;

pub fn new_metrics() -> SharedMetrics {
    Arc::new(Metrics::new())
}
