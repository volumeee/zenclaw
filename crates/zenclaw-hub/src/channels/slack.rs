//! Slack bot channel — direct HTTP API polling.
//!
//! Uses Slack Web API for minimal dependencies.
//! Polls channels the bot is a member of for new messages.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use zenclaw_core::agent::Agent;
use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::LlmProvider;
use zenclaw_core::bus::EventBus;

/// Slack bot configuration.
#[derive(Debug, Clone)]
pub struct SlackConfig {
    /// Bot User OAuth Token (xoxb-...)
    pub bot_token: String,
    /// Allowed channel/user IDs to listen to (if empty, listen to all joined channels)
    pub allowed_channels: Vec<String>,
}

/// Slack bot channel — uses HTTP API polling.
pub struct SlackChannel {
    config: SlackConfig,
    client: Client,
    api_base: String,
    bot_user_id: Option<String>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl SlackChannel {
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            api_base: "https://slack.com/api".to_string(),
            bot_user_id: None,
            shutdown_tx: None,
        }
    }

    /// Start the Slack bot polling task.
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
        let auth_info = match auth_test(&self.client, &self.api_base, &self.config.bot_token).await {
            Ok(info) => info,
            Err(e) => return Err(ZenClawError::Provider(format!("Slack auth failed: {}", e))),
        };

        let user_id = auth_info.user_id.unwrap_or_default();
        let user = auth_info.user.unwrap_or_default();
        self.bot_user_id = Some(user_id.clone());
        info!("👔 Slack bot started: {} ({})", user, user_id);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let client = self.client.clone();
        let api_base = self.api_base.clone();
        let bot_token = self.config.bot_token.clone();
        let bot_user_id = user_id;
        let allowed_channels = self.config.allowed_channels.clone();

        tokio::spawn(async move {
            let mut last_message_ts: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();

            loop {
                if shutdown_rx.try_recv().is_ok() {
                    info!("Slack bot shutting down...");
                    break;
                }

                // Get joined channels
                match joined_channels(&client, &api_base, &bot_token).await {
                    Ok(channels) => {
                        for channel in channels {
                            let channel_id = &channel.id;

                            if !allowed_channels.is_empty() && !allowed_channels.contains(channel_id) {
                                continue;
                            }

                            let oldest = last_message_ts.get(channel_id).map(|s| s.as_str());

                            match get_messages(&client, &api_base, &bot_token, channel_id, oldest).await {
                                Ok(messages) => {
                                    for msg in messages.iter().rev() {
                                        if let Some(user) = &msg.user {
                                            if user == &bot_user_id {
                                                continue;
                                            }
                                        }

                                        let content = msg.text.clone().unwrap_or_default();
                                        if content.is_empty() {
                                            continue;
                                        }

                                        let session_key = format!("slack:{}", channel_id);

                                        info!(
                                            "📨 [Slack] {}: {}",
                                            msg.user.as_deref().unwrap_or("Unknown User"),
                                            if content.len() > 80 { &content[..80] } else { &content }
                                        );

                                        let bus = EventBus::new(32);
                                        
                                        // Send initial message
                                        let mut initial_ts = None;
                                        if let Ok(resp) = send_message(&client, &api_base, &bot_token, channel_id, "🧠 _Process Started..._").await {
                                            initial_ts = resp.ts;
                                        }

                                        let ts_clone = initial_ts.clone();
                                        let bg_client = client.clone();
                                        let bg_api_base = api_base.clone();
                                        let bg_bot_token = bot_token.clone();
                                        let bg_channel_id = channel_id.to_string();
                                        let mut rx = bus.subscribe_system();

                                        let _bg_task = tokio::spawn(async move {
                                            if let Some(ts) = ts_clone {
                                                let mut last_status = String::new();
                                                while let Ok(event) = rx.recv().await {
                                                    if let Some(msg) = event.format_status() {
                                                        let new_status_msg = format!("_{}_", msg);
                                                        if new_status_msg != last_status {
                                                            last_status = new_status_msg.clone();
                                                            let _ = edit_message(
                                                                &bg_client,
                                                                &bg_api_base,
                                                                &bg_bot_token,
                                                                &bg_channel_id,
                                                                &ts,
                                                                &new_status_msg,
                                                            ).await;
                                                            tokio::time::sleep(Duration::from_millis(1000)).await;
                                                        }
                                                    }
                                                }
                                            }
                                        });

                                        match agent.process(
                                            provider.as_ref(),
                                            memory.as_ref(),
                                            &content,
                                            &session_key,
                                            Some(&bus),
                                        ).await {
                                            Ok(response) => {
                                                if let Some(ts) = initial_ts {
                                                    let _ = delete_message(&client, &api_base, &bot_token, channel_id, &ts).await;
                                                }

                                                for chunk in split_message(&response, 3000) {
                                                    let _ = send_message(
                                                        &client,
                                                        &api_base,
                                                        &bot_token,
                                                        channel_id,
                                                        &chunk,
                                                    ).await;
                                                }
                                            }
                                            Err(e) => {
                                                if let Some(ts) = initial_ts {
                                                    let _ = delete_message(&client, &api_base, &bot_token, channel_id, &ts).await;
                                                }
                                                error!("Agent error: {}", e);
                                                let _ = send_message(
                                                    &client,
                                                    &api_base,
                                                    &bot_token,
                                                    channel_id,
                                                    &format!("❌ Error: {}", e),
                                                ).await;
                                            }
                                        }

                                        last_message_ts.insert(channel_id.clone(), msg.ts.clone());
                                    }

                                    if let Some(latest) = messages.first() {
                                        last_message_ts
                                            .entry(channel_id.clone())
                                            .or_insert_with(|| latest.ts.clone());
                                    }
                                }
                                Err(e) => {
                                    debug!("Error getting messages for {}: {}", channel_id, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error getting Slack channels: {}", e);
                    }
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
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

// ─── API Types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SlackAuthTestResponse {
    ok: bool,
    user_id: Option<String>,
    user: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackConversationsResponse {
    ok: bool,
    channels: Option<Vec<SlackChannelObject>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackChannelObject {
    id: String,
}

#[derive(Debug, Deserialize)]
struct SlackHistoryResponse {
    ok: bool,
    messages: Option<Vec<SlackMessage>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackMessage {
    ts: String,
    user: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackPostResponse {
    ok: bool,
    ts: Option<String>,
}

// ─── API Helpers ─────────────────────────────────────────────

async fn auth_test(client: &Client, api_base: &str, token: &str) -> Result<SlackAuthTestResponse> {
    let url = format!("{}/auth.test", api_base);
    let resp: SlackAuthTestResponse = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json()
        .await
        .map_err(|e| ZenClawError::Provider(e.to_string()))?;
        
    if !resp.ok {
        return Err(ZenClawError::Provider(resp.error.unwrap_or_default()));
    }
    
    Ok(SlackAuthTestResponse {
        ok: true,
        user: Some(resp.user.unwrap_or_default()),
        user_id: Some(resp.user_id.unwrap_or_default()),
        error: None,
    })
}

async fn joined_channels(client: &Client, api_base: &str, token: &str) -> Result<Vec<SlackChannelObject>> {
    let url = format!("{}/users.conversations?types=public_channel,private_channel,im,mpim", api_base);
    let resp: SlackConversationsResponse = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json()
        .await
        .map_err(|e| ZenClawError::Provider(e.to_string()))?;
        
    if !resp.ok {
        return Err(ZenClawError::Provider(resp.error.unwrap_or_default()));
    }
    Ok(resp.channels.unwrap_or_default())
}

async fn get_messages(client: &Client, api_base: &str, token: &str, channel: &str, oldest: Option<&str>) -> Result<Vec<SlackMessage>> {
    let mut url = format!("{}/conversations.history?channel={}&limit=10", api_base, channel);
    if let Some(ts) = oldest {
        url.push_str(&format!("&oldest={}", ts));
    }

    let resp: SlackHistoryResponse = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json()
        .await
        .map_err(|e| ZenClawError::Provider(e.to_string()))?;

    if !resp.ok {
        return Err(ZenClawError::Provider(resp.error.unwrap_or_default()));
    }
    Ok(resp.messages.unwrap_or_default())
}

async fn send_message(client: &Client, api_base: &str, token: &str, channel: &str, text: &str) -> Result<SlackPostResponse> {
    let url = format!("{}/chat.postMessage", api_base);
    let resp: SlackPostResponse = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "channel": channel,
            "text": text,
        }))
        .send()
        .await?
        .json()
        .await
        .map_err(|e| ZenClawError::Provider(e.to_string()))?;
    Ok(resp)
}

async fn edit_message(client: &Client, api_base: &str, token: &str, channel: &str, ts: &str, text: &str) -> Result<()> {
    let url = format!("{}/chat.update", api_base);
    let _ = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "channel": channel,
            "ts": ts,
            "text": text,
        }))
        .send()
        .await;
    Ok(())
}

async fn delete_message(client: &Client, api_base: &str, token: &str, channel: &str, ts: &str) -> Result<()> {
    let url = format!("{}/chat.delete", api_base);
    let _ = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "channel": channel,
            "ts": ts,
        }))
        .send()
        .await;
    Ok(())
}

fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len { return vec![text.to_string()]; }
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
        if !current.is_empty() { current.push('\n'); }
        current.push_str(line);
    }
    if !current.is_empty() { chunks.push(current); }
    chunks
}
