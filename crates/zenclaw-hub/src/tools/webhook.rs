//! Webhook tool — receive and process external events.
//!
//! Allows the agent to register webhook endpoints and process
//! incoming payloads from external services (GitHub, Stripe, etc.)

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Stored webhook data.
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub source: String,
    pub payload: Value,
    pub received_at: String,
}

/// Shared webhook store.
pub type WebhookStore = Arc<Mutex<Vec<WebhookEvent>>>;

/// Create a new webhook store.
pub fn new_webhook_store() -> WebhookStore {
    Arc::new(Mutex::new(Vec::new()))
}

/// Webhook tool — check received webhook events.
pub struct WebhookTool {
    store: WebhookStore,
}

impl WebhookTool {
    pub fn new(store: WebhookStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for WebhookTool {
    fn name(&self) -> &str {
        "webhooks"
    }

    fn description(&self) -> &str {
        "Check received webhook events. External services can POST data to /v1/webhooks/:source, and this tool lets you inspect them."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "clear"],
                    "description": "Action: 'list' recent events, 'get' events by source, 'clear' all events"
                },
                "source": {
                    "type": "string",
                    "description": "Filter by source name (for 'get' action)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max events to return (default: 10)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let action = args["action"].as_str().unwrap_or("list");
        let limit = args["limit"].as_u64().unwrap_or(10) as usize;

        let mut store = self.store.lock().await;

        match action {
            "list" => {
                if store.is_empty() {
                    return Ok("No webhook events received.".to_string());
                }

                let events: Vec<&WebhookEvent> = store.iter().rev().take(limit).collect();
                let mut output = format!("📨 {} webhook events:\n", events.len());

                for (i, evt) in events.iter().enumerate() {
                    let payload_preview = serde_json::to_string(&evt.payload)
                        .unwrap_or_default();
                    let preview = if payload_preview.len() > 120 {
                        format!("{}...", &payload_preview[..120])
                    } else {
                        payload_preview
                    };

                    output.push_str(&format!(
                        "\n{}. [{}] {} — {}",
                        i + 1,
                        evt.received_at,
                        evt.source,
                        preview
                    ));
                }

                Ok(output)
            }
            "get" => {
                let source = args["source"].as_str().unwrap_or("");
                if source.is_empty() {
                    return Ok("Source is required for 'get' action.".to_string());
                }

                let events: Vec<&WebhookEvent> = store
                    .iter()
                    .filter(|e| e.source == source)
                    .rev()
                    .take(limit)
                    .collect();

                if events.is_empty() {
                    return Ok(format!("No events from source: {}", source));
                }

                let mut output = format!("📨 {} events from '{}':\n", events.len(), source);
                for (i, evt) in events.iter().enumerate() {
                    output.push_str(&format!(
                        "\n{}. [{}]\n{}\n",
                        i + 1,
                        evt.received_at,
                        serde_json::to_string_pretty(&evt.payload).unwrap_or_default()
                    ));
                }

                Ok(output)
            }
            "clear" => {
                let count = store.len();
                store.clear();
                Ok(format!("✅ Cleared {} webhook events.", count))
            }
            _ => Ok(format!("Unknown action: {}. Use 'list', 'get', or 'clear'.", action)),
        }
    }
}
