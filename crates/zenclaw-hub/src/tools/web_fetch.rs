//! Web fetch tool — HTTP requests for retrieving web content.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Fetch content from a URL.
pub struct WebFetchTool {
    client: Client,
    max_body_size: usize,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            max_body_size: 50_000,
        }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL via HTTP GET. Returns the response body as text."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "method": {
                    "type": "string",
                    "description": "HTTP method (GET, POST, PUT, DELETE). Default: GET",
                    "enum": ["GET", "POST", "PUT", "DELETE"]
                },
                "body": {
                    "type": "string",
                    "description": "Request body (for POST/PUT)"
                },
                "headers": {
                    "type": "object",
                    "description": "Custom headers as key-value pairs"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let url = args["url"].as_str().unwrap_or("");
        let method = args["method"].as_str().unwrap_or("GET").to_uppercase();

        let mut request = match method.as_str() {
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            _ => self.client.get(url),
        };

        // Add custom headers
        if let Some(headers) = args["headers"].as_object() {
            for (key, value) in headers {
                if let Some(v) = value.as_str() {
                    request = request.header(key.as_str(), v);
                }
            }
        }

        // Add body
        if let Some(body) = args["body"].as_str() {
            request = request.body(body.to_string());
        }

        match request.send().await {
            Ok(resp) => {
                let status = resp.status();
                let headers_str = resp
                    .headers()
                    .iter()
                    .take(10)
                    .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join("\n");

                let body = resp.text().await.unwrap_or_default();
                let truncated = if body.len() > self.max_body_size {
                    format!(
                        "{}...\n[truncated, {} total bytes]",
                        &body[..self.max_body_size],
                        body.len()
                    )
                } else {
                    body
                };

                Ok(format!(
                    "Status: {}\n\nHeaders:\n{}\n\nBody:\n{}",
                    status, headers_str, truncated
                ))
            }
            Err(e) => Ok(format!("Error fetching {}: {}", url, e)),
        }
    }
}
