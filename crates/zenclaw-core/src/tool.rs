//! Tool system — define capabilities the agent can use.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::{Result, ZenClawError};
use crate::provider::ToolDefinition;

/// Abstract tool trait — implement this to give the agent new abilities.
///
/// # Example
///
/// ```rust,ignore
/// struct HelloTool;
///
/// #[async_trait]
/// impl Tool for HelloTool {
///     fn name(&self) -> &str { "hello" }
///     fn description(&self) -> &str { "Say hello to someone" }
///     fn parameters(&self) -> Value {
///         serde_json::json!({
///             "type": "object",
///             "properties": {
///                 "name": { "type": "string", "description": "Person's name" }
///             },
///             "required": ["name"]
///         })
///     }
///
///     async fn execute(&self, args: Value) -> Result<String> {
///         let name = args["name"].as_str().unwrap_or("World");
///         Ok(format!("Hello, {}!", name))
///     }
/// }
/// ```
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name used in function calls.
    fn name(&self) -> &str;

    /// Description of what the tool does.
    fn description(&self) -> &str;

    /// JSON Schema for tool parameters.
    fn parameters(&self) -> Value;

    /// Execute the tool with given arguments.
    async fn execute(&self, args: Value) -> Result<String>;

    /// Convert to OpenAI tool definition format.
    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            r#type: "function".to_string(),
            function: crate::provider::FunctionDefinition {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: self.parameters(),
            },
        }
    }
}

/// Registry for managing tools. Core component of the agent.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a new tool.
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        tracing::debug!("Registered tool: {}", name);
        self.tools.insert(name, Box::new(tool));
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Check if a tool is registered.
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all tool definitions for LLM function calling.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.to_definition()).collect()
    }

    /// Execute a tool by name.
    pub async fn execute(&self, name: &str, args: Value) -> Result<String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ZenClawError::ToolNotFound(name.to_string()))?;

        tracing::info!("Executing tool: {} with args: {}", name, args);

        match tool.execute(args).await {
            Ok(result) => {
                tracing::debug!("Tool {} completed ({} bytes)", name, result.len());
                Ok(result)
            }
            Err(e) => {
                tracing::error!("Tool {} failed: {}", name, e);
                Err(ZenClawError::ToolExecution {
                    tool: name.to_string(),
                    message: e.to_string(),
                })
            }
        }
    }

    /// List all registered tool names.
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
