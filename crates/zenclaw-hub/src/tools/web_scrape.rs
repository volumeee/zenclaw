//! Web scrape tool — Extract clean Markdown from web pages.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

pub struct WebScrapeTool {
    client: Client,
}


impl WebScrapeTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl Default for WebScrapeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebScrapeTool {
    fn name(&self) -> &str {
        "web_scrape"
    }

    fn description(&self) -> &str {
        "Extract clean Markdown from any website URL. Best for reading articles, blogs, or documentation. Removes ads, navbars, and HTML bloat."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to extract content from"
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Maximum characters to return (default: 8000 ≈ 2K tokens). Use higher values only when you need full document (max: 50000)."
                }
            },
            "required": ["url"]
        })
    }


    async fn execute(&self, args: Value) -> Result<String> {
        let url = args["url"].as_str().unwrap_or("");
        if url.is_empty() {
            return Ok("Error: No URL provided".into());
        }

        // Respect LLM-requested limit; default 8K chars ≈ 2K tokens — enough for
        // most factual queries. LLM can request more if explicitly needed.
        let max_chars = args["max_chars"]
            .as_u64()
            .map(|n| n.clamp(500, 50_000) as usize)
            .unwrap_or(8_000);

        // 1. Try Jina Reader API to get clean Markdown
        let target_url = format!("https://r.jina.ai/{}", url);
        
        let request = self.client.get(&target_url)
            .header("X-Return-Format", "markdown");

        match request.send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    tracing::warn!("Jina Reader API failed with status {}. Falling back to local Headless Browser...", status);
                } else {
                    let body = resp.text().await.unwrap_or_default();
                    let truncated = if body.len() > max_chars {
                        format!(
                            "{}...\n\n[Truncated at {} chars. Full: {} chars. Use max_chars={} for more.]",
                            &body[..max_chars], max_chars, body.len(),
                            body.len().min(50_000)
                        )
                    } else {
                        body
                    };

                    return Ok(format!(
                        "--- EXTRACTED MARKDOWN FROM {} ---\n\n{}",
                        url, truncated
                    ));
                }
            }
            Err(e) => {
                tracing::warn!("Jina extract failed for {}: {}. Falling back to Headless Browser...", url, e);
            }
        }

        // Fallback: Local Headless SPA Browser (Puppeteer)
        tracing::info!("🕸️ Initiating Local Headless Scrape for {}...", url);
        match tokio::process::Command::new("node")
            .arg("bridge/scrape.js")
            .arg(url)
            .output()
            .await
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let body = stdout.to_string();
                if output.status.success() && !body.is_empty() {
                    let truncated = if body.len() > max_chars {
                        format!(
                            "{}...\n\n[Truncated at {} chars. Full: {} chars.]",
                            &body[..max_chars], max_chars, body.len()
                        )
                    } else {
                        body
                    };


                    Ok(format!(
                        "--- EXTRACTED TEXT VIA HEADLESS BROWSER FROM {} ---\n\n{}",
                        url, truncated
                    ))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Ok(format!("Error: Headless browser failed. Status: {}. Error: {}", output.status, stderr))
                }
            }
            Err(e) => Ok(format!("Error: Both Jina API and Local Headless Browser failed to extract {}: {}", url, e)),
        }
    }
}
