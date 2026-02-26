//! Configuration management for ZenClaw.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{Result, ZenClawError};
use crate::provider::ProviderConfig;

/// Top-level ZenClaw configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ZenClawConfig {
    /// LLM provider settings.
    #[serde(default)]
    pub provider: ProviderConfig,

    /// Agent settings.
    #[serde(default)]
    pub agent: AgentSettings,

    /// Channel configurations.
    #[serde(default)]
    pub channels: ChannelSettings,
}

/// Agent-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    /// Max iterations in the ReAct loop.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,

    /// Custom system prompt (None = use default).
    pub system_prompt: Option<String>,

    /// Workspace directory.
    pub workspace: Option<String>,
}

fn default_max_iterations() -> usize {
    20
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            system_prompt: None,
            workspace: None,
        }
    }
}

/// Channel configurations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelSettings {
    pub telegram: Option<TelegramConfig>,
    pub discord: Option<DiscordConfig>,
    pub slack: Option<SlackConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    #[serde(default)]
    pub allowed_users: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub bot_token: String,
    #[serde(default)]
    pub allowed_users: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub bot_token: String,
    #[serde(default)]
    pub allowed_channels: Vec<String>,
}

impl ZenClawConfig {
    /// Load config from a TOML file.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| ZenClawError::Config(format!("Failed to read config: {}", e)))?;
        toml::from_str(&content)
            .map_err(|e| ZenClawError::Config(format!("Failed to parse config: {}", e)))
    }

    /// Save config to a TOML file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ZenClawError::Config(format!("Failed to serialize config: {}", e)))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the default config file path.
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zenclaw")
            .join("config.toml")
    }
}

