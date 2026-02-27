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
use std::sync::Arc;

use serde::Deserialize;
use tracing::{error, info, warn};

use zenclaw_core::agent::Agent;
use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::LlmProvider;
use tokio::process::Command;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

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



/// WhatsApp channel adapter.
pub struct WhatsAppChannel {
    bridge_url: String,
    client: reqwest::Client,
    allowed_numbers: Option<HashSet<String>>,
    poll_interval_ms: u64,
    shutdown_tx: Option<tokio::sync::mpsc::Sender<()>>,
}

impl WhatsAppChannel {
    pub fn new(bridge_url: &str) -> Self {
        Self {
            bridge_url: bridge_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            allowed_numbers: None,
            poll_interval_ms: 2000,
            shutdown_tx: None,
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

    /// Start the WhatsApp bot loop — runs in background, returns immediately.
    pub async fn start(
        &mut self,
        agent: Arc<Agent>,
        provider: Arc<dyn LlmProvider>,
        memory: Arc<dyn MemoryStore>,
        log_tx: Option<tokio::sync::mpsc::Sender<String>>,
    ) -> Result<()> {
        info!("📱 WhatsApp bot starting...");
        info!("🔗 Bridge: {}", self.bridge_url);

        // Check bridge connectivity. 
        let is_ready = match self.client.get(format!("{}/status", self.bridge_url)).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(status) = resp.json::<serde_json::Value>().await {
                    status["ready"].as_bool().unwrap_or(false)
                } else {
                    false
                }
            }
            _ => false,
        };

        if is_ready {
            info!("✅ Bridge connected and ready");
        } else {
            if let Some(ref tx) = log_tx {
                let _ = tx.send("🔄 Restarting WhatsApp Bridge...".to_string()).await;
                let _ = tx.send("⏳ Waiting for port 3001 to clear...".to_string()).await;
            }
            warn!("⚠️ WhatsApp Bridge not ready or reachable. Restarting embedded bridge to capture QR code...");
            
            // Kill any existing bridge process to ensure we capture stdout
            let _ = std::process::Command::new("pkill")
                .arg("-9") // Force kill
                .arg("-f")
                .arg("bridge/bridge.js")
                .status();

            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

            if let Some(ref tx) = log_tx {
                let _ = tx.send("🚀 Spawning new bridge process...".to_string()).await;
            }

            let mut child = Command::new("node")
                .arg("bridge/bridge.js")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| ZenClawError::Other(format!("Failed to spawn bridge: {}. Is Node.js installed?", e)))?;
            
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();
            
            let log_tx_stdout = log_tx.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if let Some(ref tx) = log_tx_stdout {
                        let _ = tx.send(line.clone()).await;
                    }
                    info!("[Bridge] {}", line);
                }
            });

            let log_tx_stderr = log_tx.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if let Some(ref tx) = log_tx_stderr {
                        let _ = tx.send(format!("ERROR: {}", line)).await;
                    }
                    warn!("[Bridge Error] {}", line);
                }
            });
            
            info!("⏳ Waiting for bridge to initialize...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let bridge_url = self.bridge_url.clone();
        let client = self.client.clone();
        let allowed_numbers = self.allowed_numbers.clone();
        let poll_interval_ms = self.poll_interval_ms;

        // Clone shared app components
        let agent = agent.clone();
        let provider = provider.clone();
        let memory = memory.clone();

        tokio::spawn(async move {
            loop {
                // Check shutdown
                if shutdown_rx.try_recv().is_ok() {
                    info!("WhatsApp bot shutting down...");
                    break;
                }

                let poll_url = format!("{}/messages", bridge_url);
                match client.get(&poll_url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(messages) = resp.json::<Vec<WaMessage>>().await {
                            for msg in messages {
                                // Skip non-allowed
                                let is_allowed = match &allowed_numbers {
                                    Some(allowed) => allowed.contains(&msg.from),
                                    None => true,
                                };
                                if !is_allowed { continue; }

                                let sender = msg.sender_name.as_deref().unwrap_or(&msg.from);
                                info!("📩 [WhatsApp] {}: {}", sender, msg.body);

                                let session_key = format!("wa_{}", msg.from);
                                let provider_ref = provider.as_ref();
                                let memory_ref = memory.as_ref();

                                match agent.process(provider_ref, memory_ref, &msg.body, &session_key, None).await {
                                    Ok(response) => {
                                        info!("📤 → {}: {}...", sender, if response.len() > 80 { &response[..80] } else { &response });
                                        let send_url = format!("{}/send", bridge_url);
                                        let _ = client.post(&send_url).json(&serde_json::json!({ "to": msg.from, "message": response })).send().await;
                                    }
                                    Err(e) => {
                                        error!("Agent error for {}: {}", sender, e);
                                        let send_url = format!("{}/send", bridge_url);
                                        let _ = client.post(&send_url).json(&serde_json::json!({ "to": msg.from, "message": format!("❌ Error: {}", e) })).send().await;
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        error!("Bridge unreachable at {}", bridge_url);
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(poll_interval_ms)).await;
            }
        });

        Ok(())
    }

    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

