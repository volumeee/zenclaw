use std::sync::Arc;
use zenclaw_hub::channels::{TelegramConfig, DiscordConfig, SlackConfig};
use crate::setup_bot_env;

pub async fn run_telegram(
    cli_bot_token: Option<&str>,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
    allowed_users: Option<&str>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    let saved = crate::setup::load_saved_config();
    let mut current_token = cli_bot_token
        .map(|s| s.to_string())
        .or_else(|| {
            saved
                .as_ref()
                .and_then(|c| c.channels.telegram.as_ref())
                .map(|t| t.bot_token.clone())
                .filter(|t| !t.is_empty())
        });

    let allowed: Vec<i64> = allowed_users
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    loop {
        let token = match current_token {
            Some(ref t) => t.clone(),
            None => {
                let t = crate::tui_menu::run_tui_input(
                    "Telegram Bot Token Required",
                    "Enter your Telegram Bot Token:",
                    "",
                    true
                ).ok().flatten().unwrap_or_default();
                
                if t.is_empty() {
                    return Ok(());
                }
                t
            }
        };

        let config = TelegramConfig {
            bot_token: token.clone(),
            allowed_users: allowed.clone(),
            poll_timeout: 30,
        };

        let mut telegram = zenclaw_hub::channels::TelegramChannel::new(config);
        
        match telegram.start(agent.clone(), provider.clone(), memory.clone()).await {
            Ok(_) => {
                let _ = crate::setup::run_config_set("telegram_token", &token);
                let details = [
                    ("Channel", "Telegram"),
                    ("Allowed Users", if allowed.is_empty() { "Public" } else { "Restricted" }),
                    ("Poll Timeout", "30s"),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("Telegram", &resolved_provider, &resolved_model, &details, None);
                telegram.stop().await;
                break Ok(());
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("token is invalid") || error_msg.contains("Unauthorized") {
                    let _ = crate::tui_menu::run_tui_error("Telegram Connection Failed", &format!("{}\n\nPlease check your token and try again.", error_msg));
                    current_token = None;
                } else {
                    return Err(e.into());
                }
            }
        }
    }
}

pub async fn run_discord(
    cli_bot_token: Option<&str>,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    let saved = crate::setup::load_saved_config();
    let mut current_token = cli_bot_token
        .map(|s| s.to_string())
        .or_else(|| {
            saved
                .as_ref()
                .and_then(|c| c.channels.discord.as_ref())
                .map(|d| d.bot_token.clone())
                .filter(|d| !d.is_empty())
        });

    loop {
        let token = match current_token {
            Some(ref t) => t.clone(),
            None => {
                let t = crate::tui_menu::run_tui_input(
                    "Discord Bot Token Required",
                    "Enter your Discord Bot Token:",
                    "",
                    true
                ).ok().flatten().unwrap_or_default();
                
                if t.is_empty() {
                    return Ok(());
                }
                t
            }
        };

        let config = DiscordConfig {
            bot_token: token.clone(),
            allowed_users: vec![],
        };

        let mut discord = zenclaw_hub::channels::DiscordChannel::new(config);
        
        match discord.start(agent.clone(), provider.clone(), memory.clone()).await {
            Ok(_) => {
                let _ = crate::setup::run_config_set("discord_token", &token);
                let details = [
                    ("Channel", "Discord"),
                    ("Connection", "Gateway/Secure"),
                    ("Allowed Guilds", "All"),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("Discord", &resolved_provider, &resolved_model, &details, None);
                discord.stop().await;
                break Ok(());
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("token") || error_msg.contains("Unauthorized") || error_msg.contains("401") {
                    let _ = crate::tui_menu::run_tui_error("Discord Connection Failed", &format!("{}\n\nPlease check your discord token.", error_msg));
                    current_token = None;
                } else {
                    return Err(e.into());
                }
            }
        }
    }
}

pub async fn run_slack(
    cli_bot_token: Option<&str>,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
    allowed_channels: Vec<String>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    let saved = crate::setup::load_saved_config();
    let mut current_token = cli_bot_token
        .map(|s| s.to_string())
        .or_else(|| {
            saved
                .as_ref()
                .and_then(|c| c.channels.slack.as_ref())
                .map(|s| s.bot_token.clone())
                .filter(|s| !s.is_empty())
        });

    loop {
        let token = match current_token {
            Some(ref t) => t.clone(),
            None => {
                let t = crate::tui_menu::run_tui_input(
                    "Slack Bot Token Required",
                    "Enter your Slack Bot Token:",
                    "",
                    true
                ).ok().flatten().unwrap_or_default();
                
                if t.is_empty() {
                    return Ok(());
                }
                t
            }
        };

        let config = SlackConfig {
            bot_token: token.clone(),
            allowed_channels: allowed_channels.clone(),
        };

        let mut slack = zenclaw_hub::channels::SlackChannel::new(config);
        
        match slack.start(agent.clone(), provider.clone(), memory.clone()).await {
            Ok(_) => {
                let _ = crate::setup::run_config_set("slack_token", &token);
                let details = [
                    ("Channel", "Slack"),
                    ("Allowed Chans", if allowed_channels.is_empty() { "All" } else { "Restricted" }),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("Slack", &resolved_provider, &resolved_model, &details, None);
                slack.stop().await;
                break Ok(());
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("token") || error_msg.contains("Unauthorized") {
                    let _ = crate::tui_menu::run_tui_error("Slack Connection Failed", &format!("{}\n\nPlease check your slack token.", error_msg));
                    current_token = None;
                } else {
                    return Err(e.into());
                }
            }
        }
    }
}

pub async fn run_whatsapp(
    cli_bridge_url: &str,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
    allowed_numbers: Option<&str>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    let mut current_bridge_url = if cli_bridge_url.is_empty() {
        "http://localhost:3001".to_string()
    } else {
        cli_bridge_url.to_string()
    };

    let (log_tx, log_rx) = tokio::sync::mpsc::channel(100);

    loop {
        let mut wa = zenclaw_hub::channels::WhatsAppChannel::new(&current_bridge_url);

        if let Some(numbers) = allowed_numbers {
            let nums: Vec<String> = numbers.split(',').map(|s| s.trim().to_string()).collect();
            wa = wa.with_allowed_numbers(nums);
        }

        match wa.start(agent.clone(), provider.clone(), memory.clone(), Some(log_tx.clone())).await {
            Ok(_) => {
                let details = [
                    ("Bridge URL", current_bridge_url.as_str()),
                    ("Poll Interval", "2000ms"),
                    ("Auth", "Bridge-based"),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("WhatsApp", &resolved_provider, &resolved_model, &details, Some(log_rx));
                break Ok(());
            }
            Err(e) => {
                let _ = crate::tui_menu::run_tui_error("WhatsApp Connection Failed", &format!("{}\n\nMake sure your bridge is running or check the URL.", e));
                let input = crate::tui_menu::run_tui_input(
                    "WhatsApp Bridge URL", 
                    "Enter Bridge URL:", 
                    &current_bridge_url, 
                    false
                )?;
                
                if let Some(new_url) = input {
                    current_bridge_url = new_url;
                } else {
                    return Ok(());
                }
            }
        }
    }
}
