//! System info tool — report system status.

use async_trait::async_trait;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Report system information and health.
pub struct SystemInfoTool;

impl SystemInfoTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemInfoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SystemInfoTool {
    fn name(&self) -> &str {
        "system_info"
    }

    fn description(&self) -> &str {
        "Get system information: OS, arch, hostname, uptime, memory usage."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: Value) -> Result<String> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Ok(format!(
            "System Information:\n  OS: {}\n  Arch: {}\n  Hostname: {}\n  Working Dir: {}\n  ZenClaw: v{}",
            os,
            arch,
            hostname,
            cwd,
            env!("CARGO_PKG_VERSION"),
        ))
    }
}
