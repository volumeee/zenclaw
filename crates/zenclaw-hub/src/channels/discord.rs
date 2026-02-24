//! Discord bot channel — direct HTTP API (Gateway-less for minimal dependencies).
//!
//! Uses Discord's HTTP API with Gateway websocket for receiving messages.
//! Lightweight implementation without heavy SDK dependencies.

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
use zenclaw_core::bus::EventBus;

/// Discord bot configuration.
#[derive(Debug, Clone)]
pub struct DiscordConfig {
    /// Bot token from Discord Developer Portal.
    pub bot_token: String,
    /// Allowed user IDs (empty = allow everyone).
    pub allowed_users: Vec<String>,
}

/// Discord bot channel — uses HTTP API polling.
///
/// Note: This is a simplified implementation that polls for messages.
/// For production, consider using Discord Gateway (WebSocket).
pub struct DiscordChannel {
    config: DiscordConfig,
    client: Client,
    api_base: String,
    bot_user_id: Option<String>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl DiscordChannel {
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            api_base: "https://discord.com/api/v10".to_string(),
            bot_user_id: None,
            shutdown_tx: None,
        }
    }

    /// Start the Discord bot.
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
        // Verify token
        let me = self.get_me().await?;
        self.bot_user_id = Some(me.id.clone());
        info!("🎮 Discord bot started: {}#{}", me.username, me.discriminator.unwrap_or_default());

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let client = self.client.clone();
        let api_base = self.api_base.clone();
        let bot_token = self.config.bot_token.clone();
        let bot_user_id = me.id;
        let allowed_users = self.config.allowed_users.clone();

        // Spawn message polling task
        // Uses DM channels — the bot listens to direct messages
        tokio::spawn(async move {
            let mut last_message_ids: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();

            loop {
                if shutdown_rx.try_recv().is_ok() {
                    info!("Discord bot shutting down...");
                    break;
                }

                // Get DM channels
                match get_dm_channels(&client, &api_base, &bot_token).await {
                    Ok(channels) => {
                        for channel in channels {
                            let channel_id = &channel.id;

                            // Get recent messages
                            let after = last_message_ids
                                .get(channel_id)
                                .map(|s| s.as_str());

                            match get_messages(
                                &client,
                                &api_base,
                                &bot_token,
                                channel_id,
                                after,
                            )
                            .await
                            {
                                Ok(messages) => {
                                    for msg in messages.iter().rev() {
                                        // Skip bot's own messages
                                        if msg.author.id == bot_user_id {
                                            continue;
                                        }

                                        // Check allowed users
                                        if !allowed_users.is_empty()
                                            && !allowed_users.contains(&msg.author.id)
                                        {
                                            continue;
                                        }

                                        let content = &msg.content;
                                        if content.is_empty() {
                                            continue;
                                        }

                                        let session_key =
                                            format!("discord:{}", channel_id);

                                        info!(
                                            "📨 [Discord] {}: {}",
                                            msg.author.username,
                                            if content.len() > 80 {
                                                &content[..80]
                                            } else {
                                                content
                                            }
                                        );

                                        // Send initial thinking message
                                        let mut initial_msg_id = None;
                                        if let Ok(msg) = send_message(
                                            &client,
                                            &api_base,
                                            &bot_token,
                                            channel_id,
                                            "🧠 *Process Started...*",
                                        )
                                        .await {
                                            initial_msg_id = Some(msg.id);
                                        }

                                        let bus = EventBus::new(32);
                                        let mut rx = bus.subscribe_system();
                                        let bg_client = client.clone();
                                        let bg_api_base = api_base.clone();
                                        let bg_bot_token = bot_token.clone();
                                        let bg_channel_id = channel_id.to_string();
                                        let msg_id_clone = initial_msg_id.clone();
                                        
                                        let _bg_task = tokio::spawn(async move {
                                            if let Some(msg_id) = msg_id_clone {
                                                let mut last_status = String::new();
                                                while let Ok(event) = rx.recv().await {
                                                    if let Some(msg) = event.format_status() {
                                                        let new_status_msg = format!("*{}*", msg);
                                                        if new_status_msg != last_status {
                                                            last_status = new_status_msg.clone();
                                                            let _ = edit_message(
                                                                &bg_client,
                                                                &bg_api_base,
                                                                &bg_bot_token,
                                                                &bg_channel_id,
                                                                &msg_id,
                                                                &new_status_msg,
                                                            ).await;
                                                            
                                                            // Delay to avoid hitting Discord API rate limits
                                                            tokio::time::sleep(Duration::from_millis(1000)).await;
                                                        }
                                                    }
                                                }
                                            }
                                        });

                                        // Process through agent
                                        match agent
                                            .process(
                                                provider.as_ref(),
                                                memory.as_ref(),
                                                content,
                                                &session_key,
                                                Some(&bus),
                                            )
                                            .await
                                        {
                                            Ok(response) => {
                                                if let Some(msg_id) = initial_msg_id {
                                                    let _ = delete_message(&client, &api_base, &bot_token, channel_id, &msg_id).await;
                                                }

                                                // Split long messages (Discord limit: 2000)
                                                for chunk in split_message(&response, 1900) {
                                                    let _ = send_message(
                                                        &client,
                                                        &api_base,
                                                        &bot_token,
                                                        channel_id,
                                                        &chunk,
                                                    )
                                                    .await;
                                                }
                                            }
                                            Err(e) => {
                                                if let Some(msg_id) = initial_msg_id {
                                                    let _ = delete_message(&client, &api_base, &bot_token, channel_id, &msg_id).await;
                                                }

                                                error!("Agent error: {}", e);
                                                let _ = send_message(
                                                    &client,
                                                    &api_base,
                                                    &bot_token,
                                                    channel_id,
                                                    &format!("❌ Error: {}", e),
                                                )
                                                .await;
                                            }
                                        }

                                        // Update last seen message
                                        last_message_ids.insert(
                                            channel_id.clone(),
                                            msg.id.clone(),
                                        );
                                    }

                                    // Update last message ID even if no new messages
                                    if let Some(latest) = messages.first() {
                                        last_message_ids
                                            .entry(channel_id.clone())
                                            .or_insert_with(|| latest.id.clone());
                                    }
                                }
                                Err(e) => {
                                    debug!("Error getting messages for {}: {}", channel_id, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error getting DM channels: {}", e);
                    }
                }

                // Poll interval
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });

        Ok(())
    }

    /// Get bot user info.
    async fn get_me(&self) -> Result<DiscordUser> {
        let url = format!("{}/users/@me", self.api_base);
        let resp: DiscordUser = self
            .client
            .get(&url)
            .header("Authorization", format!("Bot {}", self.config.bot_token))
            .send()
            .await?
            .json()
            .await
            .map_err(|e| ZenClawError::Provider(format!("Discord getMe failed: {}", e)))?;
        Ok(resp)
    }

    /// Stop the bot.
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

// ─── Discord API Types ─────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    discriminator: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DcChannel {
    id: String,
    #[serde(rename = "type")]
    channel_type: u8,
}

#[derive(Debug, Deserialize)]
struct DiscordMessage {
    id: String,
    content: String,
    author: DiscordUser,
}

#[derive(Debug, Serialize)]
struct SendMessageBody {
    content: String,
}

// ─── API Helpers ───────────────────────────────────────────

async fn get_dm_channels(
    client: &Client,
    api_base: &str,
    token: &str,
) -> Result<Vec<DcChannel>> {
    let url = format!("{}/users/@me/channels", api_base);
    let resp: Vec<DcChannel> = client
        .get(&url)
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await?
        .json()
        .await
        .map_err(|e| ZenClawError::Provider(format!("Discord channels error: {}", e)))?;

    // Filter to DM channels only (type 1)
    Ok(resp.into_iter().filter(|c| c.channel_type == 1).collect())
}

async fn get_messages(
    client: &Client,
    api_base: &str,
    token: &str,
    channel_id: &str,
    after: Option<&str>,
) -> Result<Vec<DiscordMessage>> {
    let mut url = format!("{}/channels/{}/messages?limit=5", api_base, channel_id);
    if let Some(after_id) = after {
        url.push_str(&format!("&after={}", after_id));
    }

    let resp: Vec<DiscordMessage> = client
        .get(&url)
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await?
        .json()
        .await
        .map_err(|e| ZenClawError::Provider(format!("Discord messages error: {}", e)))?;

    Ok(resp)
}

async fn send_message(
    client: &Client,
    api_base: &str,
    token: &str,
    channel_id: &str,
    content: &str,
) -> Result<DiscordMessage> {
    let url = format!("{}/channels/{}/messages", api_base, channel_id);
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bot {}", token))
        .json(&SendMessageBody {
            content: content.to_string(),
        })
        .send()
        .await?;
    let msg: DiscordMessage = resp.json().await.map_err(|e| ZenClawError::Provider(e.to_string()))?;
    Ok(msg)
}

async fn edit_message(
    client: &Client,
    api_base: &str,
    token: &str,
    channel_id: &str,
    message_id: &str,
    content: &str,
) -> Result<()> {
    let url = format!("{}/channels/{}/messages/{}", api_base, channel_id, message_id);
    let _ = client
        .patch(&url)
        .header("Authorization", format!("Bot {}", token))
        .json(&SendMessageBody {
            content: content.to_string(),
        })
        .send()
        .await;
    Ok(())
}

async fn delete_message(
    client: &Client,
    api_base: &str,
    token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<()> {
    let url = format!("{}/channels/{}/messages/{}", api_base, channel_id, message_id);
    let _ = client
        .delete(&url)
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await;
    Ok(())
}

/// Split a long message into chunks.
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
