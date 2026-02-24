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

use zenclaw_core::agent::Agent;
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::LlmProvider;
use zenclaw_core::bus::EventBus;

use crate::memory::RagStore;

/// Shared API state.
pub struct ApiState {
    pub agent: Agent,
    pub provider: Box<dyn LlmProvider>,
    pub memory: Box<dyn MemoryStore>,
    pub rag: Option<RagStore>,
}

type SharedState = Arc<Mutex<ApiState>>;

// ─── Request/Response types ────────────────────────────────

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default = "default_session")]
    pub session: String,
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

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
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
) -> Result<Json<ChatResponse>, (StatusCode, Json<ErrorResponse>)> {
    let s = state.lock().await;

    match s
        .agent
        .process(s.provider.as_ref(), s.memory.as_ref(), &req.message, &req.session, None)
        .await
    {
        Ok(response) => Ok(Json(ChatResponse {
            response,
            session: req.session,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
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

    // The state and trait objects need to be cloned/Arc'd to be spawned, 
    // but we can't easily move s.agent and s.provider. 
    // So let's clone the Agent and use a specialized spawn.
    // However, they aren't clonable easily. 
    // Wait, the agent.process requires &self, provider, memory.
    // Instead of spawning agent, we can run agent in another tokio task but we need Arc<Mutex<...>>.
    let shared_state = state.clone();
    let message = req.message.clone();
    let session = req.session.clone();

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

        match s2.agent.process(p, m, &message, &session, Some(&bus)).await {
            Ok(response) => {
                let _ = tx_for_agent.send(Ok(Event::default().event("result").data(response))).await;
            }
            Err(e) => {
                let _ = tx_for_agent.send(Ok(Event::default().event("error").data(e.to_string()))).await;
            }
        }
    });

    Sse::new(ReceiverStream::new(rx))
}

async fn rag_index(
    State(state): State<SharedState>,
    Json(req): Json<RagIndexRequest>,
) -> Result<Json<RagIndexResponse>, (StatusCode, Json<ErrorResponse>)> {
    let s = state.lock().await;

    match &s.rag {
        Some(rag) => match rag.index(&req.source, &req.content, &req.metadata) {
            Ok(id) => Ok(Json(RagIndexResponse {
                id,
                source: req.source,
            })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )),
        },
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "RAG not enabled".to_string(),
            }),
        )),
    }
}

async fn rag_search(
    State(state): State<SharedState>,
    Json(req): Json<RagSearchRequest>,
) -> Result<Json<RagSearchResponse>, (StatusCode, Json<ErrorResponse>)> {
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
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )),
        },
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "RAG not enabled".to_string(),
            }),
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
pub async fn start_server(state: ApiState, host: &str, port: u16) -> anyhow::Result<()> {
    let shared = Arc::new(Mutex::new(state));
    let app = build_router(shared);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    info!("🌐 API server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
