//! Process tool — manage long-running background processes.
//!
//! Spawns, lists, checks status, and kills background processes.
//! Captures stdout and stderr in real-time.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

#[derive(Debug)]
struct ManagedProcess {
    id: String,
    command: String,
    status: ProcessStatus,
    output: Arc<Mutex<String>>,
    // We cannot easily hold Child if we want to kill it later from a different reference 
    // without complex Mutex tracking, so we store a kill sender.
    kill_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

#[derive(Debug, Clone, PartialEq)]
enum ProcessStatus {
    Running,
    Finished(i32),
    Killed,
    Failed(String),
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "🔄 running"),
            Self::Finished(code) => write!(f, "✅ done (exit: {})", code),
            Self::Killed => write!(f, "🛑 killed"),
            Self::Failed(err) => write!(f, "❌ failed: {}", err),
        }
    }
}

/// A tool to manage background processes.
pub struct ProcessTool {
    processes: Arc<Mutex<HashMap<String, ManagedProcess>>>,
    max_output_size: usize,
}

impl ProcessTool {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            max_output_size: 50_000,
        }
    }
}

impl Default for ProcessTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ProcessTool {
    fn name(&self) -> &str {
        "process"
    }

    fn description(&self) -> &str {
        "Long-Running Process Manager. Spawns and manages background tasks asynchonously.
Actions:
- 'spawn' (starts a command in the background, returns process ID)
- 'status' (checks process status and returns the latest output logs. requires 'process_id')
- 'kill' (terminates a running process. requires 'process_id')
- 'list' (lists all managed background processes)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["spawn", "status", "kill", "list"],
                    "description": "Action type"
                },
                "command": {
                    "type": "string",
                    "description": "Shell command to spawn (only for 'spawn' action)."
                },
                "process_id": {
                    "type": "string",
                    "description": "The ID of the process (required for 'status' and 'kill')."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let action = args["action"].as_str().unwrap_or("list");

        match action {
            "spawn" => {
                let command_str = args["command"].as_str().unwrap_or("").to_string();
                if command_str.is_empty() {
                    return Ok("Error: 'command' required for spawning.".into());
                }

                let id = Uuid::new_v4().to_string().chars().take(8).collect::<String>();
                let output = Arc::new(Mutex::new(String::new()));
                let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();

                let process = ManagedProcess {
                    id: id.clone(),
                    command: command_str.clone(),
                    status: ProcessStatus::Running,
                    output: output.clone(),
                    kill_tx: Some(kill_tx),
                };

                let processes = self.processes.clone();
                let output_clone = output.clone();
                let max_size = self.max_output_size;

                let id_clone = id.clone();

                let command_clone = command_str.clone();

                tokio::spawn(async move {
                    #[cfg(target_os = "windows")]
                    let (shell, arg) = ("cmd", "/C");
                    #[cfg(not(target_os = "windows"))]
                    let (shell, arg) = ("sh", "-c");

                    let mut cmd = Command::new(shell);
                    cmd.arg(arg)
                       .arg(&command_clone)
                       .stdout(Stdio::piped())
                       .stderr(Stdio::piped());

                    let mut child_process = match cmd.spawn() {
                        Ok(c) => c,
                        Err(e) => {
                            let mut map = processes.lock().await;
                            if let Some(p) = map.get_mut(&id_clone) {
                                p.status = ProcessStatus::Failed(e.to_string());
                            }
                            return;
                        }
                    };

                    let stdout = child_process.stdout.take().unwrap();
                    let stderr = child_process.stderr.take().unwrap();

                    let out_ref1 = output_clone.clone();
                    let out_ref2 = output_clone.clone();

                    // Readers for stdout and stderr appending to the string
                    let read_stdout = async move {
                        let mut reader = BufReader::new(stdout);
                        let mut buf = [0; 1024];
                        while let Ok(n) = reader.read(&mut buf).await {
                            if n == 0 { break; }
                            let mut locked = out_ref1.lock().await;
                            locked.push_str(&String::from_utf8_lossy(&buf[..n]));
                            if locked.len() > max_size {
                                *locked = format!("...[truncated] {}", &locked[locked.len() - max_size..]);
                            }
                        }
                    };

                    let read_stderr = async move {
                        let mut reader = BufReader::new(stderr);
                        let mut buf = [0; 1024];
                        while let Ok(n) = reader.read(&mut buf).await {
                            if n == 0 { break; }
                            let mut locked = out_ref2.lock().await;
                            locked.push_str(&String::from_utf8_lossy(&buf[..n]));
                            if locked.len() > max_size {
                                *locked = format!("...[truncated] {}", &locked[locked.len() - max_size..]);
                            }
                        }
                    };

                    tokio::select! {
                        _ = kill_rx => {
                            let _ = child_process.kill().await;
                            let mut map = processes.lock().await;
                            if let Some(p) = map.get_mut(&id_clone) {
                                p.status = ProcessStatus::Killed;
                            }
                        }
                        status = child_process.wait() => {
                            // Finish reading the streams
                            tokio::join!(read_stdout, read_stderr);
                            
                            let mut map = processes.lock().await;
                            if let Some(p) = map.get_mut(&id_clone) {
                                let status_result: std::io::Result<std::process::ExitStatus> = status;
                                match status_result {
                                    Ok(exit_status) => p.status = ProcessStatus::Finished(exit_status.code().unwrap_or(0)),
                                    Err(e) => p.status = ProcessStatus::Failed(e.to_string()),
                                }
                            }
                        }
                    }
                });

                self.processes.lock().await.insert(id.clone(), process);

                Ok(format!("✅ Background Process Spawned.\n  ID: {}\n  Target: {}\nTip: use action='status' to check output.", id, command_str))
            }
            "status" => {
                let pid = args["process_id"].as_str().unwrap_or("");
                if pid.is_empty() {
                    return Ok("Error: 'process_id' is required.".into());
                }

                let map = self.processes.lock().await;
                if let Some(process) = map.get(pid) {
                    let out = process.output.lock().await;
                    Ok(format!("Process: {}\nCommand: {}\nStatus: {}\n--- Output Log ---\n{}", 
                               pid, process.command, process.status, *out))
                } else {
                    Ok(format!("Error: Process {} not found.", pid))
                }
            }
            "kill" => {
                let pid = args["process_id"].as_str().unwrap_or("");
                if pid.is_empty() {
                    return Ok("Error: 'process_id' is required.".into());
                }

                let mut map = self.processes.lock().await;
                if let Some(process) = map.get_mut(pid) {
                    if let Some(tx) = process.kill_tx.take() {
                        let _ = tx.send(()); // Triggers the await select loop
                        process.status = ProcessStatus::Killed;
                        Ok(format!("✅ Sent kill signal to Process {}.", pid))
                    } else if process.status == ProcessStatus::Running {
                        Ok("Error: Kill signal already sent.".into())
                    } else {
                        Ok(format!("Process {} is already {}.", pid, process.status))
                    }
                } else {
                    Ok(format!("Error: Process {} not found.", pid))
                }
            }
            "list" => {
                let map = self.processes.lock().await;
                if map.is_empty() {
                    return Ok("No background processes found.".into());
                }

                let list = map.values()
                    .map(|p| format!("• {} | {} | cmd: '{}'", p.id, p.status, p.command))
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(format!("Active/Terminated Background Processes:\n{}", list))
            }
            _ => Ok("Unknown action.".into())
        }
    }
}
