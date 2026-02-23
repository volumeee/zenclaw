//! History export tool — export/import conversation history.
//!
//! Supports JSON and Markdown export formats.
//! Useful for backup, sharing, and analysis.

use async_trait::async_trait;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// History export tool.
pub struct HistoryTool;

impl HistoryTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HistoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HistoryTool {
    fn name(&self) -> &str {
        "history"
    }

    fn description(&self) -> &str {
        "Export conversation history to a file. Supports JSON and Markdown formats. Also can show recent sessions."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["export", "sessions"],
                    "description": "Action: 'export' to save history, 'sessions' to list sessions"
                },
                "format": {
                    "type": "string",
                    "enum": ["json", "markdown"],
                    "description": "Export format (default: markdown)"
                },
                "output": {
                    "type": "string",
                    "description": "Output file path"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let action = args["action"].as_str().unwrap_or("sessions");

        match action {
            "sessions" => {
                // List SQLite sessions
                let data_dir = dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("zenclaw");

                let db_path = data_dir.join("memory.db");
                if !db_path.exists() {
                    return Ok("No conversation history found.".to_string());
                }

                let conn = rusqlite::Connection::open(&db_path)
                    .map_err(|e| zenclaw_core::error::ZenClawError::Memory(e.to_string()))?;

                let mut stmt = conn
                    .prepare(
                        "SELECT session_key, COUNT(*) as msg_count, MAX(created_at) as last_msg
                         FROM history
                         GROUP BY session_key
                         ORDER BY last_msg DESC
                         LIMIT 20",
                    )
                    .map_err(|e| zenclaw_core::error::ZenClawError::Memory(e.to_string()))?;

                let sessions: Vec<String> = stmt
                    .query_map([], |row| {
                        let key: String = row.get(0)?;
                        let count: i64 = row.get(1)?;
                        let last: String = row.get(2)?;
                        Ok(format!("• {} ({} messages, last: {})", key, count, last))
                    })
                    .map_err(|e| zenclaw_core::error::ZenClawError::Memory(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                if sessions.is_empty() {
                    Ok("No sessions found.".to_string())
                } else {
                    Ok(format!("Sessions:\n{}", sessions.join("\n")))
                }
            }
            "export" => {
                let format = args["format"].as_str().unwrap_or("markdown");
                let output = args["output"]
                    .as_str()
                    .unwrap_or(if format == "json" {
                        "history.json"
                    } else {
                        "history.md"
                    });

                let data_dir = dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("zenclaw");

                let db_path = data_dir.join("memory.db");
                if !db_path.exists() {
                    return Ok("No conversation history to export.".to_string());
                }

                let conn = rusqlite::Connection::open(&db_path)
                    .map_err(|e| zenclaw_core::error::ZenClawError::Memory(e.to_string()))?;

                let mut stmt = conn
                    .prepare(
                        "SELECT session_key, role, content, created_at
                         FROM history
                         ORDER BY session_key, created_at ASC",
                    )
                    .map_err(|e| zenclaw_core::error::ZenClawError::Memory(e.to_string()))?;

                let messages: Vec<(String, String, String, String)> = stmt
                    .query_map([], |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                        ))
                    })
                    .map_err(|e| zenclaw_core::error::ZenClawError::Memory(e.to_string()))?
                    .filter_map(|r| r.ok())
                    .collect();

                let content = match format {
                    "json" => {
                        let entries: Vec<Value> = messages
                            .iter()
                            .map(|(session, role, content, time)| {
                                json!({
                                    "session": session,
                                    "role": role,
                                    "content": content,
                                    "timestamp": time,
                                })
                            })
                            .collect();

                        serde_json::to_string_pretty(&entries).unwrap_or_default()
                    }
                    _ => {
                        let mut md = String::from("# ZenClaw Conversation History\n\n");
                        let mut current_session = String::new();

                        for (session, role, content, time) in &messages {
                            if session != &current_session {
                                md.push_str(&format!("\n## Session: {}\n\n", session));
                                current_session = session.clone();
                            }

                            let emoji = match role.as_str() {
                                "user" => "👤",
                                "assistant" => "🤖",
                                _ => "📝",
                            };

                            md.push_str(&format!(
                                "{} **{}** _{}_\n\n{}\n\n---\n\n",
                                emoji, role, time, content
                            ));
                        }

                        md
                    }
                };

                std::fs::write(output, &content)?;

                Ok(format!(
                    "✅ Exported {} messages to {} ({})",
                    messages.len(),
                    output,
                    format
                ))
            }
            _ => Ok(format!("Unknown action: {}. Use 'export' or 'sessions'.", action)),
        }
    }
}
