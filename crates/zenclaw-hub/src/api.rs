//! REST API server — expose ZenClaw as an HTTP service.
//!
//! Endpoints:
//! - POST /v1/chat — Send a message and get a response
//! - GET  /v1/status — System status
//! - GET  /v1/health — Health check
//! - POST /v1/rag/index — Index a document into RAG
//! - POST /v1/rag/search — Search indexed documents

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    response::sse::{Event, Sse},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use core::convert::Infallible;
use tracing::info;

use zenclaw_core::agent::{Agent, ProcessOptions};
use zenclaw_core::error::ApiErrorEnvelope;
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::LlmProvider;
use zenclaw_core::bus::EventBus;

use crate::memory::RagStore;

/// Shared API state.
#[derive(Clone)]
pub struct ApiState {
    pub agent: Arc<Agent>,
    pub provider: Arc<dyn LlmProvider>,
    pub memory: Arc<dyn MemoryStore>,
    pub rag: Option<Arc<RagStore>>,
}

type SharedState = Arc<Mutex<ApiState>>;

// ─── Request/Response types ────────────────────────────────

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub media: Vec<String>,
    #[serde(default = "default_session")]
    pub session: String,
    /// Optional system prompt override for this request.
    pub system: Option<String>,
    /// Optional model override for this request (fallback to server default).
    pub model: Option<String>,
    /// Optional response format: "json" to enforce JSON output.
    pub response_format: Option<String>,
}

fn default_session() -> String {
    "api".to_string()
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub session: String,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub version: String,
    pub status: String,
    pub tools: Vec<String>,
}

#[derive(Deserialize)]
pub struct RagIndexRequest {
    pub source: String,
    pub content: String,
    #[serde(default)]
    pub metadata: String,
}

#[derive(Serialize)]
pub struct RagIndexResponse {
    pub id: i64,
    pub source: String,
}

#[derive(Deserialize)]
pub struct RagSearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    5
}

#[derive(Serialize)]
pub struct RagSearchResult {
    pub source: String,
    pub content: String,
    pub metadata: String,
    pub rank: f64,
}

#[derive(Serialize)]
pub struct RagSearchResponse {
    pub results: Vec<RagSearchResult>,
    pub count: usize,
}

// ─── Handlers ──────────────────────────────────────────────

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

async fn status(State(state): State<SharedState>) -> Json<StatusResponse> {
    let s = state.lock().await;
    let tools: Vec<String> = s.agent.tools.names().iter().map(|t| t.to_string()).collect();

    Json(StatusResponse {
        version: "0.1.0".to_string(),
        status: "running".to_string(),
        tools,
    })
}

async fn chat(
    State(state): State<SharedState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, Json<ApiErrorEnvelope>)> {
    let s = state.lock().await;

    let opts = ProcessOptions {
        system_prompt: req.system,
        model: req.model,
        response_format: req.response_format,
    };

    match s
        .agent
        .process_with_media(s.provider.as_ref(), s.memory.as_ref(), &req.message, req.media.clone(), &req.session, None, opts)
        .await
    {
        Ok(response) => Ok(Json(ChatResponse {
            response,
            session: req.session,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorEnvelope::from_error(&e)),
        )),
    }
}

async fn chat_stream(
    State(state): State<SharedState>,
    Json(req): Json<ChatRequest>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel(128);

    let bus = EventBus::new(32);
    let mut bus_rx = bus.subscribe_system();

    let shared_state = state.clone();
    let message = req.message.clone();
    let session = req.session.clone();
    let media = req.media.clone();
    let opts = ProcessOptions {
        system_prompt: req.system,
        model: req.model,
        response_format: req.response_format,
    };

    // Background task listening to EventBus
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        while let Ok(event) = bus_rx.recv().await {
            // Raw JSON payloads
            let data_str = serde_json::to_string(&event.data).unwrap_or_default();
            let sse_event = Event::default().event(event.event_type.clone()).data(data_str);
            if tx_clone.send(Ok(sse_event)).await.is_err() {
                break;
            }
            
            // Human readable status stream
            if let Some(msg) = event.format_status() {
                let status_event = Event::default().event("status_text").data(msg);
                let _ = tx_clone.send(Ok(status_event)).await;
            }
        }
    });

    let tx_for_agent = tx.clone();
    tokio::spawn(async move {
        let s2 = shared_state.lock().await;
        let p = s2.provider.as_ref();
        let m = s2.memory.as_ref();

        match s2.agent.process_with_media(p, m, &message, media, &session, Some(&bus), opts).await {
            Ok(response) => {
                let _ = tx_for_agent.send(Ok(Event::default().event("result").data(response))).await;
            }
            Err(e) => {
                // SSE errors also use consistent JSON envelope
                let envelope = ApiErrorEnvelope::from_error(&e);
                let err_json = serde_json::to_string(&envelope).unwrap_or_else(|_| e.to_string());
                let _ = tx_for_agent.send(Ok(Event::default().event("error").data(err_json))).await;
            }
        }
    });

    Sse::new(ReceiverStream::new(rx))
}

async fn rag_index(
    State(state): State<SharedState>,
    Json(req): Json<RagIndexRequest>,
) -> Result<Json<RagIndexResponse>, (StatusCode, Json<ApiErrorEnvelope>)> {
    let s = state.lock().await;

    match &s.rag {
        Some(rag) => match rag.index(&req.source, &req.content, &req.metadata) {
            Ok(id) => Ok(Json(RagIndexResponse {
                id,
                source: req.source,
            })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorEnvelope::new("RAG_INDEX_ERROR", &e.to_string())),
            )),
        },
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiErrorEnvelope::new("RAG_UNAVAILABLE", "RAG not enabled")),
        )),
    }
}

async fn rag_search(
    State(state): State<SharedState>,
    Json(req): Json<RagSearchRequest>,
) -> Result<Json<RagSearchResponse>, (StatusCode, Json<ApiErrorEnvelope>)> {
    let s = state.lock().await;

    match &s.rag {
        Some(rag) => match rag.search(&req.query, req.limit) {
            Ok(results) => {
                let count = results.len();
                Ok(Json(RagSearchResponse {
                    results: results
                        .into_iter()
                        .map(|r| RagSearchResult {
                            source: r.source,
                            content: r.content,
                            metadata: r.metadata,
                            rank: r.rank,
                        })
                        .collect(),
                    count,
                }))
            }
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorEnvelope::new("RAG_SEARCH_ERROR", &e.to_string())),
            )),
        },
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiErrorEnvelope::new("RAG_UNAVAILABLE", "RAG not enabled")),
        )),
    }
}

// ─── Server builder ────────────────────────────────────────

/// Build the API router.
pub fn build_router(state: SharedState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/status", get(status))
        .route("/v1/chat", post(chat))
        .route("/v1/chat/stream", post(chat_stream))
        .route("/v1/rag/index", post(rag_index))
        .route("/v1/rag/search", post(rag_search))
        .with_state(state)
}

/// Start the API server.
pub async fn start_server_from_state(state: ApiState, host: &str, port: u16) -> anyhow::Result<()> {
    let shared = Arc::new(Mutex::new(state));
    let app = build_router(shared);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    info!("🌐 API server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt; // For collecting the body stream
    use tower::ServiceExt; // for oneshot
    use zenclaw_core::error::ZenClawError;
    use zenclaw_core::message::{ChatMessage, LlmResponse};
    use zenclaw_core::provider::{ChatRequest as ProviderChatRequest, LlmProvider};
    use zenclaw_core::memory::InMemoryStore;
    use zenclaw_core::agent::Agent;

    struct ErrorMockProvider;

    #[async_trait]
    impl LlmProvider for ErrorMockProvider {
        fn name(&self) -> &str {
            "error-mock"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }

        async fn chat(&self, _request: ProviderChatRequest) -> zenclaw_core::error::Result<LlmResponse> {
            Err(ZenClawError::Provider("simulated provider error".to_string()))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_chat_stream_error_envelope() {
        let agent = Arc::new(Agent::new());
        let provider = Arc::new(ErrorMockProvider);
        let memory = Arc::new(InMemoryStore::new());

        let state = ApiState {
            agent,
            provider,
            memory,
            rag: None,
        };

        let app = build_router(Arc::new(Mutex::new(state)));

        let req_body = r#"{ "message": "Hello", "session": "test-session" }"#;

        let req = Request::builder()
            .method("POST")
            .uri("/v1/chat/stream")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(req_body.as_bytes()))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);

        let mut res_body = res.into_body();
        let mut body_str = String::new();
        
        while let Some(frame_result) = res_body.frame().await {
            let frame = frame_result.unwrap();
            if let Some(chunk) = frame.data_ref() {
                let s = String::from_utf8(chunk.to_vec()).unwrap();
                println!("Got chunk: {}", s);
                body_str.push_str(&s);
            }
        }

        // Check that the error event contains the correct JSON serialized envelope
        assert!(body_str.contains("event: error"));
        assert!(body_str.contains(r#""code":"PROVIDER_ERROR""#));
        assert!(body_str.contains("simulated provider error"));
    }
}
