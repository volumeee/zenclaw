//! Health monitoring tool — system health for edge devices.
//!
//! Reports CPU, memory, disk, and network status.
//! Essential for agents running on STB/RPi to self-diagnose issues.

use async_trait::async_trait;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Health monitor tool — system diagnostics.
pub struct HealthTool;

impl HealthTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HealthTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HealthTool {
    fn name(&self) -> &str {
        "health"
    }

    fn description(&self) -> &str {
        "Check system health: CPU load, memory usage, disk space, uptime, and network. Essential for edge device monitoring."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "check": {
                    "type": "string",
                    "enum": ["all", "cpu", "memory", "disk", "network", "uptime"],
                    "description": "What to check. Default: 'all'"
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let check = args["check"].as_str().unwrap_or("all");

        let mut report = Vec::new();

        if check == "all" || check == "cpu" {
            report.push(get_cpu_info().await);
        }
        if check == "all" || check == "memory" {
            report.push(get_memory_info().await);
        }
        if check == "all" || check == "disk" {
            report.push(get_disk_info().await);
        }
        if check == "all" || check == "uptime" {
            report.push(get_uptime().await);
        }
        if check == "all" || check == "network" {
            report.push(get_network_info().await);
        }

        Ok(report.join("\n"))
    }
}

async fn get_cpu_info() -> String {
    let load = run_cmd("cat /proc/loadavg 2>/dev/null || echo 'N/A'").await;
    let cores = run_cmd("nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo '?'").await;
    let temp = run_cmd(
        "cat /sys/class/thermal/thermal_zone0/temp 2>/dev/null | awk '{printf \"%.1f°C\", $1/1000}' || echo 'N/A'"
    ).await;

    format!(
        "## CPU\n  Load: {}\n  Cores: {}\n  Temperature: {}",
        load.trim(),
        cores.trim(),
        temp.trim()
    )
}

async fn get_memory_info() -> String {
    let info = run_cmd(
        "free -h 2>/dev/null | awk 'NR==2{printf \"Total: %s, Used: %s, Free: %s, Usage: %.1f%%\", $2, $3, $4, $3/$2*100}' || vm_stat 2>/dev/null | head -5 || echo 'N/A'",
    )
    .await;

    format!("## Memory\n  {}", info.trim())
}

async fn get_disk_info() -> String {
    let info = run_cmd(
        "df -h / 2>/dev/null | awk 'NR==2{printf \"Total: %s, Used: %s, Free: %s, Usage: %s\", $2, $3, $4, $5}'"
    ).await;

    format!("## Disk\n  {}", info.trim())
}

async fn get_uptime() -> String {
    let uptime = run_cmd("uptime -p 2>/dev/null || uptime | awk -F'up ' '{print $2}' | awk -F',' '{print $1}' || echo 'N/A'").await;

    format!("## Uptime\n  {}", uptime.trim())
}

async fn get_network_info() -> String {
    let ip = run_cmd(
        "hostname -I 2>/dev/null | awk '{print $1}' || ifconfig 2>/dev/null | grep 'inet ' | grep -v 127.0.0.1 | awk '{print $2}' | head -1 || echo 'N/A'"
    ).await;

    let dns = run_cmd("ping -c 1 -W 2 8.8.8.8 >/dev/null 2>&1 && echo 'Online' || echo 'Offline'").await;

    format!(
        "## Network\n  IP: {}\n  Internet: {}",
        ip.trim(),
        dns.trim()
    )
}

async fn run_cmd(cmd: &str) -> String {
    match tokio::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .await
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => "N/A".to_string(),
    }
}
