//! Auto-update checker — check for new ZenClaw releases on GitHub.

use serde::Deserialize;
use tracing::info;

use zenclaw_core::error::{Result, ZenClawError};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_API: &str = "https://api.github.com/repos/volumeee/zenclaw/releases/latest";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
}

/// Check for updates on GitHub.
pub async fn check_for_updates() -> Result<Option<UpdateInfo>> {
    let client = reqwest::Client::builder()
        .user_agent("zenclaw-update-checker")
        .build()
        .map_err(|e| ZenClawError::Other(format!("HTTP client error: {}", e)))?;

    let resp = client
        .get(GITHUB_API)
        .send()
        .await
        .map_err(|e| ZenClawError::Other(format!("Update check failed: {}", e)))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let release: GitHubRelease = resp
        .json()
        .await
        .map_err(|e| ZenClawError::Other(format!("Parse error: {}", e)))?;

    let latest = release.tag_name.trim_start_matches('v');
    let current = CURRENT_VERSION;

    if version_greater(latest, current) {
        info!("🆕 New version available: v{} (current: v{})", latest, current);
        Ok(Some(UpdateInfo {
            current: current.to_string(),
            latest: latest.to_string(),
            url: release.html_url,
            changelog: release.body.unwrap_or_default(),
        }))
    } else {
        Ok(None)
    }
}

/// Update info.
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub url: String,
    pub changelog: String,
}

/// Simple semantic version comparison.
fn version_greater(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    let va = parse(a);
    let vb = parse(b);

    for i in 0..va.len().max(vb.len()) {
        let x = va.get(i).copied().unwrap_or(0);
        let y = vb.get(i).copied().unwrap_or(0);
        if x > y {
            return true;
        }
        if x < y {
            return false;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(version_greater("0.2.0", "0.1.0"));
        assert!(version_greater("1.0.0", "0.9.9"));
        assert!(version_greater("0.1.1", "0.1.0"));
        assert!(!version_greater("0.1.0", "0.1.0"));
        assert!(!version_greater("0.0.9", "0.1.0"));
    }
}
