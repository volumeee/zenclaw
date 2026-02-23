//! Telegram bot channel — direct HTTP API, no heavy SDK.
//!
//! Uses raw Telegram Bot API via reqwest for minimal binary size.
//! Supports: text messages, typing indicator, markdown formatting.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use zenclaw_core::agent::Agent;
use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::LlmProvider;

/// Telegram bot configuration.
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    /// Bot token from @BotFather.
    pub bot_token: String,
    /// Allowed user IDs (empty = allow everyone).
    pub allowed_users: Vec<i64>,
    /// Polling timeout in seconds.
    pub poll_timeout: u64,
}

/// Telegram bot channel — runs as a long-polling service.
pub struct TelegramChannel {
    config: TelegramConfig,
    client: Client,
    api_base: String,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl TelegramChannel {
    pub fn new(config: TelegramConfig) -> Self {
        let api_base = format!(
            "https://api.telegram.org/bot{}",
            config.bot_token
        );

        Self {
            config,
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
            api_base,
            shutdown_tx: None,
        }
    }

    /// Start the Telegram bot — runs in background, returns immediately.
    pub async fn start<P, M>(
        &mut self,
        agent: Arc<Agent>,
        provider: Arc<P>,
        memory: Arc<M>,
    ) -> Result<()>
    where
        P: LlmProvider + 'static,
        M: MemoryStore + 'static,
    {
        // Verify token works
        let me = self.get_me().await?;
        info!("🤖 Telegram bot started: @{}", me.username.unwrap_or_default());

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let client = self.client.clone();
        let api_base = self.api_base.clone();
        let allowed_users = self.config.allowed_users.clone();
        let poll_timeout = self.config.poll_timeout;

        // Spawn polling task
        tokio::spawn(async move {
            let mut offset: i64 = 0;

            loop {
                // Check shutdown
                if shutdown_rx.try_recv().is_ok() {
                    info!("Telegram bot shutting down...");
                    break;
                }

                // Long poll for updates
                match get_updates(&client, &api_base, offset, poll_timeout).await {
                    Ok(updates) => {
                        for update in updates {
                            if let Some(msg) = update.message {
                                offset = update.update_id + 1;

                                // Check allowed users
                                if !allowed_users.is_empty()
                                    && !allowed_users.contains(&msg.sender_id())
                                {
                                    warn!("Blocked user: {}", msg.sender_id());
                                    continue;
                                }

                                let text = msg.text.clone().unwrap_or_default();
                                if text.is_empty() {
                                    continue;
                                }

                                let chat_id = msg.chat.id;
                                let session_key = format!("telegram:{}", chat_id);
                                
                                info!(
                                    "📨 [{}] {}: {}",
                                    chat_id,
                                    msg.sender_name(),
                                    if text.len() > 80 { &text[..80] } else { &text }
                                );

                                // Handle commands
                                if text.starts_with('/') {
                                    match text.as_str() {
                                        "/start" => {
                                            let welcome = "⚡ *ZenClaw AI* — Build AI the simple way\\!\n\nSend me any message and I'll help you\\. 🦀";
                                            let _ = send_message(
                                                &client,
                                                &api_base,
                                                chat_id,
                                                welcome,
                                                Some("MarkdownV2"),
                                            )
                                            .await;
                                            continue;
                                        }
                                        "/clear" | "/reset" => {
                                            let _ = memory.clear_history(&session_key).await;
                                            let _ = send_message(
                                                &client,
                                                &api_base,
                                                chat_id,
                                                "🗑️ Conversation history cleared.",
                                                None,
                                            )
                                            .await;
                                            continue;
                                        }
                                        "/help" => {
                                            let help = "Available commands:\n/start — Welcome message\n/clear — Clear conversation\n/help — This help message\n\nOr just send any message!";
                                            let _ = send_message(
                                                &client,
                                                &api_base,
                                                chat_id,
                                                help,
                                                None,
                                            )
                                            .await;
                                            continue;
                                        }
                                        _ => {} // Fall through to agent
                                    }
                                }

                                // Send typing indicator
                                let _ = send_typing(&client, &api_base, chat_id).await;

                                // Process through agent
                                match agent.process(
                                    provider.as_ref(),
                                    memory.as_ref(),
                                    &text,
                                    &session_key,
                                ).await {
                                    Ok(response) => {
                                        info!("📤 [{}] Response: {} chars", chat_id, response.len());

                                        // Split long messages (Telegram limit: 4096)
                                        for chunk in split_message(&response, 4000) {
                                            if let Err(e) = send_message(
                                                &client,
                                                &api_base,
                                                chat_id,
                                                &chunk,
                                                None,
                                            )
                                            .await
                                            {
                                                error!("Failed to send message: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Agent error: {}", e);
                                        let _ = send_message(
                                            &client,
                                            &api_base,
                                            chat_id,
                                            &format!("❌ Error: {}", e),
                                            None,
                                        )
                                        .await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Polling error: {}. Retrying in 5s...", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Get bot info to verify token.
    async fn get_me(&self) -> Result<TgUser> {
        let url = format!("{}/getMe", self.api_base);
        let resp: TgResponse<TgUser> = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| ZenClawError::Provider(format!("Telegram getMe failed: {}", e)))?;

        if !resp.ok {
            return Err(ZenClawError::Provider(
                "Telegram bot token is invalid".into(),
            ));
        }
        resp.result
            .ok_or_else(|| ZenClawError::Provider("No result from getMe".into()))
    }

    /// Stop the bot.
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

// ─── Telegram API Types ────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TgResponse<T> {
    ok: bool,
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct TgUpdate {
    update_id: i64,
    message: Option<TgMessage>,
}

#[derive(Debug, Deserialize)]
struct TgMessage {
    text: Option<String>,
    chat: TgChat,
    from: Option<TgUser>,
}

impl TgMessage {
    fn sender_id(&self) -> i64 {
        self.from.as_ref().map(|u| u.id).unwrap_or(0)
    }

    fn sender_name(&self) -> String {
        self.from
            .as_ref()
            .map(|u| {
                u.first_name
                    .clone()
                    .unwrap_or_else(|| u.username.clone().unwrap_or_else(|| "Unknown".into()))
            })
            .unwrap_or_else(|| "Unknown".into())
    }
}

#[derive(Debug, Deserialize)]
struct TgChat {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct TgUser {
    id: i64,
    first_name: Option<String>,
    username: Option<String>,
}

#[derive(Debug, Serialize)]
struct SendMessageBody {
    chat_id: i64,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
}

// ─── API Helpers ───────────────────────────────────────────

async fn get_updates(
    client: &Client,
    api_base: &str,
    offset: i64,
    timeout: u64,
) -> Result<Vec<TgUpdate>> {
    let url = format!(
        "{}/getUpdates?offset={}&timeout={}&allowed_updates=[\"message\"]",
        api_base, offset, timeout
    );

    let resp: TgResponse<Vec<TgUpdate>> = client
        .get(&url)
        .timeout(Duration::from_secs(timeout + 10))
        .send()
        .await?
        .json()
        .await
        .map_err(|e| ZenClawError::Provider(format!("getUpdates parse error: {}", e)))?;

    Ok(resp.result.unwrap_or_default())
}

async fn send_message(
    client: &Client,
    api_base: &str,
    chat_id: i64,
    text: &str,
    parse_mode: Option<&str>,
) -> Result<()> {
    let url = format!("{}/sendMessage", api_base);
    let body = SendMessageBody {
        chat_id,
        text: text.to_string(),
        parse_mode: parse_mode.map(String::from),
    };

    let resp = client.post(&url).json(&body).send().await?;

    if !resp.status().is_success() {
        // If markdown failed, try plain text
        if parse_mode.is_some() {
            debug!("Markdown send failed, retrying as plain text");
            let plain_body = SendMessageBody {
                chat_id,
                text: text.to_string(),
                parse_mode: None,
            };
            client.post(&url).json(&plain_body).send().await?;
        }
    }

    Ok(())
}

async fn send_typing(client: &Client, api_base: &str, chat_id: i64) -> Result<()> {
    let url = format!("{}/sendChatAction", api_base);
    let _ = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "action": "typing"
        }))
        .send()
        .await;
    Ok(())
}

/// Split a long message into chunks at line boundaries.
fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        if current.len() + line.len() + 1 > max_len {
            if !current.is_empty() {
                chunks.push(current.clone());
                current.clear();
            }
            // If single line exceeds max, hard split
            if line.len() > max_len {
                for i in (0..line.len()).step_by(max_len) {
                    let end = (i + max_len).min(line.len());
                    chunks.push(line[i..end].to_string());
                }
                continue;
            }
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}
