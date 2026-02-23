//! Cron/scheduler tool — run tasks on a schedule.
//!
//! Allows the agent to schedule one-shot delayed tasks.
//! Useful for reminders, periodic checks, and delayed actions.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tracing::info;

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// A scheduled task.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ScheduledTask {
    id: String,
    description: String,
    delay_secs: u64,
    command: String,
    status: TaskStatus,
}

#[derive(Debug, Clone)]
enum TaskStatus {
    Pending,
    Running,
    Done(String),
    Failed(String),
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "⏳ pending"),
            Self::Running => write!(f, "🔄 running"),
            Self::Done(r) => write!(f, "✅ done: {}", r),
            Self::Failed(e) => write!(f, "❌ failed: {}", e),
        }
    }
}

/// Cron tool — schedule delayed tasks.
pub struct CronTool {
    tasks: Arc<Mutex<Vec<ScheduledTask>>>,
}

impl CronTool {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for CronTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }

    fn description(&self) -> &str {
        "Schedule a delayed task or list scheduled tasks. Actions: 'schedule' (run a shell command after delay), 'list' (show all tasks)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["schedule", "list"],
                    "description": "Action: 'schedule' or 'list'"
                },
                "command": {
                    "type": "string",
                    "description": "Shell command to run (for 'schedule')"
                },
                "delay_seconds": {
                    "type": "integer",
                    "description": "Delay in seconds before running (for 'schedule')"
                },
                "description": {
                    "type": "string",
                    "description": "Human-readable description of the task"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let action = args["action"].as_str().unwrap_or("list");

        match action {
            "schedule" => {
                let command = args["command"].as_str().unwrap_or("").to_string();
                let delay = args["delay_seconds"].as_u64().unwrap_or(60);
                let desc = args["description"]
                    .as_str()
                    .unwrap_or("Scheduled task")
                    .to_string();

                if command.is_empty() {
                    return Ok("Error: command is required for 'schedule' action".to_string());
                }

                let id = format!("task_{}", chrono::Utc::now().timestamp());

                let task = ScheduledTask {
                    id: id.clone(),
                    description: desc.clone(),
                    delay_secs: delay,
                    command: command.clone(),
                    status: TaskStatus::Pending,
                };

                self.tasks.lock().await.push(task);

                // Spawn delayed execution
                let tasks = self.tasks.clone();
                let task_id = id.clone();
                let spawn_desc = desc.clone();
                let spawn_cmd = command.clone();
                tokio::spawn(async move {
                    info!("⏰ Task {} scheduled: {} in {}s", task_id, spawn_desc, delay);
                    tokio::time::sleep(Duration::from_secs(delay)).await;

                    // Update status
                    if let Some(task) = tasks.lock().await.iter_mut().find(|t| t.id == task_id) {
                        task.status = TaskStatus::Running;
                    }

                    // Execute
                    let result = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(&spawn_cmd)
                        .output()
                        .await;

                    match result {
                        Ok(output) => {
                            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                            if let Some(task) =
                                tasks.lock().await.iter_mut().find(|t| t.id == task_id)
                            {
                                task.status = if output.status.success() {
                                    TaskStatus::Done(stdout.trim().to_string())
                                } else {
                                    let stderr =
                                        String::from_utf8_lossy(&output.stderr).to_string();
                                    TaskStatus::Failed(stderr.trim().to_string())
                                };
                            }
                        }
                        Err(e) => {
                            if let Some(task) =
                                tasks.lock().await.iter_mut().find(|t| t.id == task_id)
                            {
                                task.status = TaskStatus::Failed(e.to_string());
                            }
                        }
                    }
                });

                Ok(format!(
                    "✅ Scheduled: {}\n  ID: {}\n  Command: {}\n  Delay: {}s",
                    desc, id, command, delay
                ))
            }
            "list" => {
                let tasks = self.tasks.lock().await;
                if tasks.is_empty() {
                    return Ok("No scheduled tasks.".to_string());
                }

                let list = tasks
                    .iter()
                    .map(|t| {
                        format!(
                            "• {} — {} ({})",
                            t.id, t.description, t.status
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(format!("Scheduled Tasks:\n{}", list))
            }
            _ => Ok(format!("Unknown action: {}. Use 'schedule' or 'list'.", action)),
        }
    }
}
