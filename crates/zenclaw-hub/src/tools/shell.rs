//! Shell execution tool — run commands on the system.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::Command;

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Execute shell commands. Core tool for system interaction.
pub struct ShellTool {
    /// Working directory for commands.
    pub working_dir: Option<String>,
    /// Maximum output length in bytes.
    pub max_output: usize,
}

impl ShellTool {
    pub fn new() -> Self {
        Self {
            working_dir: None,
            max_output: 10_000,
        }
    }

    pub fn with_working_dir(mut self, dir: &str) -> Self {
        self.working_dir = Some(dir.to_string());
        self
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "exec"
    }

    fn description(&self) -> &str {
        "Execute a shell command on the system. Returns stdout, stderr, and exit code."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory (optional)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let command = args["command"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let cwd = args["working_dir"]
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| self.working_dir.clone());

        tracing::info!("Executing: {}", command);

        #[cfg(target_os = "windows")]
        let (shell, arg) = ("cmd", "/C");
        #[cfg(not(target_os = "windows"))]
        let (shell, arg) = ("sh", "-c");

        let mut cmd = Command::new(shell);
        cmd.arg(arg)
            .arg(&command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref dir) = cwd {
            cmd.current_dir(dir);
        }

        let output = cmd.output().await.map_err(|e| {
            zenclaw_core::error::ZenClawError::ToolExecution {
                tool: "exec".to_string(),
                message: format!("Failed to execute: {}", e),
            }
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        // Truncate output if too long
        let truncate = |s: &str| -> String {
            if s.len() > self.max_output {
                format!(
                    "{}... [truncated, {} total bytes]",
                    &s[..self.max_output],
                    s.len()
                )
            } else {
                s.to_string()
            }
        };

        let result = format!(
            "Exit code: {}\n\n--- stdout ---\n{}\n--- stderr ---\n{}",
            exit_code,
            truncate(&stdout),
            truncate(&stderr)
        );

        Ok(result)
    }
}
