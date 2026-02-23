//! Web search tool — search the internet via DuckDuckGo HTML.
//!
//! Uses DDG's HTML-only endpoint — no API key, no JavaScript needed.
//! Perfect for embedded/edge devices.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Web search tool using DuckDuckGo HTML.
pub struct WebSearchTool {
    client: Client,
    max_results: usize,
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            max_results: 8,
        }
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the internet using DuckDuckGo. Returns titles, URLs, and snippets."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 8)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let query = args["query"].as_str().unwrap_or("");
        let max_results = args["max_results"]
            .as_u64()
            .unwrap_or(self.max_results as u64) as usize;

        if query.is_empty() {
            return Ok("Error: query is required".to_string());
        }

        tracing::info!("Searching: {}", query);

        // Use DuckDuckGo HTML search
        let url = "https://html.duckduckgo.com/html/";
        let resp = self
            .client
            .post(url)
            .form(&[("q", query)])
            .header("User-Agent", "ZenClaw/0.1 (AI Agent)")
            .send()
            .await;

        match resp {
            Ok(response) => {
                let html = response.text().await.unwrap_or_default();
                let results = parse_ddg_html(&html, max_results);

                if results.is_empty() {
                    return Ok(format!("No results found for: {}", query));
                }

                let formatted = results
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        format!(
                            "{}. **{}**\n   {}\n   {}\n",
                            i + 1,
                            r.title,
                            r.url,
                            r.snippet
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(format!(
                    "Search results for: \"{}\"\n\n{}",
                    query, formatted
                ))
            }
            Err(e) => Ok(format!("Search error: {}", e)),
        }
    }
}

/// A single search result.
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Parse DuckDuckGo HTML search results.
/// Simple HTML parsing without heavy dependencies.
fn parse_ddg_html(html: &str, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // DDG results are in <a class="result__a" href="...">title</a>
    // and <a class="result__snippet" ...>snippet</a>
    let mut pos = 0;

    while results.len() < max_results {
        // Find next result link
        let link_marker = "class=\"result__a\"";
        let link_start = match html[pos..].find(link_marker) {
            Some(p) => pos + p,
            None => break,
        };

        // Extract href
        let url = extract_href(&html[link_start.saturating_sub(200)..link_start + 50])
            .unwrap_or_default();

        // Extract title (content between > and </a>)
        let title_start = match html[link_start..].find('>') {
            Some(p) => link_start + p + 1,
            None => { pos = link_start + 20; continue; }
        };
        let title_end = match html[title_start..].find("</a>") {
            Some(p) => title_start + p,
            None => { pos = title_start; continue; }
        };
        let title = strip_html_tags(&html[title_start..title_end]);

        // Find snippet
        let snippet_marker = "class=\"result__snippet\"";
        let snippet = if let Some(sp) = html[title_end..].find(snippet_marker) {
            let sp_start = title_end + sp;
            let content_start = html[sp_start..].find('>').map(|p| sp_start + p + 1);
            let content_end = content_start.and_then(|cs| html[cs..].find("</").map(|p| cs + p));

            match (content_start, content_end) {
                (Some(cs), Some(ce)) => strip_html_tags(&html[cs..ce]),
                _ => String::new(),
            }
        } else {
            String::new()
        };

        // Clean URL — DuckDuckGo wraps URLs in redirects
        let clean_url = clean_ddg_url(&url);

        if !title.is_empty() && !clean_url.is_empty() {
            results.push(SearchResult {
                title,
                url: clean_url,
                snippet,
            });
        }

        pos = title_end + 10;
    }

    results
}

/// Extract href="..." from a chunk of HTML.
fn extract_href(html: &str) -> Option<String> {
    let href_start = html.find("href=\"")? + 6;
    let href_end = html[href_start..].find('"')? + href_start;
    Some(html[href_start..href_end].to_string())
}

/// Strip HTML tags from text.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    // Decode common HTML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

/// Clean DuckDuckGo redirect URL to get the actual URL.
fn clean_ddg_url(url: &str) -> String {
    // DDG wraps URLs like: //duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com...
    if url.contains("uddg=") {
        if let Some(start) = url.find("uddg=") {
            let encoded = &url[start + 5..];
            let end = encoded.find('&').unwrap_or(encoded.len());
            let encoded_url = &encoded[..end];
            // URL decode
            return url_decode(encoded_url);
        }
    }

    // Also handle direct URLs
    if url.starts_with("http") {
        return url.to_string();
    }

    url.to_string()
}

/// Simple percent-decoding.
fn url_decode(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if ch == '+' {
            result.push(' ');
        } else {
            result.push(ch);
        }
    }

    result
}
