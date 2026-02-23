//! LLM Provider trait — the abstraction over different AI model APIs.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::message::{ChatMessage, LlmResponse};

/// Tool definition in OpenAI function calling format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: FunctionDefinition,
}

/// Function definition for tool calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Configuration for a provider request.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub model: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for ChatRequest {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            tools: Vec::new(),
            model: None,
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

/// LLM Provider trait — implement this to add support for new AI providers.
///
/// # Example
///
/// ```rust,ignore
/// struct MyProvider;
///
/// #[async_trait]
/// impl LlmProvider for MyProvider {
///     fn name(&self) -> &str { "my-provider" }
///     fn default_model(&self) -> &str { "my-model-v1" }
///
///     async fn chat(&self, request: ChatRequest) -> Result<LlmResponse> {
///         // Call your API here
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name (e.g., "openai", "anthropic", "ollama").
    fn name(&self) -> &str;

    /// Default model for this provider.
    fn default_model(&self) -> &str;

    /// Send a chat completion request.
    async fn chat(&self, request: ChatRequest) -> Result<LlmResponse>;

    /// List available models (optional).
    async fn list_models(&self) -> Result<Vec<String>> {
        Ok(vec![self.default_model().to_string()])
    }
}

/// Provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_temperature() -> f32 {
    0.7
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: None,
            api_base: None,
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}
