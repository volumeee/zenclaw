//! OpenAI-compatible provider — works with OpenAI, Ollama, LM Studio, etc.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::message::{FunctionCall, LlmResponse, TokenUsage, ToolCall};
use zenclaw_core::provider::{ChatRequest, LlmProvider, ProviderConfig};

/// OpenAI-compatible provider.
///
/// Works with any API that follows the OpenAI chat completions format:
/// - OpenAI (api.openai.com)
/// - Ollama (localhost:11434)
/// - LM Studio (localhost:1234)
/// - OpenRouter (openrouter.ai)
/// - Groq, Together, Fireworks, etc.
pub struct OpenAiProvider {
    client: Client,
    config: ProviderConfig,
    api_url: String,
}

impl OpenAiProvider {
    pub fn new(config: ProviderConfig) -> Self {
        let api_base = config
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        let api_url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

        Self {
            client: Client::new(),
            config,
            api_url,
        }
    }

    /// Create a provider for Ollama (local).
    pub fn ollama(model: &str) -> Self {
        Self::new(ProviderConfig {
            provider: "ollama".to_string(),
            model: model.to_string(),
            api_key: Some("ollama".to_string()),
            api_base: Some("http://localhost:11434/v1".to_string()),
            ..Default::default()
        })
    }

    /// Create a provider for OpenAI.
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::new(ProviderConfig {
            provider: "openai".to_string(),
            model: model.to_string(),
            api_key: Some(api_key.to_string()),
            api_base: None,
            ..Default::default()
        })
    }

    /// Create a provider for OpenRouter.
    pub fn openrouter(api_key: &str, model: &str) -> Self {
        Self::new(ProviderConfig {
            provider: "openrouter".to_string(),
            model: model.to_string(),
            api_key: Some(api_key.to_string()),
            api_base: Some("https://openrouter.ai/api/v1".to_string()),
            ..Default::default()
        })
    }

    /// Create a provider for Google Gemini (via OpenAI-compatible endpoint).
    pub fn gemini(api_key: &str, model: &str) -> Self {
        Self::new(ProviderConfig {
            provider: "gemini".to_string(),
            model: model.to_string(),
            api_key: Some(api_key.to_string()),
            api_base: Some("https://generativelanguage.googleapis.com/v1beta/openai".to_string()),
            ..Default::default()
        })
    }

    /// Create a provider for Groq (via OpenAI-compatible endpoint).
    pub fn groq(api_key: &str, model: &str) -> Self {
        Self::new(ProviderConfig {
            provider: "groq".to_string(),
            model: model.to_string(),
            api_key: Some(api_key.to_string()),
            api_base: Some("https://api.groq.com/openai/v1".to_string()),
            ..Default::default()
        })
    }
}


/// Internal request body.
#[derive(Serialize)]
struct ApiRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
    max_tokens: u32,
    temperature: f32,
}

/// Internal response body.
#[derive(Deserialize)]
struct ApiResponse {
    choices: Vec<ApiChoice>,
    model: String,
    usage: Option<ApiUsage>,
}

#[derive(Deserialize)]
struct ApiChoice {
    message: ApiMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ApiMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Deserialize)]
struct ApiToolCall {
    id: String,
    r#type: String,
    function: ApiFunction,
}

#[derive(Deserialize)]
struct ApiFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct ApiUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiErrorDetail,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: String,
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        &self.config.provider
    }

    fn default_model(&self) -> &str {
        &self.config.model
    }

    async fn chat(&self, request: ChatRequest) -> Result<LlmResponse> {
        let model = request
            .model
            .unwrap_or_else(|| self.config.model.clone());

        info!("Calling {} model: {}", self.config.provider, model);

        // Convert messages to API format
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| serde_json::to_value(m).unwrap_or_default())
            .collect();

        // Convert tools to API format
        let tools: Vec<serde_json::Value> = request
            .tools
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or_default())
            .collect();

        let mut max_tokens = request.max_tokens;
        if self.config.provider == "groq" {
            // Groq free tier has very tight Tokens Per Minute limits (e.g. 6000 TPM limit).
            // It calculates request cost as: input_tokens + max_tokens.
            // If max_tokens is 4096, it instantly throws a 429 Too Many Requests.
            // We clamp it to 1024 to survive the free tier limits.
            max_tokens = max_tokens.min(1024);
        }

        let body = ApiRequest {
            model: model.clone(),
            messages,
            tools,
            max_tokens,
            temperature: request.temperature,
        };


        let api_key = self.config.api_key.as_deref().unwrap_or("");

        let resp = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let body_text = resp.text().await?;

        debug!("API response status: {}, body length: {}", status, body_text.len());

        if !status.is_success() {
            // Try to parse error
            if let Ok(err) = serde_json::from_str::<ApiError>(&body_text) {
                return Err(ZenClawError::Provider(format!(
                    "{} API error ({}): {}",
                    self.config.provider, status, err.error.message
                )));
            }
            return Err(ZenClawError::Provider(format!(
                "{} API error ({}): {}",
                self.config.provider,
                status,
                &body_text[..body_text.len().min(200)]
            )));
        }

        let api_resp: ApiResponse = serde_json::from_str(&body_text).map_err(|e| {
            ZenClawError::Provider(format!("Failed to parse response: {} — body: {}", e, &body_text[..body_text.len().min(200)]))
        })?;

        // Use the model name from the API response (more accurate, API may remap)
        let actual_model = api_resp.model;

        let choice = api_resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ZenClawError::Provider("No choices in response".to_string()))?;

        let tool_calls = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                r#type: tc.r#type,
                function: FunctionCall {
                    name: tc.function.name,
                    arguments: tc.function.arguments,
                },
            })
            .collect();

        let usage = api_resp.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens.unwrap_or(0),
            completion_tokens: u.completion_tokens.unwrap_or(0),
            total_tokens: u.total_tokens.unwrap_or(0),
        }).unwrap_or_default();

        Ok(LlmResponse {
            content: choice.message.content,
            tool_calls,
            model: actual_model,
            usage,
            finish_reason: choice.finish_reason.unwrap_or_else(|| "stop".to_string()),
        })
    }
}
