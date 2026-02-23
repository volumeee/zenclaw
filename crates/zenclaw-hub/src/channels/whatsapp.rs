//! WhatsApp channel adapter — connects via HTTP bridge to Baileys/wa-automate.
//!
//! ZenClaw doesn't bundle a full WhatsApp library (too heavy for edge).
//! Instead, it connects to a lightweight HTTP bridge that you run separately:
//!
//! ```text
//! [WhatsApp] <--WS--> [Baileys Bridge :3001] <--HTTP--> [ZenClaw]
//! ```
//!
//! The bridge exposes:
//! - GET  /messages — poll for new messages
//! - POST /send     — send a message
//!
//! You can use any Baileys-based bridge, e.g. whatsapp-web.js or wa-automate-nodejs.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use zenclaw_core::agent::Agent;
use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::LlmProvider;

/// WhatsApp message from the bridge.
#[derive(Debug, Deserialize)]
pub struct WaMessage {
    pub id: String,
    pub from: String,
    pub body: String,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub group_name: Option<String>,
    #[serde(default)]
    pub sender_name: Option<String>,
}

/// Send message request.
#[derive(Debug, Serialize)]
struct WaSendRequest {
    to: String,
    message: String,
}

/// WhatsApp channel adapter.
pub struct WhatsAppChannel {
    bridge_url: String,
    client: reqwest::Client,
    allowed_numbers: Option<HashSet<String>>,
    poll_interval_ms: u64,
}

impl WhatsAppChannel {
    pub fn new(bridge_url: &str) -> Self {
        Self {
            bridge_url: bridge_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            allowed_numbers: None,
            poll_interval_ms: 2000,
        }
    }

    /// Set allowed phone numbers (whitelist).
    pub fn with_allowed_numbers(mut self, numbers: Vec<String>) -> Self {
        self.allowed_numbers = Some(numbers.into_iter().collect());
        self
    }

    /// Set poll interval in milliseconds.
    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.poll_interval_ms = ms;
        self
    }

    /// Check if a number is allowed.
    fn is_allowed(&self, number: &str) -> bool {
        match &self.allowed_numbers {
            Some(allowed) => allowed.contains(number),
            None => true,
        }
    }

    /// Poll for new messages from the bridge.
    async fn poll_messages(&self) -> Result<Vec<WaMessage>> {
        let url = format!("{}/messages", self.bridge_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ZenClawError::Other(format!("WhatsApp poll failed: {}", e)))?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let messages: Vec<WaMessage> = resp
            .json()
            .await
            .unwrap_or_default();

        Ok(messages)
    }

    /// Send a message via the bridge.
    async fn send_message(&self, to: &str, message: &str) -> Result<()> {
        let url = format!("{}/send", self.bridge_url);
        let body = WaSendRequest {
            to: to.to_string(),
            message: message.to_string(),
        };

        self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ZenClawError::Other(format!("WhatsApp send failed: {}", e)))?;

        Ok(())
    }

    /// Start the WhatsApp bot loop.
    pub async fn start(
        &self,
        agent: &Agent,
        provider: &dyn LlmProvider,
        memory: &dyn MemoryStore,
    ) -> Result<()> {
        info!("📱 WhatsApp bot starting...");
        info!("🔗 Bridge: {}", self.bridge_url);

        // Check bridge connectivity
        match self.client.get(&format!("{}/status", self.bridge_url)).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("✅ Bridge connected");
            }
            _ => {
                warn!("⚠️ Bridge not reachable at {}. Continuing anyway...", self.bridge_url);
            }
        }

        if let Some(ref allowed) = self.allowed_numbers {
            info!("🔒 Allowed numbers: {:?}", allowed);
        } else {
            info!("🔓 All numbers allowed");
        }

        loop {
            match self.poll_messages().await {
                Ok(messages) => {
                    for msg in messages {
                        // Skip non-allowed
                        if !self.is_allowed(&msg.from) {
                            continue;
                        }

                        let sender = msg.sender_name.as_deref().unwrap_or(&msg.from);
                        info!("📩 [{}] {}", sender, msg.body);

                        // Process with agent
                        let session_key = format!("wa_{}", msg.from);

                        match agent
                            .process(provider, memory, &msg.body, &session_key)
                            .await
                        {
                            Ok(response) => {
                                info!("📤 → {}: {}...",
                                    sender,
                                    if response.len() > 80 { &response[..80] } else { &response }
                                );

                                if let Err(e) = self.send_message(&msg.from, &response).await {
                                    error!("Failed to send message: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Agent error for {}: {}", sender, e);
                                let _ = self
                                    .send_message(
                                        &msg.from,
                                        &format!("❌ Error: {}", e),
                                    )
                                    .await;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Poll error: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(self.poll_interval_ms)).await;
        }
    }
}
