//! Cron/scheduler tool — run tasks on a schedule (Persistent Background Worker).
//!
//! Allows the agent to schedule one-shot or periodic cron jobs.

use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use cron::Schedule;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::{error, info, warn};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Persistent Cron tool — schedule and execute background tasks via SQLite.
pub struct CronTool {
    db_path: PathBuf,
}

impl CronTool {
    pub fn new() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("zenclaw")
            .join("cron");
        std::fs::create_dir_all(&data_dir).unwrap_or_default();
        let db_path = data_dir.join("cron.db");

        let tool = Self { db_path };
        tool.init_db();
        tool.start_worker();
        tool
    }

    fn init_db(&self) {
        if let Ok(conn) = Connection::open(&self.db_path) {
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS cron_jobs (
                    id TEXT PRIMARY KEY,
                    description TEXT NOT NULL,
                    command TEXT NOT NULL,
                    schedule TEXT,
                    next_run INTEGER NOT NULL,
                    status TEXT NOT NULL
                )",
                [],
            );
        } else {
            warn!("Failed to open cron.db. Background worker may not persist tasks.");
        }
    }

    #[allow(clippy::collapsible_if)]
    fn start_worker(&self) {
        let db_path = self.db_path.clone();
        
        tokio::spawn(async move {
            info!("⚙️ Persistent Background Worker started.");
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;

                let now = Utc::now().timestamp();
                let mut jobs_to_run = Vec::new();

                if let Ok(conn) = Connection::open(&db_path) {
                    // Find pending tasks whose next_run <= now
                    if let Ok(mut stmt) = conn.prepare(
                        "SELECT id, command, schedule, description FROM cron_jobs 
                         WHERE next_run <= ?1 AND status = 'pending'",
                    ) {
                        if let Ok(rows) = stmt.query_map([now], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, Option<String>>(2)?,
                                row.get::<_, String>(3)?,
                            ))
                        }) {
                            for row in rows.flatten() {
                                jobs_to_run.push(row);
                            }
                        }
                    }

                    for (id, command, schedule_opt, desc) in jobs_to_run {
                        // Mark as running
                        let _ = conn.execute(
                            "UPDATE cron_jobs SET status = 'running' WHERE id = ?1",
                            [&id],
                        );

                        info!("⏰ Running Cron Job: {} ({})", desc, id);
                        let cmd_clone = command.clone();
                        let id_clone = id.clone();
                        let db_path_clone = db_path.clone();
                        let sched_clone = schedule_opt.clone();

                        // Execute the command in the background
                        tokio::spawn(async move {
                            #[cfg(target_os = "windows")]
                            let (shell, arg) = ("cmd", "/C");
                            #[cfg(not(target_os = "windows"))]
                            let (shell, arg) = ("sh", "-c");

                            let result = tokio::process::Command::new(shell)
                                .arg(arg)
                                .arg(&cmd_clone)
                                .output()
                                .await;

                            let new_status = if let Ok(out) = result {
                                if out.status.success() {
                                    info!("✅ Cron Job {} Success", id_clone);
                                    "done"
                                } else {
                                    error!("❌ Cron Job {} Failed: {}", id_clone, String::from_utf8_lossy(&out.stderr));
                                    "failed"
                                }
                            } else {
                                "failed"
                            };

                            // Update DB: either set next schedule or mark completed/failed
                            if let Ok(conn2) = Connection::open(&db_path_clone) {
                                if let Some(sch_str) = sched_clone {
                                    if let Ok(schedule) = Schedule::from_str(&sch_str) {
                                        // If valid schedule, get next run
                                        if let Some(next) = schedule.upcoming(Utc).next() {
                                            let _ = conn2.execute(
                                                "UPDATE cron_jobs SET status = 'pending', next_run = ?1 WHERE id = ?2",
                                                rusqlite::params![next.timestamp(), id_clone],
                                            );
                                            return;
                                        }
                                    }
                                }
                                
                                // One-shot task or invalid cron expr marks as done/failed.
                                let _ = conn2.execute(
                                    "UPDATE cron_jobs SET status = ?1 WHERE id = ?2",
                                    rusqlite::params![new_status, id_clone],
                                );
                            }
                        });
                    }
                }
            }
        });
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
        "Background Task Scheduler. Creates persistent background processes. 
Actions: 
- 'schedule' (one-time command execution after delay_seconds)
- 'cron' (periodic command execution using 7-field standard cron string e.g. '0 30 9 * * * *' : sec min hour dom mon dow year).
- 'agent_task' (periodic or scheduled execution of an AI agent task, prompt goes into 'command' param, uses cron_expression if provided otherwise delay)
- 'list' (list all active/completed jobs)
- 'delete' (delete a job via ID)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["schedule", "cron", "agent_task", "list", "delete"],
                    "description": "Action type"
                },
                "command": {
                    "type": "string",
                    "description": "Shell command to execute."
                },
                "delay_seconds": {
                    "type": "integer",
                    "description": "Delay before first run (only for 'schedule' action)."
                },
                "cron_expression": {
                    "type": "string",
                    "description": "7-field cron string (only for 'cron' action). E.g. '0 0 12 * * * *'"
                },
                "description": {
                    "type": "string",
                    "description": "A short memo of what the task does."
                },
                "job_id": {
                    "type": "string",
                    "description": "ID of the job (required for 'delete')."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let action = args["action"].as_str().unwrap_or("list");
        let conn = Connection::open(&self.db_path)
            .map_err(|e: rusqlite::Error| zenclaw_core::error::ZenClawError::Other(format!("DB error: {}", e)))?;

        match action {
            "schedule" | "cron" | "agent_task" => {
                let command = args["command"].as_str().unwrap_or("").to_string();
                let desc = args["description"]
                    .as_str()
                    .unwrap_or("Scheduled Task")
                    .to_string();

                if command.is_empty() {
                    return Ok("Error: 'command' required for scheduling.".into());
                }

                let id = format!("task_{}", Utc::now().timestamp());
                let (next_run, schedule_str) = if action == "cron" || (action == "agent_task" && args.get("cron_expression").is_some()) {
                    let expr = args["cron_expression"].as_str().unwrap_or("");
                    match Schedule::from_str(expr) {
                        Ok(sch) => {
                            let next = sch.upcoming(Utc).next()
                                .ok_or_else(|| zenclaw_core::error::ZenClawError::Other("Cron expression has no future runs".into()))?;
                            (next.timestamp(), Some(expr.to_string()))
                        }
                        Err(e) => return Ok(format!("Error parsing cron: {}", e)),
                    }
                } else {
                    let delay = args["delay_seconds"].as_u64().unwrap_or(60);
                    (Utc::now().timestamp() + delay as i64, None)
                };

                let final_command = if action == "agent_task" {
                    // Prepend standard binary path
                    let exe_path = std::env::current_exe()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "zenclaw".to_string());
                    format!("{} ask \"{}\"", exe_path, command.replace("\"", "\\\""))
                } else {
                    command
                };

                conn.execute(
                    "INSERT INTO cron_jobs (id, description, command, schedule, next_run, status) 
                     VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
                    rusqlite::params![id, desc, final_command, schedule_str, next_run],
                ).map_err(|e| zenclaw_core::error::ZenClawError::Other(e.to_string()))?;

                Ok(format!("✅ Scheduled: {}\n  ID: {}\n  Target: {}", desc, id, next_run))
            }
            "list" => {
                let mut stmt = conn.prepare("SELECT id, description, status, next_run, schedule FROM cron_jobs ORDER BY next_run DESC LIMIT 20")
                    .map_err(|e: rusqlite::Error| zenclaw_core::error::ZenClawError::Other(e.to_string()))?;
                
                let rows = stmt.query_map([], |row| {
                    Ok(format!(
                        "• {} | {} | {} | next:{} | sched:{}", 
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<String>>(4)?.unwrap_or_else(|| "none".into())
                    ))
                }).map_err(|e: rusqlite::Error| zenclaw_core::error::ZenClawError::Other(e.to_string()))?;

                let results: Vec<String> = rows.filter_map(|r| r.ok()).collect();
                if results.is_empty() {
                    Ok("No tasks in database.".into())
                } else {
                    Ok(format!("Scheduled Tasks:\n{}", results.join("\n")))
                }
            }
            "delete" => {
                let job_id = args["job_id"].as_str().unwrap_or("");
                if job_id.is_empty() {
                    return Ok("Error: 'job_id' is required.".into());
                }

                let deleted = conn.execute("DELETE FROM cron_jobs WHERE id = ?1", [job_id])
                    .map_err(|e: rusqlite::Error| zenclaw_core::error::ZenClawError::Other(e.to_string()))?;

                if deleted > 0 {
                    Ok(format!("✅ Job {} deleted.", job_id))
                } else {
                    Ok(format!("❌ Job {} not found.", job_id))
                }
            }
            _ => Ok("Unknown action.".into())
        }
    }
}
