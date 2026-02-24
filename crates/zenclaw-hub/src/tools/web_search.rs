//! Web search tool — search the internet via DuckDuckGo HTML.
//!
//! Uses DDG's HTML-only endpoint — no API key, no JavaScript needed.
//! Falls back to DDG Lite if main endpoint is blocked.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

// Realistic browser User-Agent — bot UAs get blocked/captcha'd by DDG
const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";

/// Web search tool using DuckDuckGo HTML.
pub struct WebSearchTool {
    client: Client,
    max_results: usize,
}

impl WebSearchTool {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_default();

        Self {
            client,
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
        "Search the internet using DuckDuckGo. Returns titles, URLs, and snippets for the query."
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
                    "description": "Maximum number of results (default: 8, max: 15)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let query = args["query"].as_str().unwrap_or("").trim();
        let max_results = args["max_results"]
            .as_u64()
            .map(|n| n.min(15) as usize)
            .unwrap_or(self.max_results);

        if query.is_empty() {
            return Ok("Error: query is required".to_string());
        }

        tracing::info!("Searching: {}", query);

        // Try DDG HTML endpoint first
        if let Some(results) = self.try_ddg_html(query, max_results).await {
            if !results.is_empty() {
                return Ok(format_results(query, &results));
            }
        }

        // Fallback: DDG Lite (simpler HTML, harder to block)
        if let Some(results) = self.try_ddg_lite(query, max_results).await {
            if !results.is_empty() {
                return Ok(format_results(query, &results));
            }
        }

        // Both failed — return a helpful message instead of empty/confusing output
        Ok(format!(
            "⚠️ Search for \"{}\" returned no results.\n\
            DuckDuckGo may be rate-limiting this device. Try:\n\
            • Use the `web_fetch` tool with a direct URL\n\
            • Use the `web_scrape` tool on a known page\n\
            • Try rephrasing the query",
            query
        ))
    }
}

impl WebSearchTool {
    /// Try the main DuckDuckGo HTML endpoint.
    async fn try_ddg_html(&self, query: &str, max_results: usize) -> Option<Vec<SearchResult>> {
        let resp = self
            .client
            .post("https://html.duckduckgo.com/html/")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.5")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Referer", "https://duckduckgo.com/")
            .form(&[("q", query), ("b", ""), ("kl", "wt-wt")])
            .send()
            .await
            .ok()?;

        let status = resp.status().as_u16();
        let html = resp.text().await.unwrap_or_default();

        tracing::debug!(
            "DDG HTML response: {} status, {} bytes",
            status,
            html.len()
        );

        // If we got a suspiciously small response, it's likely a block/redirect
        if html.len() < 500 {
            tracing::warn!("DDG HTML returned tiny response ({} bytes) — likely blocked", html.len());
            return Some(vec![]);
        }

        Some(parse_ddg_html(&html, max_results))
    }

    /// Try the DDG Lite endpoint (simpler HTML, fallback).
    async fn try_ddg_lite(&self, query: &str, max_results: usize) -> Option<Vec<SearchResult>> {
        let encoded = percent_encode(query);
        let url = format!("https://lite.duckduckgo.com/lite/?q={}", encoded);

        let resp = self
            .client
            .get(&url)
            .header("Accept", "text/html,application/xhtml+xml")
            .header("Accept-Language", "en-US,en;q=0.5")
            .send()
            .await
            .ok()?;

        let html = resp.text().await.unwrap_or_default();

        tracing::debug!("DDG Lite response: {} bytes", html.len());

        if html.len() < 200 {
            return Some(vec![]);
        }

        Some(parse_ddg_lite(&html, max_results))
    }
}

/// Format results into text for the LLM.
fn format_results(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return format!("No results found for: \"{}\"", query);
    }

    let formatted: Vec<String> = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let snippet = if r.snippet.is_empty() {
                String::new()
            } else {
                format!("\n   {}", r.snippet)
            };
            format!("{}. **{}**\n   {}{}", i + 1, r.title, r.url, snippet)
        })
        .collect();

    format!(
        "Search results for: \"{}\"\n\n{}\n",
        query,
        formatted.join("\n\n")
    )
}

/// A single search result.
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Parse DuckDuckGo HTML search results.
fn parse_ddg_html(html: &str, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut pos = 0;

    while results.len() < max_results {
        // DDG main: <a class="result__a" href="...">title</a>
        let link_marker = "class=\"result__a\"";
        let link_start = match html[pos..].find(link_marker) {
            Some(p) => pos + p,
            None => break,
        };

        // Extract href — look back up to 300 chars for the href attribute
        let search_start = link_start.saturating_sub(300);
        let url = extract_href(&html[search_start..link_start + 50]).unwrap_or_default();

        // Extract title
        let title_start = match html[link_start..].find('>') {
            Some(p) => link_start + p + 1,
            None => { pos = link_start + 20; continue; }
        };
        let title_end = match html[title_start..].find("</a>") {
            Some(p) => title_start + p,
            None => { pos = title_start; continue; }
        };
        let title = strip_html_tags(&html[title_start..title_end]);

        // Find snippet after the title
        let snippet_marker = "class=\"result__snippet\"";
        let snippet = if let Some(sp) = html[title_end..].find(snippet_marker) {
            let sp_abs = title_end + sp;
            let cs = html[sp_abs..].find('>').map(|p| sp_abs + p + 1);
            let ce = cs.and_then(|c| {
                // Snippet ends at next opening tag or </
                html[c..].find("</").map(|p| c + p)
            });
            match (cs, ce) {
                (Some(c), Some(e)) if e > c => strip_html_tags(&html[c..e]),
                _ => String::new(),
            }
        } else {
            String::new()
        };

        let clean_url = clean_ddg_url(&url);

        if !title.is_empty() && !clean_url.is_empty() {
            results.push(SearchResult { title, url: clean_url, snippet });
        }

        pos = title_end + 5;
    }

    results
}

/// Parse DuckDuckGo Lite results (simpler table-based HTML).
fn parse_ddg_lite(html: &str, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut pos = 0;

    while results.len() < max_results {
        // DDG Lite uses <a class="result-link" href="...">title</a>
        let link_marker = "class=\"result-link\"";
        let link_start = match html[pos..].find(link_marker) {
            Some(p) => pos + p,
            None => break,
        };

        let search_start = link_start.saturating_sub(200);
        let url = match extract_href(&html[search_start..link_start + 50]) {
            Some(u) if !u.is_empty() => u,
            _ => { pos = link_start + 20; continue; }
        };

        let title_start = match html[link_start..].find('>') {
            Some(p) => link_start + p + 1,
            None => { pos = link_start + 20; continue; }
        };
        let title_end = match html[title_start..].find("</a>") {
            Some(p) => title_start + p,
            None => { pos = title_start; continue; }
        };
        let title = strip_html_tags(&html[title_start..title_end]);

        // Snippet in DDG Lite: <td class="result-snippet">...</td>
        let snippet_marker = "class=\"result-snippet\"";
        let snippet = if let Some(sp) = html[title_end..].find(snippet_marker) {
            let sp_abs = title_end + sp;
            let cs = html[sp_abs..].find('>').map(|p| sp_abs + p + 1);
            let ce = cs.and_then(|c| html[c..].find("</").map(|p| c + p));
            match (cs, ce) {
                (Some(c), Some(e)) if e > c => strip_html_tags(&html[c..e]),
                _ => String::new(),
            }
        } else {
            String::new()
        };

        if !title.is_empty() {
            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }

        pos = title_end + 5;
    }

    results
}

/// Extract href="..." from a chunk of HTML.
fn extract_href(html: &str) -> Option<String> {
    let href_start = html.find("href=\"")? + 6;
    let href_end = html[href_start..].find('"')? + href_start;
    Some(html[href_start..href_end].to_string())
}

/// Strip HTML tags and decode common entities.
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

    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Clean DuckDuckGo redirect URL to get the actual URL.
fn clean_ddg_url(url: &str) -> String {
    // DDG wraps URLs like: //duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&...
    if let Some(start) = url.find("uddg=") {
        let encoded = &url[start + 5..];
        let end = encoded.find('&').unwrap_or(encoded.len());
        return url_decode(&encoded[..end]);
    }

    if url.starts_with("http") {
        return url.to_string();
    }

    // Handle protocol-relative URLs
    if url.starts_with("//") {
        return format!("https:{}", url);
    }

    url.to_string()
}

/// Simple percent-encoding for query strings.
fn percent_encode(s: &str) -> String {
    let mut result = String::new();
    for ch in s.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(ch),
            ' ' => result.push('+'),
            _ => {
                for byte in ch.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    result
}

/// Simple percent-decoding for DDG redirect URLs.
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
