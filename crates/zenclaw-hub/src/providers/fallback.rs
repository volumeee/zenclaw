//! Provider with automatic model fallback.
//!
//! Wraps any LlmProvider and tries alternative models if the primary fails.

use async_trait::async_trait;
use tracing::{info, warn};

use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::message::LlmResponse;
use zenclaw_core::provider::{ChatRequest, LlmProvider};

/// Provider wrapper that supports automatic model fallback.
///
/// If the primary model fails, it tries the fallback models in order.
pub struct FallbackProvider<P: LlmProvider> {
    inner: P,
    fallback_models: Vec<String>,
}

impl<P: LlmProvider> FallbackProvider<P> {
    /// Create a new fallback provider.
    pub fn new(inner: P, fallbacks: Vec<String>) -> Self {
        Self {
            inner,
            fallback_models: fallbacks,
        }
    }
}

#[async_trait]
impl<P: LlmProvider> LlmProvider for FallbackProvider<P> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn default_model(&self) -> &str {
        self.inner.default_model()
    }

    async fn chat(&self, request: ChatRequest) -> Result<LlmResponse> {
        // Try primary model first
        match self.inner.chat(request.clone()).await {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                if self.fallback_models.is_empty() {
                    return Err(e);
                }
                warn!(
                    "Primary model failed: {}. Trying {} fallback(s)...",
                    e,
                    self.fallback_models.len()
                );
            }
        }

        // Try fallback models
        for (i, model) in self.fallback_models.iter().enumerate() {
            info!(
                "Trying fallback model {}/{}: {}",
                i + 1,
                self.fallback_models.len(),
                model
            );

            let fallback_request = ChatRequest {
                model: Some(model.clone()),
                ..request.clone()
            };

            match self.inner.chat(fallback_request).await {
                Ok(resp) => {
                    info!("Fallback model {} succeeded", model);
                    return Ok(resp);
                }
                Err(e) => {
                    warn!("Fallback model {} failed: {}", model, e);
                }
            }
        }

        Err(ZenClawError::Provider(
            "All models (primary + fallbacks) failed".to_string(),
        ))
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        self.inner.list_models().await
    }
}
