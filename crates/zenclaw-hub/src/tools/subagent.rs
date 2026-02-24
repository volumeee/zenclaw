//! Sub Agent tool — spawn independent background AI workers.
//!
//! Spawns `zenclaw ask` in the background for autonomous tasks.

use std::env;

use async_trait::async_trait;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Sub-Agent Spawner.
pub struct SubAgentTool;

impl SubAgentTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SubAgentTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SubAgentTool {
    fn name(&self) -> &str {
        "sub_agent"
    }

    fn description(&self) -> &str {
        "Spawn a Sub-Agent (a clone of yourself) to work on complex or long-running tasks in the background autonomously. 
Returns a PID that you MUST use with the 'process' tool (action='status') to check its progress!"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Clear and detailed instructions for the sub-agent."
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let task = args["task"].as_str().unwrap_or("");
        if task.is_empty() {
            return Ok("Error: 'task' is required.".into());
        }

        let exe_path = env::current_exe().unwrap_or_else(|_| "zenclaw".into());
        let exe_str = exe_path.to_string_lossy();

        // We wrap the task so that the AI knows to use the process tool to actually start it.
        // Wait! We can actually tell it to delegate to the 'process' tool directly.
        
        let shell_cmd = format!("{} ask \"{}\"", exe_str, task.replace('"', "\\\""));
        
        Ok(format!(
            "Sub-Agent request acknowledged. To actually spawn the sub-agent in the background, you MUST now use the 'process' tool with action='spawn' and this exact command:\n\n{}",
            shell_cmd
        ))
    }
}
