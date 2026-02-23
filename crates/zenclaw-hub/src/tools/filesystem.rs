//! Filesystem tools — read, write, edit, list directories.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;

use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::tool::Tool;

/// Resolve path, optionally restricting to a workspace.
fn resolve_path(path: &str, workspace: Option<&Path>) -> std::result::Result<PathBuf, String> {
    let resolved = PathBuf::from(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path));

    if let Some(ws) = workspace {
        let ws_resolved = ws.canonicalize().unwrap_or_else(|_| ws.to_path_buf());
        if !resolved.starts_with(&ws_resolved) {
            return Err(format!(
                "Path {} is outside workspace {}",
                path,
                ws.display()
            ));
        }
    }
    Ok(resolved)
}

// ─── ReadFile ──────────────────────────────────────────────

pub struct ReadFileTool {
    pub workspace: Option<PathBuf>,
}

impl ReadFileTool {
    pub fn new() -> Self {
        Self { workspace: None }
    }
    pub fn with_workspace(mut self, ws: &Path) -> Self {
        self.workspace = Some(ws.to_path_buf());
        self
    }
}

impl Default for ReadFileTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str {
        "Read the contents of a file at the given path."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The file path to read" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"].as_str().unwrap_or("");
        let resolved = resolve_path(path, self.workspace.as_deref())
            .map_err(|e| ZenClawError::ToolExecution { tool: "read_file".into(), message: e })?;

        if !resolved.exists() {
            return Ok(format!("Error: File not found: {}", path));
        }
        match fs::read_to_string(&resolved).await {
            Ok(content) => Ok(content),
            Err(e) => Ok(format!("Error reading file: {}", e)),
        }
    }
}

// ─── WriteFile ─────────────────────────────────────────────

pub struct WriteFileTool {
    pub workspace: Option<PathBuf>,
}

impl WriteFileTool {
    pub fn new() -> Self { Self { workspace: None } }
    pub fn with_workspace(mut self, ws: &Path) -> Self {
        self.workspace = Some(ws.to_path_buf());
        self
    }
}

impl Default for WriteFileTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    fn description(&self) -> &str {
        "Write content to a file. Creates parent directories if needed."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The file path to write to" },
                "content": { "type": "string", "description": "The content to write" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");
        let resolved = resolve_path(path, self.workspace.as_deref())
            .map_err(|e| ZenClawError::ToolExecution { tool: "write_file".into(), message: e })?;

        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent).await.ok();
        }
        match fs::write(&resolved, content).await {
            Ok(()) => Ok(format!("✅ Wrote {} bytes to {}", content.len(), path)),
            Err(e) => Ok(format!("Error writing file: {}", e)),
        }
    }
}

// ─── EditFile ──────────────────────────────────────────────

pub struct EditFileTool {
    pub workspace: Option<PathBuf>,
}

impl EditFileTool {
    pub fn new() -> Self { Self { workspace: None } }
    pub fn with_workspace(mut self, ws: &Path) -> Self {
        self.workspace = Some(ws.to_path_buf());
        self
    }
}

impl Default for EditFileTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str { "edit_file" }
    fn description(&self) -> &str {
        "Edit a file by replacing old_text with new_text. The old_text must exist exactly."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The file path to edit" },
                "old_text": { "type": "string", "description": "Exact text to find and replace" },
                "new_text": { "type": "string", "description": "Text to replace with" }
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"].as_str().unwrap_or("");
        let old_text = args["old_text"].as_str().unwrap_or("");
        let new_text = args["new_text"].as_str().unwrap_or("");

        let resolved = resolve_path(path, self.workspace.as_deref())
            .map_err(|e| ZenClawError::ToolExecution { tool: "edit_file".into(), message: e })?;

        let content = match fs::read_to_string(&resolved).await {
            Ok(c) => c,
            Err(e) => return Ok(format!("Error reading file: {}", e)),
        };

        if !content.contains(old_text) {
            return Ok(format!(
                "Error: old_text not found in {}. Use read_file to get exact content.",
                path
            ));
        }

        let count = content.matches(old_text).count();
        if count > 1 {
            return Ok(format!(
                "Warning: old_text appears {} times. Provide more context to make unique.",
                count
            ));
        }

        let new_content = content.replacen(old_text, new_text, 1);
        match fs::write(&resolved, new_content).await {
            Ok(()) => Ok(format!("✅ Edited {}", path)),
            Err(e) => Ok(format!("Error writing file: {}", e)),
        }
    }
}

// ─── ListDir ───────────────────────────────────────────────

pub struct ListDirTool {
    pub workspace: Option<PathBuf>,
}

impl ListDirTool {
    pub fn new() -> Self { Self { workspace: None } }
    pub fn with_workspace(mut self, ws: &Path) -> Self {
        self.workspace = Some(ws.to_path_buf());
        self
    }
}

impl Default for ListDirTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str { "list_dir" }
    fn description(&self) -> &str {
        "List the contents of a directory."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The directory path to list" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"].as_str().unwrap_or(".");
        let resolved = resolve_path(path, self.workspace.as_deref())
            .map_err(|e| ZenClawError::ToolExecution { tool: "list_dir".into(), message: e })?;

        let mut entries = match fs::read_dir(&resolved).await {
            Ok(rd) => rd,
            Err(e) => return Ok(format!("Error reading directory: {}", e)),
        };

        let mut items = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let meta = entry.metadata().await.ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let prefix = if is_dir { "📁" } else { "📄" };
            let size_str = if is_dir {
                String::new()
            } else {
                format!(" ({})", human_size(size))
            };
            items.push(format!(
                "{} {}{}",
                prefix,
                entry.file_name().to_string_lossy(),
                size_str
            ));
        }

        items.sort();
        if items.is_empty() {
            Ok(format!("Directory {} is empty", path))
        } else {
            Ok(items.join("\n"))
        }
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    for unit in UNITS {
        if size < 1024.0 {
            return format!("{:.1}{}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1}TB", size)
}
