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

/// Per-request overrides — allows callers (e.g. REST API) to customize
/// system prompt, model, and output format on each request.
#[derive(Debug, Clone, Default)]
pub struct ProcessOptions {
    /// Override the system prompt for this request only.
    pub system_prompt: Option<String>,
    /// Override the model for this request only.
    pub model: Option<String>,
    /// Response format: "json" to enforce JSON output, None for default text.
    pub response_format: Option<String>,
}

/// Structured metrics for tool executions within a single process run.
#[derive(Debug, Clone, Default)]
pub struct ToolMetrics {
    pub invocations: usize,
    pub successes: usize,
    pub errors: usize,
    pub timeouts: usize,
}

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
        self.process_with_media(provider, memory, user_message, Vec::new(), session_key, bus, ProcessOptions::default()).await
    }

    /// Run the ReAct loop for a single user message containing a media payload.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_with_media(
        &self,
        provider: &dyn LlmProvider,
        memory: &dyn MemoryStore,
        user_message: &str,
        media: Vec<String>,
        session_key: &str,
        bus: Option<&EventBus>,
        opts: ProcessOptions,
    ) -> Result<String> {
        // 1. Load conversation history
        let history = memory.get_history(session_key, 50).await?;

        // 2. Build initial messages
        let mut messages = Vec::new();
        let mut tool_metrics = ToolMetrics::default();

        // System prompt — use per-request override if provided, else config default
        let mut sys_prompt = opts.system_prompt
            .unwrap_or_else(|| self.config.system_prompt.clone());

        // RAG Auto-Inject: search knowledge base for relevant context
        if let Some(b) = bus {
            b.publish_system(SystemEvent {
                run_id: session_key.to_string(),
                event_type: "rag_search".into(),
                data: serde_json::json!({ "query": user_message }),
            });
        }
        if let Ok(Some(context)) = memory.search_knowledge(user_message, 3).await
            && !context.is_empty() 
        {
            sys_prompt.push_str("\n\n");
            sys_prompt.push_str(&context);
            
            if let Some(b) = bus {
                b.publish_system(crate::bus::SystemEvent {
                    run_id: session_key.to_string(),
                    event_type: "rag_inject".into(),
                    data: serde_json::json!({ "status": "RAG Context Injected" }),
                });
            }
        }

        // Inject current date/time so the AI knows what "today" is
        let now = chrono::Local::now();
        sys_prompt.push_str(&format!(
            "\n\n## Current Date & Time\nToday is {}. CRITICAL INSTRUCTION: When asked for 'latest' ('terbaru') news or current events, YOU ARE FORBIDDEN from answering directly from memory. You MUST trigger the `web_search` tool! Do NOT hallucinate events that haven't happened yet.",
            now.format("%A, %d %B %Y, %H:%M %Z")
        ));

        messages.push(ChatMessage::system(&sys_prompt));

        // Token Summarization Strategy (Context Window Control)
        // Keep calculating total string length to approximate tokens
        let mut history_to_keep = Vec::new();
        let mut history_to_summarize = Vec::new();
        let mut history_len = 0;
        let max_history_chars = 30_000; // Roughly ~7500 tokens before it gets too huge

        for msg in history.into_iter().rev() { // Start from newest
            let content_len = msg.content.as_deref().unwrap_or("").len();
            if history_len + content_len > max_history_chars {
                history_to_summarize.push(msg);
            } else {
                history_len += content_len;
                history_to_keep.push(msg);
            }
        }
        
        history_to_keep.reverse();
        history_to_summarize.reverse();

        if !history_to_summarize.is_empty() {
            if let Some(b) = bus {
                b.publish_system(crate::bus::SystemEvent {
                    run_id: session_key.to_string(),
                    event_type: "memory_truncate".into(),
                    data: serde_json::json!({ 
                        "kept_chars": history_len,
                        "status": "Summarizing older context into a Memory Card..."
                    }),
                });
            }

            let mut summary_text = String::new();
            for m in &history_to_summarize {
                let role_str = match m.role {
                    crate::message::Role::System => "System",
                    crate::message::Role::User => "User",
                    crate::message::Role::Assistant => "Assistant",
                    crate::message::Role::Tool => "Tool",
                };
                summary_text.push_str(&format!("{}: {}\n\n", role_str, m.content.as_deref().unwrap_or("")));
            }
            
            // Limit text size to prevent exceeding LLM context limit just for the summary
            if summary_text.len() > 60_000 {
                let start = summary_text.len() - 60_000;
                summary_text = summary_text[start..].to_string();
            }

            let summary_prompt = format!(
                "Summarize the following old conversation history into a dense, highly factual 'Memory Card'. Extract key facts, user preferences, unresolved steps, key decisions, and important context. Do not include pleasantries. Keep it concise but information-dense:\n\n{}",
                summary_text
            );

            let summary_request = crate::provider::ChatRequest {
                messages: vec![crate::message::ChatMessage::user(&summary_prompt)],
                tools: vec![],
                // Can use lower tier model if configurable in the future, for now use current model
                model: opts.model.clone().or_else(|| self.config.model.clone()),
                max_tokens: 1000,
                temperature: 0.1,
                response_format: None,
            };

            let memory_card = if let Ok(resp) = provider.chat(summary_request).await {
                resp.content.unwrap_or_else(|| "Previous conversation context truncated.".into())
            } else {
                "Previous conversation context truncated.".into()
            };

            messages.push(crate::message::ChatMessage::system(&format!(
                "📚 [System Context - Memory Card]:\nThe following is a summarized memory card of older conversation history that was truncated to save space:\n---\n{}\n---",
                memory_card.trim()
            )));
        }

        messages.extend(history_to_keep);

        // Current user message
        messages.push(ChatMessage::user_with_media(user_message, media));

        // Get tool definitions
        let tool_defs = self.tools.definitions();

        // 3. ReAct loop
        let mut iterations = 0;
        let mut audited = false;
        let mut web_search_called = false; // Track if web_search has been called this session
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

            // Determine model: per-request override > agent config > provider default
            let model_override = opts.model.clone()
                .or_else(|| self.config.model.clone());

            // Determine response_format for this request
            let response_format = opts.response_format.clone();

            // Call LLM
            let request = ChatRequest {
                messages: messages.clone(),
                tools: if tool_defs.is_empty() {
                    vec![]
                } else {
                    tool_defs.clone()
                },
                model: model_override,
                max_tokens: self.config.max_tokens,
                temperature: self.config.temperature,
                response_format: response_format.clone(),
            };

            let mut retry_count = 0;
            let max_retries = 5; // Increased from 3 to allow waiting out rate limits
            let mut backoff_ms = 2000;

            let response: LlmResponse = loop {
                match provider.chat(request.clone()).await {
                    Ok(resp) => break resp,
                    Err(e) => {
                        if retry_count >= max_retries {
                            tracing::error!("LLM Provider failed after {} retries: {}", max_retries, e);
                            return Err(e);
                        }

                        let err_str = e.to_string();
                        // 429 handles "Too Many Requests" rate limit
                        let is_rate_limit = err_str.contains("429") || err_str.to_lowercase().contains("rate limit");
                        
                        let wait_ms = if is_rate_limit {
                            // Free tiers (e.g. 3 RPM) need ~20s cool-off between retries.
                            20_000
                        } else {
                            backoff_ms
                        };

                        tracing::warn!("LLM Error: {}. Retrying in {}ms...", e, wait_ms);
                        if let Some(b) = bus {
                            b.publish_system(SystemEvent {
                                run_id: session_key.to_string(),
                                event_type: "llm_retry".into(),
                                data: serde_json::json!({ 
                                    "attempt": retry_count + 1, 
                                    "error": e.to_string(),
                                    "is_rate_limit": is_rate_limit,
                                    "wait_ms": wait_ms,
                                }),
                            });
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                        
                        retry_count += 1;
                        if !is_rate_limit {
                            backoff_ms *= 2;
                        }
                    }
                }
            };

            tracing::debug!(
                "LLM response: finish_reason={}, tool_calls={}, tokens={}",
                response.finish_reason,
                response.tool_calls.len(),
                response.usage.total_tokens,
            );

            // ── Reasoning visibility ────────────────────────────
            // Publish reasoning if provided by the model, regardless of tool calls.
            if let Some(reasoning) = &response.content 
                && !reasoning.is_empty() 
                && (response.has_tool_calls() || reasoning.len() > 50) 
            {
                tracing::info!("Agent reasoning: {}", if reasoning.len() > 200 { &reasoning[..200] } else { reasoning });
                if let Some(b) = bus {
                    b.publish_system(SystemEvent {
                        run_id: session_key.to_string(),
                        event_type: "agent_reasoning".into(),
                        data: serde_json::json!({ 
                            "reasoning": reasoning,
                            "iteration": iterations
                        }),
                    });
                }
            }

            if response.has_tool_calls() {
                // Agent wants to use tools — execute them
                let mut tool_calls = response.tool_calls.clone();

                if tool_calls.len() > 5 {
                    tracing::warn!("LLM generated {} tool calls! Truncating to 5 to prevent proxy/API failures.", tool_calls.len());
                    tool_calls.truncate(5);
                }

                // Add assistant message with tool calls to history
                messages.push(ChatMessage::assistant_with_tools(
                    response.content.as_deref(),
                    tool_calls.clone(),
                ));

                // Execute each tool call concurrently
                let mut exec_futures = Vec::new();

                for call in tool_calls {
                    // Track web_search usage
                    if call.function.name == "web_search" {
                        web_search_called = true;
                    }

                    let args: serde_json::Value =
                        serde_json::from_str(&call.function.arguments).unwrap_or_default();

                    let mut is_search_engine = false;
                    let mut extracted_query = user_message.to_string();

                    if (call.function.name == "web_scrape" || call.function.name == "web_fetch") 
                        && let Some(url) = args["url"].as_str() 
                    {
                        let url_lower = url.to_lowercase();
                        let decoded = url_lower.replace("%2f", "/").replace("%3a", ":").replace("%3f", "?").replace("%3d", "=");
                        if decoded.contains("google.com/search") 
                            || decoded.contains("duckduckgo.com") 
                            || decoded.contains("bing.com/search") 
                            || decoded.contains("yahoo.com") 
                            || decoded.contains("google.com/?")
                            || decoded.contains("jina.ai/search")
                            || decoded.contains("jina.ai/?q=")
                            || decoded.contains("s.jina.ai")
                        {
                            is_search_engine = true;
                            if let Some(idx) = decoded.find("q=") {
                                let q_part = &decoded[idx + 2..];
                                let end_idx = q_part.find('&').unwrap_or(q_part.len());
                                let query_str = q_part[..end_idx].replace("+", " ");
                                if !query_str.is_empty() {
                                    extracted_query = query_str;
                                }
                            } else if decoded.contains("s.jina.ai/") {
                                // Extract from path like https://s.jina.ai/berita+terkini
                                let parts: Vec<&str> = decoded.split("s.jina.ai/").collect();
                                if parts.len() > 1 {
                                    let q_part = parts[1];
                                    let end_idx = q_part.find('?').unwrap_or(q_part.len());
                                    let query_str = q_part[..end_idx].replace("+", " ").replace("%20", " ");
                                    if !query_str.is_empty() {
                                        extracted_query = query_str;
                                    }
                                }
                            }
                        }
                    }

                    // HARD BLOCK + AUTO-REDIRECT: 
                    // 1. If web_scrape is called before web_search
                    // 2. OR if the AI is stubbornly trying to scrape/fetch a search engine page directly
                    let should_redirect = (call.function.name == "web_scrape" && !web_search_called) || is_search_engine;

                    if should_redirect {
                        tracing::warn!("INTERCEPTED: Invalid {} attempt. Auto-executing web_search with query '{}'...", call.function.name, extracted_query);
                        
                        if let Some(b) = bus {
                            b.publish_system(SystemEvent {
                                run_id: session_key.to_string(),
                                event_type: "tool_redirect".into(),
                                data: serde_json::json!({ "from": call.function.name, "to": "web_search", "reason": "auto-search interception" }),
                            });
                        }

                        // Auto-execute web_search
                        let search_args = serde_json::json!({"query": extracted_query, "lang": "auto"});
                        let search_result = match self.tools.execute("web_search", search_args).await {
                            Ok(r) => r,
                            Err(e) => format!("web_search failed: {}", e),
                        };
                        web_search_called = true;

                        let redirect_msg = format!(
                            "REDIRECTED: Your `{}` was intercepted because you either forgot to search first, or you tried to scrape a search engine directly. \
                            I automatically ran `web_search` for you instead. Here are FRESH search results with REAL URLs — \
                            pick URLs from these results ONLY:\n\n{}",
                            call.function.name,
                            search_result
                        );
                        messages.push(ChatMessage::tool_result(
                            &call.id,
                            &call.function.name,
                            &redirect_msg,
                        ));
                        continue;
                    }

                    if let Some(b) = bus {
                        b.publish_system(SystemEvent {
                            run_id: session_key.to_string(),
                            event_type: "tool_use".into(),
                            data: serde_json::json!({ "tool": call.function.name, "args": call.function.arguments }),
                        });
                    }

                    let call_name = call.function.name.clone();
                    let tool_schema = tool_defs.iter()
                        .find(|d| d.function.name == call_name)
                        .map(|d| serde_json::to_string_pretty(&d.function.parameters).unwrap_or_default())
                        .unwrap_or_else(|| "Unknown schema".into());

                    let fut = async move {
                        // Anti-Freeze Execution (Hard Timeout)
                        let timeout_duration = std::time::Duration::from_secs(60);
                        let execution = tokio::time::timeout(
                            timeout_duration,
                            self.tools.execute(&call.function.name, args),
                        );

                        let result = match execution.await {
                            Ok(Ok(r)) => r,
                            Ok(Err(e)) => format!(
                                "Tool Execution Error: {}\n\n💡 Hint: You may have used incorrect arguments. The STRICT JSON SCHEMA for '{}' is:\n{}", 
                                e, call.function.name, tool_schema
                            ),
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
                    tool_metrics.invocations += 1;
                    if result.starts_with("Error: Tool execution timed out") {
                        tool_metrics.timeouts += 1;
                    } else if result.starts_with("Tool Execution Error:") {
                        tool_metrics.errors += 1;
                    } else {
                        tool_metrics.successes += 1;
                    }

                    messages.push(ChatMessage::tool_result(
                        &call.id,
                        &call.function.name,
                        &result,
                    ));

                    if let Some(b) = bus {
                        b.publish_system(SystemEvent {
                            run_id: session_key.to_string(),
                            event_type: "tool_result".into(),
                            data: serde_json::json!({
                                "tool": call.function.name,
                                "result_len": result.len()
                            }),
                        });
                    }
                    tracing::debug!("Tool execution complete: {} ({} bytes).\n--- FULL RESULT START ---\n{}\n--- FULL RESULT END ---", call.function.name, result.len(), result);
                }

                // Continue loop — LLM will see tool results and decide next step
                continue;
            }

            // ══════════════════════════════════════════════════════
            // No tool calls — this is the candidate final answer
            // ══════════════════════════════════════════════════════
            let mut answer = response.content.unwrap_or_default();
            tracing::debug!("Candidate AI response generated ({} bytes).\n--- FULL RESPONSE START ---\n{}\n--- FULL RESPONSE END ---", answer.len(), answer);

            // ── Stalled response detection ────────────────────────
            // If the LLM returns text that *promises* to use a tool or describes
            // what it *would* do but didn't actually call one, force another iteration
            // so it actually acts instead of leaving the user hanging.
            let answer_lower = answer.to_lowercase();
            
            // Check if the answer mentions search/tool intent without having called any
            let has_search_intent = 
                // Indonesian patterns
                answer_lower.contains("saya akan menggunakan")
                || answer_lower.contains("saya akan melakukan pencarian")
                || answer_lower.contains("saya akan mencari")
                || answer_lower.contains("saya akan coba")
                || answer_lower.contains("mari saya cari")
                || answer_lower.contains("saya memahami anda ingin pencarian")
                || answer_lower.contains("pencarian yang lebih")
                || answer_lower.contains("saya telah melakukan pencarian")
                || answer_lower.contains("alat pencarian web")
                || answer_lower.contains("menggunakan alat")
                || answer_lower.contains("memanfaatkan alat")
                // English patterns
                || answer_lower.contains("let me search")
                || answer_lower.contains("i'll search")
                || answer_lower.contains("i will search")
                || answer_lower.contains("i will use") && answer_lower.contains("tool")
                || answer_lower.contains("let me use the")
                || answer_lower.contains("i'll use the")
                || answer_lower.contains("searching for")
                || answer_lower.contains("let me look")
                || answer_lower.contains("i'll look up");

            // A response that claims to search/use tools but doesn't is ALWAYS a stalled
            // hallucination, regardless of how incredibly long the fake answer is.
            let is_stalled_promise = iterations < self.config.max_iterations.saturating_sub(1)
                && has_search_intent;

            if is_stalled_promise {
                tracing::warn!("Stalled promise detected: LLM said it would use tools but didn't. Forcing re-iteration. Answer was: {}", answer);
                if let Some(b) = bus {
                    b.publish_system(SystemEvent {
                        run_id: session_key.to_string(),
                        event_type: "agent_reasoning".into(),
                        data: serde_json::json!({ 
                            "reasoning": "Agent promised to use tools but didn't — forcing actual tool execution...",
                            "iteration": iterations
                        }),
                    });
                }
                messages.push(ChatMessage::assistant(&answer));
                messages.push(ChatMessage::user(
                    "CRITICAL: You just described what you WOULD do but didn't actually call any tool. \
                     Stop explaining and EXECUTE the function call NOW using the native tool calling schema. \
                     Do not output natural language, just trigger the tool immediately."
                ));
                continue;
            }

            // ── Empty answer guard ────────────────────────────────
            // If the answer is completely empty, don't silently return nothing.
            if answer.trim().is_empty() {
                if iterations < self.config.max_iterations.saturating_sub(1) {
                    tracing::warn!("Empty answer received. Retrying...");
                    messages.push(ChatMessage::user(
                        "You returned an empty response. Please provide a proper answer to my question."
                    ));
                    continue;
                } else {
                    answer = "Maaf, saya tidak bisa memberikan jawaban saat ini. Silakan coba lagi atau periksa konfigurasi API key di Settings.".to_string();
                }
            }

            // JSON output mode: validate and auto-retry once if invalid
            if let Some(ref fmt) = response_format 
                && fmt == "json" 
                && serde_json::from_str::<serde_json::Value>(&answer).is_err() 
            {
                tracing::warn!("JSON output mode: response is not valid JSON, retrying once...");
                if let Some(b) = bus {
                    b.publish_system(SystemEvent {
                        run_id: session_key.to_string(),
                        event_type: "json_retry".into(),
                        data: serde_json::json!({ "reason": "invalid_json" }),
                    });
                }
                messages.push(ChatMessage::assistant(&answer));
                messages.push(ChatMessage::user(
                    "Your previous response was not valid JSON. Please respond ONLY with a valid JSON object or array. No markdown, no code fences, no explanation — just raw JSON."
                ));
                let retry_request = ChatRequest {
                    messages: messages.clone(),
                    tools: vec![],
                    model: opts.model.clone().or_else(|| self.config.model.clone()),
                    max_tokens: self.config.max_tokens,
                    temperature: self.config.temperature,
                    response_format: response_format.clone(),
                };
                if let Ok(retry_resp) = provider.chat(retry_request).await {
                    answer = retry_resp.content.unwrap_or_default();
                }
            }

            // ── Short answer re-expansion ─────────────────────────
            // If the answer is too short for a question that likely needs detail,
            // ask the LLM to elaborate — prevents lazy one-liner responses.
            let needs_expansion = answer.len() < 200
                && !user_message.starts_with('/')
                && response_format.is_none()
                && (user_message.contains('?')
                    || user_message.to_lowercase().starts_with("coba")
                    || user_message.to_lowercase().starts_with("jelaskan")
                    || user_message.to_lowercase().starts_with("explain")
                    || user_message.to_lowercase().starts_with("how")
                    || user_message.to_lowercase().starts_with("why")
                    || user_message.to_lowercase().starts_with("what")
                    || user_message.to_lowercase().starts_with("kenapa")
                    || user_message.to_lowercase().starts_with("apa")
                    || user_message.to_lowercase().starts_with("bagaimana")
                    || user_message.to_lowercase().starts_with("analyze")
                    || user_message.to_lowercase().starts_with("analisa"));

            if needs_expansion {
                tracing::info!("Answer too short ({} chars) for a detailed question, requesting elaboration...", answer.len());
                if let Some(b) = bus {
                    b.publish_system(SystemEvent {
                        run_id: session_key.to_string(),
                        event_type: "answer_expansion".into(),
                        data: serde_json::json!({ "original_len": answer.len() }),
                    });
                }
                messages.push(ChatMessage::assistant(&answer));
                messages.push(ChatMessage::user(
                    "Your answer is too brief. Please provide a much more detailed, comprehensive, and thorough response. Include examples, explanations, and context. The user expects a complete and helpful answer."
                ));
                let expand_request = ChatRequest {
                    messages: messages.clone(),
                    tools: vec![],
                    model: opts.model.clone().or_else(|| self.config.model.clone()),
                    max_tokens: self.config.max_tokens,
                    temperature: self.config.temperature,
                    response_format: None,
                };
                if let Ok(expanded) = provider.chat(expand_request).await {
                    let expanded_answer = expanded.content.unwrap_or_default();
                    if expanded_answer.len() > answer.len() {
                        answer = expanded_answer;
                    }
                }
            }

            // ── AI Self-Audit: Quality evaluation loop ────────────
            // Before returning, run a quick audit to check if the answer
            // actually addresses the user's question properly.
            // Only audit non-trivial answers (skip greetings, short commands, etc.)
            // Only audit ONCE per run — never double-fire.
            let should_audit = !audited
                && answer.len() > 100
                && response_format.is_none()
                && iterations < self.config.max_iterations.saturating_sub(2);

            if should_audit {
                #[allow(unused_assignments)]
                {
                    audited = true;
                }
                if let Some(b) = bus {
                    b.publish_system(SystemEvent {
                        run_id: session_key.to_string(),
                        event_type: "answer_audit".into(),
                        data: serde_json::json!({ "status": "Evaluating answer quality..." }),
                    });
                }

                let audit_prompt = format!(
                    r#"You are an AI answer quality auditor. Evaluate this answer against the original question.

## Original Question
{}

## Answer to Evaluate
{}

## Evaluation Criteria
1. **Completeness**: Does the answer fully address all parts of the question?
2. **Accuracy**: Is the information correct and well-reasoned?
3. **Relevance**: Does the answer stay on topic and directly help the user?
4. **Depth**: Is the answer sufficiently detailed and thorough?

## Your Task
Respond with EXACTLY this JSON format (no other text):
{{"score": <1-10>, "pass": <true if score >= 7>, "feedback": "<specific improvement suggestion if score < 7, or 'PASS' if good>"}}
"#,
                    user_message,
                    if answer.len() > 2000 { &answer[..2000] } else { &answer }
                );

                let audit_request = ChatRequest {
                    messages: vec![
                        ChatMessage::system("You are a strict answer quality auditor. Respond ONLY with JSON."),
                        ChatMessage::user(&audit_prompt),
                    ],
                    tools: vec![],
                    model: opts.model.clone().or_else(|| self.config.model.clone()),
                    max_tokens: 1000, // Increased to prevent weird cutoff issues in some proxies
                    temperature: 0.1, // Low temperature for consistent evaluation
                    response_format: Some("json".to_string()),
                };

                if let Ok(audit_resp) = provider.chat(audit_request).await {
                    let audit_text = audit_resp.content.unwrap_or_default();
                    tracing::info!("Raw Audit result: {}", audit_text);

                    // Clean the text in case the LLM returned markdown (e.g. ```json ... ```)
                    let cleaned_text = audit_text
                        .trim()
                        .trim_start_matches("```json")
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim();

                    if let Ok(audit_json) = serde_json::from_str::<serde_json::Value>(cleaned_text) {
                        let score = audit_json["score"].as_u64().unwrap_or(10);
                        let pass = audit_json["pass"].as_bool().unwrap_or(true);
                        let feedback = audit_json["feedback"].as_str().unwrap_or("PASS");

                        if let Some(b) = bus {
                            b.publish_system(SystemEvent {
                                run_id: session_key.to_string(),
                                event_type: "audit_result".into(),
                                data: serde_json::json!({
                                    "score": score,
                                    "pass": pass,
                                    "feedback": feedback
                                }),
                            });
                        }

                        if !pass && score < 7 {
                            tracing::info!("Audit FAILED (score: {}). Refining answer with feedback: {}", score, feedback);
                            if let Some(b) = bus {
                                b.publish_system(SystemEvent {
                                    run_id: session_key.to_string(),
                                    event_type: "answer_refine".into(),
                                    data: serde_json::json!({ 
                                        "status": format!("Score {}/10 — refining answer...", score),
                                        "feedback": feedback
                                    }),
                                });
                            }

                            // Refine the answer using audit feedback
                            messages.push(ChatMessage::assistant(&answer));
                            messages.push(ChatMessage::user(&format!(
                                "A quality audit found your answer could be improved. Score: {}/10.\n\nFeedback: {}\n\nPlease provide an improved, more complete answer that addresses this feedback. Be thorough and comprehensive.",
                                score, feedback
                            )));

                            let refine_request = ChatRequest {
                                messages: messages.clone(),
                                tools: vec![],
                                model: opts.model.clone().or_else(|| self.config.model.clone()),
                                max_tokens: self.config.max_tokens,
                                temperature: self.config.temperature,
                                response_format: None,
                            };

                            if let Ok(refined) = provider.chat(refine_request).await {
                                let refined_answer = refined.content.unwrap_or_default();
                                if !refined_answer.is_empty() && refined_answer.len() >= answer.len() / 2 {
                                    answer = refined_answer;
                                    tracing::info!("Answer refined successfully ({} chars)", answer.len());
                                }
                            }
                            
                            // Mark refinement as complete for UI
                            if let Some(b) = bus {
                                b.publish_system(SystemEvent {
                                    run_id: session_key.to_string(),
                                    event_type: "refine_complete".into(),
                                    data: serde_json::json!({ "status": "Refinement done" }),
                                });
                            }
                        } else {
                            tracing::info!("Audit PASSED (score: {})", score);
                        }
                    } else {
                        // Fail-safe: if JSON parsing completely fails (e.g. cut off), try to detect if it was failing
                        let mut pass = true;
                        let mut score = 10;
                        if cleaned_text.replace(" ", "").contains("\"pass\":false") {
                            pass = false;
                            score = 5;
                            tracing::warn!("Failed to parse complete audit JSON, but detected explicit FAIL. Refining...");
                        } else {
                            tracing::warn!("Failed to parse audit JSON. Assuming PASS.");
                        }
                        
                        if let Some(b) = bus {
                            b.publish_system(SystemEvent {
                                run_id: session_key.to_string(),
                                event_type: "audit_result".into(),
                                data: serde_json::json!({
                                    "score": score,
                                    "pass": pass,
                                    "feedback": if pass { "Parse error, auto-passing" } else { "Parse error, but failed audit" }
                                }),
                            });
                        }
                        
                        if !pass {
                            // Trigger refinement loop anyway if we detected a fail
                            tracing::info!("Audit FAILED (fallback). Refining answer...");
                            if let Some(b) = bus {
                                b.publish_system(SystemEvent {
                                    run_id: session_key.to_string(),
                                    event_type: "answer_refine".into(),
                                    data: serde_json::json!({ 
                                        "status": "Answer did not pass quality check — refining...",
                                        "feedback": "Answer was deemed insufficient or incomplete."
                                    }),
                                });
                            }

                            messages.push(ChatMessage::assistant(&answer));
                            messages.push(ChatMessage::user(
                                "Your answer did not pass the quality check. Please provide a much more complete, thorough, and accurate answer."
                            ));

                            let refine_request = ChatRequest {
                                messages: messages.clone(),
                                tools: vec![],
                                model: opts.model.clone().or_else(|| self.config.model.clone()),
                                max_tokens: self.config.max_tokens,
                                temperature: self.config.temperature,
                                response_format: None,
                            };

                            if let Ok(refined) = provider.chat(refine_request).await {
                                let refined_answer = refined.content.unwrap_or_default();
                                if !refined_answer.is_empty() && refined_answer.len() >= answer.len() / 2 {
                                    answer = refined_answer;
                                    tracing::info!("Answer refined successfully ({} chars)", answer.len());
                                }
                            }
                            
                            if let Some(b) = bus {
                                b.publish_system(SystemEvent {
                                    run_id: session_key.to_string(),
                                    event_type: "refine_complete".into(),
                                    data: serde_json::json!({ "status": "Refinement done" }),
                                });
                            }
                        }
                    }
                } else {
                    // Fail-safe: if provider chat fails completely
                    if let Some(b) = bus {
                        b.publish_system(SystemEvent {
                            run_id: session_key.to_string(),
                            event_type: "audit_result".into(),
                            data: serde_json::json!({
                                "score": 10,
                                "pass": true,
                                "feedback": "API error, auto-passing"
                            }),
                        });
                    }
                }
            }

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

        if tool_metrics.invocations > 0 {
            tracing::info!(
                "Tool Metrics - Invocations: {}, Successes: {}, Errors: {}, Timeouts: {}",
                tool_metrics.invocations, tool_metrics.successes, tool_metrics.errors, tool_metrics.timeouts
            );
        }

        Ok(final_response)
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

/// Default system prompt for ZenClaw.
pub const DEFAULT_SYSTEM_PROMPT: &str = r#"You are ZenClaw, a capable and highly thorough AI assistant.

## Core Principles
- Be helpful, accurate, and THOROUGH — always provide complete, detailed answers
- Use tools proactively when they can help accomplish tasks
- Think step by step for complex problems
- Never give lazy one-liner answers — users expect comprehensive help
- Admit when you don't know something, but try your best first

## Reasoning & Execution
- DO NOT summarize or narrate your tool intentions to the user (e.g. don't say "I will use the web_fetch tool..."). 
- NEVER output raw tool names as text. If you need a tool, just trigger its native function call mechanism silently!
- If you need to search or fetch, just do it directly.

## Response Quality Standards
- For questions: provide detailed explanations with examples
- For tasks: plan carefully, execute methodically, verify results
- For analysis: be comprehensive and cover multiple angles
- Always consider edge cases and potential issues

## Capabilities & Tool Selection
You have access to various tools. Use them proactively.

### MANDATORY WORKFLOW for any internet research:
1. FIRST: Call `web_search` to find relevant URLs and snippets.
2. THEN (only if needed): Call `web_scrape` on URLs returned BY `web_search` to read full article content.
3. NEVER skip step 1. NEVER call `web_scrape` on a URL you invented, remembered, or guessed from training data.

### Tool Rules:
- `web_search` (Jina Search): Your PRIMARY tool for ALL internet lookups. Always call this FIRST.
- `web_scrape` (Jina Reader): ONLY for reading the full text of a specific URL that was already found via `web_search`.
- `web_fetch`: ONLY for raw API/JSON endpoints. Never for HTML pages.
- CRITICAL - NO MEMORY ANSWERS FOR NEWS: If the user asks for 'berita terbaru', 'latest news', or current events, YOU ARE STRICTLY FORBIDDEN from answering using your internal memory or training data. You MUST call `web_search` immediately. DO NOT hallucinate or guess events. DO NOT pretend you searched if you haven't triggered the tool.
- CRITICAL - NO HALLUCINATED URLS: NEVER invent, hallucinate, or guess URLs. URLs from your training data are likely outdated. You MUST obtain fresh URLs from `web_search`.

When executing tasks:
1. Understand the request fully — ask if unclear
2. Plan the approach step by step
3. Execute using available native tools blindly and accurately
4. Verify the results thoroughly
5. Report back with a clear, detailed summary

Always prioritize accuracy and completeness. If you need to use multiple tools, do so methodically. Quality matters more than speed."#;

