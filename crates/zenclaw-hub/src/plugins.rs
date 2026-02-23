//! Plugin system — dynamic tool extension.
//!
//! Plugins are external tools defined as JSON manifests + shell scripts.
//! This allows extending ZenClaw without recompiling.
//!
//! Plugin structure:
//! ```text
//! plugins/
//! └── my_plugin/
//!     ├── plugin.json     # Manifest
//!     └── run.sh          # Executable script
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs;
use tracing::{info, warn};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Plugin manifest (plugin.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name (used as tool name).
    pub name: String,
    /// Description shown to LLM.
    pub description: String,
    /// JSON Schema for parameters.
    pub parameters: Value,
    /// Command to execute (relative to plugin dir).
    #[serde(default = "default_command")]
    pub command: String,
    /// Author info.
    pub author: Option<String>,
    /// Version.
    pub version: Option<String>,
}

fn default_command() -> String {
    "run.sh".to_string()
}

/// A plugin-based tool — runs external scripts.
pub struct PluginTool {
    manifest: PluginManifest,
    plugin_dir: PathBuf,
}

impl PluginTool {
    /// Load a plugin from its directory.
    pub async fn load(dir: &Path) -> Result<Self> {
        let manifest_path = dir.join("plugin.json");
        let content = fs::read_to_string(&manifest_path).await?;
        let manifest: PluginManifest = serde_json::from_str(&content)?;

        info!("Loaded plugin: {} v{}", manifest.name, manifest.version.as_deref().unwrap_or("0.0"));

        Ok(Self {
            manifest,
            plugin_dir: dir.to_path_buf(),
        })
    }
}

#[async_trait]
impl Tool for PluginTool {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn description(&self) -> &str {
        &self.manifest.description
    }

    fn parameters(&self) -> Value {
        self.manifest.parameters.clone()
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let script = self.plugin_dir.join(&self.manifest.command);

        if !script.exists() {
            return Ok(format!("Plugin error: script not found: {}", script.display()));
        }

        // Pass args as JSON env var
        let args_json = serde_json::to_string(&args).unwrap_or_default();

        let output = tokio::task::spawn_blocking(move || {
            Command::new("sh")
                .arg("-c")
                .arg(script.to_string_lossy().to_string())
                .env("ZENCLAW_ARGS", &args_json)
                .env("ZENCLAW_PLUGIN", "1")
                .output()
        })
        .await
        .map_err(|e| zenclaw_core::error::ZenClawError::ToolExecution { tool: "plugin".into(), message: format!("spawn error: {}", e) })?
        .map_err(|e| zenclaw_core::error::ZenClawError::ToolExecution { tool: "plugin".into(), message: format!("exec error: {}", e) })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(stdout)
        } else {
            Ok(format!("Plugin error (exit {}):\n{}\n{}", output.status.code().unwrap_or(-1), stdout, stderr))
        }
    }
}

/// Plugin manager — discovers and loads plugins.
pub struct PluginManager {
    plugins_dir: PathBuf,
}

impl PluginManager {
    pub fn new(plugins_dir: &Path) -> Self {
        Self {
            plugins_dir: plugins_dir.to_path_buf(),
        }
    }

    /// Discover and load all plugins.
    pub async fn load_all(&self) -> Vec<PluginTool> {
        let mut plugins = Vec::new();

        if !self.plugins_dir.exists() {
            // Create plugins dir + example
            fs::create_dir_all(&self.plugins_dir).await.ok();
            self.create_example_plugin().await;
        }

        let mut entries = match fs::read_dir(&self.plugins_dir).await {
            Ok(rd) => rd,
            Err(_) => return plugins,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() && path.join("plugin.json").exists() {
                match PluginTool::load(&path).await {
                    Ok(plugin) => plugins.push(plugin),
                    Err(e) => warn!("Failed to load plugin {:?}: {}", path, e),
                }
            }
        }

        plugins
    }

    /// Create an example plugin for reference.
    async fn create_example_plugin(&self) {
        let example_dir = self.plugins_dir.join("example_hello");
        fs::create_dir_all(&example_dir).await.ok();

        let manifest = serde_json::json!({
            "name": "hello_plugin",
            "description": "An example plugin that greets users. Shows how to create ZenClaw plugins.",
            "version": "1.0.0",
            "author": "ZenClaw",
            "command": "run.sh",
            "parameters": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name to greet"
                    }
                },
                "required": ["name"]
            }
        });

        let script = r#"#!/bin/sh
# Example ZenClaw plugin
# Arguments are passed via ZENCLAW_ARGS env var as JSON

# Parse the "name" from JSON args (using simple grep/sed)
NAME=$(echo "$ZENCLAW_ARGS" | grep -o '"name":"[^"]*"' | sed 's/"name":"//;s/"//')

if [ -z "$NAME" ]; then
    NAME="World"
fi

echo "Hello, $NAME! 👋"
echo "This is an example ZenClaw plugin."
echo "Plugin dir: $(dirname $0)"
echo "Args: $ZENCLAW_ARGS"
"#;

        fs::write(example_dir.join("plugin.json"), serde_json::to_string_pretty(&manifest).unwrap_or_default()).await.ok();
        fs::write(example_dir.join("run.sh"), script).await.ok();

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(example_dir.join("run.sh")) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(example_dir.join("run.sh"), perms).ok();
            }
        }
    }

    pub fn dir(&self) -> &Path {
        &self.plugins_dir
    }
}
