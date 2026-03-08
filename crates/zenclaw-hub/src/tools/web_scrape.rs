//! Web scrape tool — Extract clean Markdown from web pages.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

pub struct WebScrapeTool {
    client: Client,
    jina_api_key: Option<String>,
}


impl WebScrapeTool {
    pub fn new(jina_api_key: Option<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
                .tcp_keepalive(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            jina_api_key,
        }
    }
}

impl Default for WebScrapeTool {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl Tool for WebScrapeTool {
    fn name(&self) -> &str {
        "web_scrape"
    }

    fn description(&self) -> &str {
        "Extract clean Markdown from a website URL using Jina Reader (r.jina.ai). Best for reading articles, blogs, or documentation. \
        IMPORTANT RULES: 1) You MUST call `web_search` FIRST to obtain real URLs before calling this tool. 2) NEVER pass a URL from your memory or training data — only use URLs returned by `web_search`. 3) If this tool returns empty content or an error for a URL, do NOT retry the same URL — try a different one from your search results."
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

        let url_lower = url.to_lowercase();
        let decoded = url_lower.replace("%2f", "/").replace("%3a", ":").replace("%3f", "?").replace("%3d", "=");
        if decoded.contains("google.com/search") 
            || decoded.contains("bing.com/search") 
            || decoded.contains("duckduckgo.com") 
            || decoded.contains("yahoo.com") 
            || decoded.contains("google.com/?")
        {
            return Err(zenclaw_core::error::ZenClawError::ToolExecution {
                tool: "web_scrape".into(),
                message: "FATAL: You attempted to use `web_scrape` on a search engine URL. This is STRICTLY FORBIDDEN. You MUST completely abort this tool call and use the `web_search` native tool instead. DO NOT RETRY web_scrape. USE web_search!".to_string(),
            });
        }

        // Respect LLM-requested limit; default 8K chars ≈ 2K tokens — enough for
        // most factual queries. LLM can request more if explicitly needed.
        let max_chars = args["max_chars"]
            .as_u64()
            .map(|n| n.clamp(500, 50_000) as usize)
            .unwrap_or(8_000);

        // 1. Try Jina Reader API to get clean Markdown
        let target_url = format!("https://r.jina.ai/{}", url);
        
        let mut request = self.client.get(&target_url)
            .header("X-Return-Format", "markdown");

        if let Some(ref key) = self.jina_api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        match request.send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    if status.as_u16() == 401 || status.as_u16() == 403 {
                        let has_key = self.jina_api_key.is_some();
                        tracing::error!(
                            "Jina Reader AUTH FAILED (status {}). Key configured: {}. The API key may be invalid or expired.",
                            status, has_key
                        );
                    } else {
                        tracing::warn!("Jina Reader API failed with status {}. Falling back to local Headless Browser...", status);
                    }
                } else {
                    let body = resp.text().await.unwrap_or_default();
                    let body_lower = body.to_lowercase();

                    // Detect empty content
                    if body.trim().is_empty() {
                        return Ok(format!(
                            "Error: Successfully connected to {}, but the page content is EMPTY. DO NOT RETRY this URL. Use a different URL from your `web_search` results instead.",
                            url
                        ));
                    }

                    // Detect blocked/captcha/404 pages
                    let is_blocked = body_lower.contains("attention required")
                        || body_lower.contains("please enable cookies")
                        || body_lower.contains("you have been blocked")
                        || body_lower.contains("access denied")
                        || body_lower.contains("captcha")
                        || body_lower.contains("halaman tidak ditemukan")
                        || body_lower.contains("page not found")
                        || body_lower.contains("404 not found")
                        || body_lower.contains("error 403")
                        || body_lower.contains("error 404");
                    if is_blocked {
                        return Ok(format!(
                            "Error: The page at {} is BLOCKED (Cloudflare/CAPTCHA/404). Content cannot be extracted. DO NOT RETRY this URL. Use a different URL from your `web_search` results instead.",
                            url
                        ));
                    }

                    // Detect pages with too little meaningful content (navbars, footers only)
                    let meaningful_len = body.trim().len();
                    if meaningful_len < 200 {
                        return Ok(format!(
                            "Error: The page at {} returned only {} characters of content, which is too short to be a real article. DO NOT RETRY this URL. Use a different URL from your `web_search` results instead.",
                            url, meaningful_len
                        ));
                    }

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
            Err(e) => {
                let jina_hint = if self.jina_api_key.is_none() {
                    " ⚠️ JINA_API_KEY is NOT set — Jina Reader requires authentication. Ask user to configure it via Settings → Set JINA_API_KEY."
                } else {
                    ""
                };
                Ok(format!("Error: Both Jina API and Local Headless Browser failed to extract {}: {}{}", url, e, jina_hint))
            }
        }
    }
}
