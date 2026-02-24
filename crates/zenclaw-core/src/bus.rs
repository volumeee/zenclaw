//! Event Bus — async pub/sub message passing between components.

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};

use crate::message::{InboundMessage, OutboundMessage};

/// Event types flowing through the bus.
#[derive(Debug, Clone)]
pub enum BusEvent {
    /// Incoming message from a channel.
    Inbound(InboundMessage),
    /// Outgoing message to a channel.
    Outbound(OutboundMessage),
    /// System event (lifecycle, tool, error).
    System(SystemEvent),
}

/// System event for monitoring.
#[derive(Debug, Clone)]
pub struct SystemEvent {
    pub run_id: String,
    pub event_type: String,
    pub data: serde_json::Value,
}

impl SystemEvent {
    /// Helper to format a system event into a human-readable string for UI/UX
    pub fn format_status(&self) -> Option<String> {
        match self.event_type.as_str() {
            "agent_think" => {
                let it = self.data["iteration"].as_u64().unwrap_or(0);
                Some(format!("🧠 Thinking... (iteration {})", it))
            }
            "tool_use" => {
                if let Some(tool) = self.data["tool"].as_str() {
                    let mut target = String::new();
                    let json_args_opt = self.data["args"].as_str().and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
                    if let Some(json_args) = json_args_opt {
                        if let Some(url) = json_args["url"].as_str() {
                            target = format!(" ({})", url);
                        } else if let Some(path) = json_args["path"].as_str() {
                            target = format!(" ({})", path);
                        } else if let Some(query) = json_args["query"].as_str() {
                            target = format!(" ({})", query);
                        } else if let Some(cmd) = json_args["command"].as_str() {
                            target = format!(" ({})", cmd);
                        }
                    }
                    
                    let human_tool = match tool {
                        "web_scrape" | "web_fetch" => "Reading Page",
                        "web_search" => "Searching Web",
                        "shell" => "Running Command",
                        "read_file" | "list_dir" => "Checking File",
                        "write_file" | "edit_file" => "Modifying Code",
                        _ => tool,
                    };
                    
                    Some(format!("🛠️ {}{}", human_tool, target))
                } else {
                    None
                }
            }
            "tool_result" => {
                Some("✅ Analysis Complete. Thinking...".to_string())
            }
            "memory_truncate" => {
                Some("🧹 Summarizing old memories...".to_string())
            }
            "tool_timeout" => {
                let tool = self.data["tool"].as_str().unwrap_or("unknown");
                Some(format!("⚠️ Tool timeout: {}", tool))
            }
            _ => None,
        }
    }
}

/// The event bus — central nervous system of ZenClaw.
///
/// Components publish events, other components subscribe to them.
/// Uses tokio channels for async, non-blocking communication.
pub struct EventBus {
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: Arc<Mutex<mpsc::Receiver<InboundMessage>>>,
    outbound_tx: broadcast::Sender<OutboundMessage>,
    system_tx: broadcast::Sender<SystemEvent>,
}

impl EventBus {
    pub fn new(buffer_size: usize) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel(buffer_size);
        let (outbound_tx, _) = broadcast::channel(buffer_size);
        let (system_tx, _) = broadcast::channel(buffer_size);

        Self {
            inbound_tx,
            inbound_rx: Arc::new(Mutex::new(inbound_rx)),
            outbound_tx,
            system_tx,
        }
    }

    /// Publish an inbound message (from channel → agent).
    pub async fn publish_inbound(&self, msg: InboundMessage) {
        if let Err(e) = self.inbound_tx.send(msg).await {
            tracing::error!("Failed to publish inbound: {}", e);
        }
    }

    /// Receive the next inbound message (agent consumes).
    pub async fn recv_inbound(&self) -> Option<InboundMessage> {
        let mut rx = self.inbound_rx.lock().await;
        rx.recv().await
    }

    /// Publish an outbound message (agent → channel).
    pub fn publish_outbound(&self, msg: OutboundMessage) {
        let _ = self.outbound_tx.send(msg);
    }

    /// Subscribe to outbound messages (channels consume).
    pub fn subscribe_outbound(&self) -> broadcast::Receiver<OutboundMessage> {
        self.outbound_tx.subscribe()
    }

    /// Publish a system event (monitoring).
    pub fn publish_system(&self, event: SystemEvent) {
        let _ = self.system_tx.send(event);
    }

    /// Subscribe to system events.
    pub fn subscribe_system(&self) -> broadcast::Receiver<SystemEvent> {
        self.system_tx.subscribe()
    }

    /// Get a clone of the inbound sender (for channels to use).
    pub fn inbound_sender(&self) -> mpsc::Sender<InboundMessage> {
        self.inbound_tx.clone()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}
