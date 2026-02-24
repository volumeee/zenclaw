//! Agent loop — the core ReAct (Reason + Act) engine.
//!
//! This is the brain of ZenClaw. It:
//! 1. Receives user messages
//! 2. Builds context (system prompt + history + memory)
//! 3. Calls the LLM provider
//! 4. Executes tool calls if any
//! 5. Loops until the agent gives a final answer
//! 6. Returns the response

use crate::error::{Result, ZenClawError};
use crate::memory::MemoryStore;
use crate::message::{ChatMessage, LlmResponse};
use crate::provider::{ChatRequest, LlmProvider};
use crate::tool::ToolRegistry;
use crate::bus::{EventBus, SystemEvent};

/// Configuration for the agent loop.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum reasoning iterations before giving up.
    pub max_iterations: usize,
    /// System prompt template.
    pub system_prompt: String,
    /// Model override (None = use provider default).
    pub model: Option<String>,
    /// Max tokens per response.
    pub max_tokens: u32,
    /// Temperature for generation.
    pub temperature: f32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            model: None,
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

/// The core agent — ties together provider, tools, and memory.
pub struct Agent {
    pub config: AgentConfig,
    pub tools: ToolRegistry,
}

impl Agent {
    /// Create a new agent with default config.
    pub fn new() -> Self {
        Self {
            config: AgentConfig::default(),
            tools: ToolRegistry::new(),
        }
    }

    /// Create a new agent with custom config.
    pub fn with_config(config: AgentConfig) -> Self {
        Self {
            config,
            tools: ToolRegistry::new(),
        }
    }

    /// Run the ReAct loop for a single user message.
    ///
    /// This is the core reasoning engine:
    /// 1. Build messages (system + history + user message)
    /// 2. Call LLM
    /// 3. If LLM returns tool calls → execute them → add results → loop
    /// 4. If LLM returns text → return as final answer
    pub async fn process(
        &self,
        provider: &dyn LlmProvider,
        memory: &dyn MemoryStore,
        user_message: &str,
        session_key: &str,
        bus: Option<&EventBus>,
    ) -> Result<String> {
        // 1. Load conversation history
        let history = memory.get_history(session_key, 50).await?;

        // 2. Build initial messages
        let mut messages = Vec::new();

        // System prompt
        messages.push(ChatMessage::system(&self.config.system_prompt));

        // Token Summarization Strategy (Context Window Control)
        // Keep calculating total string length to approximate tokens
        let mut history_to_keep = Vec::new();
        let mut history_len = 0;
        let max_history_chars = 30_000; // Roughly ~7500 tokens before it gets too huge

        for msg in history.into_iter().rev() { // Start from newest
            let content_len = msg.content.as_deref().unwrap_or("").len();
            if history_len + content_len > max_history_chars {
                if let Some(b) = bus {
                    b.publish_system(crate::bus::SystemEvent {
                        run_id: session_key.to_string(),
                        event_type: "memory_truncate".into(),
                        data: serde_json::json!({ "kept_chars": history_len }),
                    });
                }
                history_to_keep.push(ChatMessage::system(
                    "[System Note: Older conversation history has been truncated automatically to prevent memory overflow.]",
                ));
                break;
            }
            history_len += content_len;
            history_to_keep.push(msg);
        }
        
        history_to_keep.reverse();
        messages.extend(history_to_keep);

        // Current user message
        messages.push(ChatMessage::user(user_message));

        // Get tool definitions
        let tool_defs = self.tools.definitions();

        // 3. ReAct loop
        let mut iterations = 0;
        let final_response = loop {
            iterations += 1;
            if iterations > self.config.max_iterations {
                return Err(ZenClawError::MaxIterations(self.config.max_iterations));
            }

            tracing::info!(
                "Agent loop iteration {}/{}",
                iterations,
                self.config.max_iterations
            );

            if let Some(b) = bus {
                b.publish_system(SystemEvent {
                    run_id: session_key.to_string(),
                    event_type: "agent_think".into(),
                    data: serde_json::json!({ "iteration": iterations }),
                });
            }

            // Call LLM
            let request = ChatRequest {
                messages: messages.clone(),
                tools: if tool_defs.is_empty() {
                    vec![]
                } else {
                    tool_defs.clone()
                },
                model: self.config.model.clone(),
                max_tokens: self.config.max_tokens,
                temperature: self.config.temperature,
            };

            let mut retry_count = 0;
            let max_retries = 3;
            let mut backoff_ms = 1000;

            let response: LlmResponse = loop {
                match provider.chat(request.clone()).await {
                    Ok(resp) => break resp,
                    Err(e) => {
                        if retry_count >= max_retries {
                            tracing::error!("LLM Provider failed after {} retries: {}", max_retries, e);
                            return Err(e); // Can't recover
                        }
                        tracing::warn!("LLM Error: {}. Retrying in {}ms...", e, backoff_ms);
                        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        retry_count += 1;
                        backoff_ms *= 2;
                    }
                }
            };

            tracing::debug!(
                "LLM response: finish_reason={}, tool_calls={}, tokens={}",
                response.finish_reason,
                response.tool_calls.len(),
                response.usage.total_tokens,
            );

            if response.has_tool_calls() {
                // Agent wants to use tools — execute them
                let tool_calls = response.tool_calls.clone();

                // Add assistant message with tool calls to history
                messages.push(ChatMessage::assistant_with_tools(
                    response.content.as_deref(),
                    tool_calls.clone(),
                ));

                // Execute each tool call concurrently
                let mut exec_futures = Vec::new();

                for call in tool_calls {
                    if let Some(b) = bus {
                        b.publish_system(SystemEvent {
                            run_id: session_key.to_string(),
                            event_type: "tool_use".into(),
                            data: serde_json::json!({ "tool": call.function.name, "args": call.function.arguments }),
                        });
                    }

                    let args: serde_json::Value =
                        serde_json::from_str(&call.function.arguments).unwrap_or_default();

                    let fut = async move {
                        // Anti-Freeze Execution (Hard Timeout)
                        let timeout_duration = std::time::Duration::from_secs(60);
                        let execution = tokio::time::timeout(
                            timeout_duration,
                            self.tools.execute(&call.function.name, args),
                        );

                        let result = match execution.await {
                            Ok(Ok(r)) => r,
                            Ok(Err(e)) => format!("Error: {}", e),
                            Err(_) => {
                                let timeout_err_msg = format!(
                                    "Error: Tool execution timed out after {} seconds. The task might be hanging or taking too long. Please try a different approach, refine your parameters, or break the task into smaller chunks.",
                                    timeout_duration.as_secs()
                                );
                                if let Some(b) = bus {
                                    b.publish_system(crate::bus::SystemEvent {
                                        run_id: session_key.to_string(),
                                        event_type: "tool_timeout".into(),
                                        data: serde_json::json!({ "tool": &call.function.name }),
                                    });
                                }
                                timeout_err_msg
                            }
                        };
                        (call, result)
                    };
                    exec_futures.push(fut);
                }

                let results = futures::future::join_all(exec_futures).await;

                // Add tool results to messages
                for (call, result) in results {
                    messages.push(ChatMessage::tool_result(
                        &call.id,
                        &call.function.name,
                        &result,
                    ));

                    if let Some(b) = bus {
                        b.publish_system(SystemEvent {
                            run_id: session_key.to_string(),
                            event_type: "tool_result".into(),
                            data: serde_json::json!({ "tool": call.function.name, "result_len": result.len() }),
                        });
                    }
                }

                // Continue loop — LLM will see tool results and decide next step
                continue;
            }

            // No tool calls — this is the final answer
            let answer = response.content.unwrap_or_default();
            break answer;
        };

        // 4. Save to memory
        memory
            .save_turn(session_key, user_message, &final_response)
            .await?;

        tracing::info!(
            "Agent completed in {} iteration(s), response: {} chars",
            iterations,
            final_response.len()
        );

        Ok(final_response)
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

/// Default system prompt for ZenClaw.
pub const DEFAULT_SYSTEM_PROMPT: &str = r#"You are ZenClaw, a capable and helpful AI assistant.

## Core Principles
- Be helpful, accurate, and concise
- Use tools when needed to accomplish tasks
- Think step by step for complex problems
- Admit when you don't know something

## Capabilities
You have access to various tools. Use them proactively when they can help answer the user's question or accomplish their task.

When executing tasks:
1. Understand the request fully
2. Plan the approach
3. Execute using available tools
4. Verify the results
5. Report back clearly

Always prioritize accuracy over speed. If you need to use multiple tools, do so methodically."#;
