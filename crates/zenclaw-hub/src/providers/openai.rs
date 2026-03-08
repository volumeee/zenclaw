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

    /// Create a provider for DeepSeek.
    pub fn deepseek(api_key: &str, model: &str) -> Self {
        Self::new(ProviderConfig {
            provider: "deepseek".to_string(),
            model: model.to_string(),
            api_key: Some(api_key.to_string()),
            api_base: Some("https://api.deepseek.com/v1".to_string()),
            ..Default::default()
        })
    }

    /// Create a provider for OpenClaw local API gateway.
    pub fn openclaw(api_key: &str, model: &str) -> Self {
        Self::new(ProviderConfig {
            provider: "openclaw".to_string(),
            model: model.to_string(),
            api_key: Some(api_key.to_string()),
            api_base: Some("http://localhost:18789/v1".to_string()), // Often 18789 for openclaw or 8045 depending on proxy
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
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
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
            .map(|m| {
                let mut val = serde_json::to_value(m).unwrap_or_default();
                if let Some(obj) = val.as_object_mut() {
                    // Remove internal media array from sending to LLM
                    obj.remove("media");
                    
                    if !m.media.is_empty() {
                        let mut parts = vec![];
                        if let Some(text) = &m.content {
                            parts.push(serde_json::json!({"type": "text", "text": text}));
                        }
                        for url in &m.media {
                            parts.push(serde_json::json!({
                                "type": "image_url",
                                "image_url": { "url": url }
                            }));
                        }
                        obj.insert("content".to_string(), serde_json::json!(parts));
                    } else if m.content.is_none() {
                        // Crucial proxy fix: some OpenAI-compatible endpoints (like Gemini mappers)
                        // crash with "e is not iterable" when `content` is null (e.g., when sending back tool_calls).
                        obj.insert("content".to_string(), serde_json::json!(""));
                    }
                }
                val
            })
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

        // Build response_format for supported providers
        let response_format_val = request.response_format.as_deref().and_then(|fmt| {
            if fmt == "json" {
                Some(serde_json::json!({ "type": "json_object" }))
            } else {
                None
            }
        });

        // Set tool_choice to "auto" when tools are present
        // Many OpenAI-compatible gateways require this explicitly
        let tool_choice = if tools.is_empty() {
            None
        } else {
            Some(serde_json::json!("auto"))
        };

        let body = ApiRequest {
            model: model.clone(),
            messages,
            tools,
            tool_choice,
            max_tokens,
            temperature: request.temperature,
            response_format: response_format_val,
        };


        let api_key = self.config.api_key.as_deref().unwrap_or("");
        
        let msg_summary: Vec<String> = body.messages.iter()
            .map(|m| {
                let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("unknown");
                let has_content = m.get("content").is_some_and(|v| !v.is_null() && (v.is_string() && !v.as_str().unwrap().is_empty() || v.is_array() && !v.as_array().unwrap().is_empty()));
                format!("role:{},content:{}", role, has_content)
            }).collect();
            
        debug!("Request Messages Summary: count={}, [{}]", body.messages.len(), msg_summary.join(", "));

        debug!(
            "Sending request to {} with {} tools, tool_choice={:?}",
            self.api_url,
            body.tools.len(),
            body.tool_choice
        );

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

        let tc_count = choice.message.tool_calls.as_ref().map(|v| v.len()).unwrap_or(0);
        debug!(
            "Response: model={}, finish_reason={:?}, tool_calls={}, content_len={}",
            actual_model,
            choice.finish_reason,
            tc_count,
            choice.message.content.as_ref().map(|c| c.len()).unwrap_or(0)
        );

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
