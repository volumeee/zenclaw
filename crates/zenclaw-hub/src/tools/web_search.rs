//! Web search tool — multi-engine, robust, zero API key required.
//!
//! Search strategy (tried in this order until results are found):
//!
//! 1. DDG Instant Answer API  — JSON, no HTML parsing, fastest
//! 2. DDG HTML endpoint        — Full web results via POST
//! 3. DDG Lite endpoint         — Simpler HTML, harder to block
//! 4. Wikipedia Search API      — Best for factual/encyclopedic queries
//!
//! Results from all successful engines are merged + deduplicated.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

// Rotate between realistic browser User-Agents to reduce block chance
const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux aarch64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.3 Safari/605.1.15",
];

// Simple rotate by process uptime seconds (no rand dependency needed)
fn pick_user_agent() -> &'static str {
    let idx = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as usize)
        .unwrap_or(0)
        % USER_AGENTS.len();
    USER_AGENTS[idx]
}

// ─────────────────────────────────────────────────────────────────────────────

/// Web search tool — multi-engine, no API key, works on edge devices.
pub struct WebSearchTool {
    client: Client,
    max_results: usize,
}

impl WebSearchTool {
    pub fn new() -> Self {
        let ua = pick_user_agent();
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent(ua)
            .tcp_keepalive(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self { client, max_results: 8 }
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
        "Search the internet. Returns titles, URLs, and snippets.\n\
        Provides direct answers for factual questions (who, what, when, where).\n\
        Uses multiple search engines internally for reliability."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query. Be specific for better results."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results to return (1-15, default: 8)"
                },
                "lang": {
                    "type": "string",
                    "description": "Language hint for results: 'id' (Indonesia), 'en' (English), 'auto' (default)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let query = args["query"].as_str().unwrap_or("").trim();
        let max_results = args["max_results"]
            .as_u64()
            .map(|n| n.clamp(1, 15) as usize)
            .unwrap_or(self.max_results);
        let lang = args["lang"].as_str().unwrap_or("auto");

        if query.is_empty() {
            return Ok("Error: query is required".to_string());
        }

        tracing::info!("web_search: \"{}\" (max={}, lang={})", query, max_results, lang);

        let mut output = String::new();
        let mut all_results: Vec<SearchResult> = Vec::new();
        let mut seen_urls: HashSet<String> = HashSet::new();

        // ── Run all engines in parallel ───────────────────────────────────
        let (instant_opt, ddg_results, lite_results, wiki_results) = tokio::join!(
            self.ddg_instant(query),
            self.ddg_html(query, max_results, lang),
            self.ddg_lite(query, max_results, lang),
            self.wikipedia_search(query, 4, lang),
        );

        // ── Strategy 1: DDG Instant Answer (direct factual answer) ────────
        if let Some(instant) = instant_opt {
            output.push_str(&instant);
            output.push_str("\n\n");
        }

        // ── Strategy 2: DDG HTML results ──────────────────────────────────
        for r in ddg_results {
            if seen_urls.insert(r.url.clone()) {
                all_results.push(r);
            }
        }

        // ── Strategy 3: DDG Lite (always merge, dedup handles overlap) ───
        for r in lite_results {
            if seen_urls.insert(r.url.clone()) {
                all_results.push(r);
            }
        }

        // ── Strategy 4: Wikipedia (always — best for factual queries) ─────
        let mut wiki_top_url: Option<String> = None;
        for r in wiki_results {
            if wiki_top_url.is_none() {
                wiki_top_url = Some(r.url.clone());
            }
            if seen_urls.insert(r.url.clone()) {
                all_results.push(r);
            }
        }

        // ── Strategy 5: Jina Reader — fetch full Wikipedia article text ───
        // Only when no instant answer was found (common on restricted networks).
        // Fetches the top Wikipedia article for richer factual context.
        if output.trim().is_empty() && let Some(wiki_url) = wiki_top_url {
            tracing::info!("web_search: fetching full article via Jina Reader: {}", wiki_url);
            let jina_url = format!("https://r.jina.ai/{}", wiki_url);
            if let Ok(resp) = self.client.get(&jina_url)
                .header("Accept", "text/plain")
                .send().await
                && let Ok(text) = resp.text().await
            {
                let trimmed = if text.len() > 1500 {
                    format!("{}...\n*(truncated — use web_fetch for full article)*", &text[..1500])
                } else {
                    text
                };
                if trimmed.len() > 100 {
                    output.push_str("## Wikipedia Article (via Jina Reader)\n\n");
                    output.push_str(&trimmed);
                    output.push_str("\n\n");
                }
            }
        }



        // ── Compile final output ──────────────────────────────────────────
        let take = all_results.len().min(max_results);
        if take > 0 {
            output.push_str(&format!("## Web Results for: \"{}\"\n\n", query));
            for (i, r) in all_results[..take].iter().enumerate() {
                output.push_str(&format!(
                    "{}. **{}**\n   🔗 {}\n",
                    i + 1,
                    r.title,
                    r.url
                ));
                if !r.snippet.is_empty() {
                    output.push_str(&format!("   {}\n", r.snippet));
                }
                output.push('\n');
            }
        }

        if output.trim().is_empty() {
            return Ok(format!(
                "⚠️ No results found for: \"{}\"\n\n\
                Try:\n\
                • Rephrase the query (simpler terms)\n\
                • Use `web_fetch` with a direct URL (e.g. wikipedia page)\n\
                • Use `web_scrape` to extract content from a known site",
                query
            ));
        }

        tracing::debug!(
            "web_search: {} total results ({} bytes)",
            take,
            output.len()
        );

        Ok(output)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Engine implementations
// ─────────────────────────────────────────────────────────────────────────────

impl WebSearchTool {
    /// DDG Instant Answer API — returns direct answers, abstracts, related topics.
    /// Uses the official JSON API endpoint (no API key, no block).
    async fn ddg_instant(&self, query: &str) -> Option<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct DdgResponse {
            #[serde(default)]
            answer: String,
            #[serde(default)]
            answer_type: String,
            #[serde(default)]
            abstract_text: String,
            #[serde(default)]
            abstract_source: String,
            #[serde(default)]
            abstract_url: String,
            #[serde(default)]
            definition: String,
            #[serde(default)]
            definition_source: String,
            #[serde(default)]
            related_topics: Vec<RelatedTopic>,
        }

        #[derive(Deserialize)]
        struct RelatedTopic {
            #[serde(rename = "Text", default)]
            text: String,
            #[serde(rename = "FirstURL", default)]
            first_url: String,
        }

        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            percent_encode(query)
        );

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .ok()?;

        let ddg: DdgResponse = resp.json().await.ok()?;
        let mut parts: Vec<String> = Vec::new();

        // Direct answer (e.g. "2 + 2 = 4", calculator)
        if !ddg.answer.is_empty() {
            let atype = if ddg.answer_type.is_empty() {
                String::new()
            } else {
                format!(" ({})", ddg.answer_type)
            };
            parts.push(format!("💡 **Direct Answer{}:** {}", atype, ddg.answer));
        }

        // Abstract (Wikipedia-style summary)
        if !ddg.abstract_text.is_empty() {
            let src = if ddg.abstract_source.is_empty() {
                String::new()
            } else if ddg.abstract_url.is_empty() {
                format!(" — *{}*", ddg.abstract_source)
            } else {
                format!(" — *{}* ({})", ddg.abstract_source, ddg.abstract_url)
            };
            parts.push(format!("📖 **Summary{}:**\n{}", src, ddg.abstract_text));
        }

        // Definition
        if !ddg.definition.is_empty() {
            let src = if ddg.definition_source.is_empty() {
                String::new()
            } else {
                format!(" ({})", ddg.definition_source)
            };
            parts.push(format!("📝 **Definition{}:** {}", src, ddg.definition));
        }

        // Related topics (up to 4)
        let topics: Vec<String> = ddg
            .related_topics
            .iter()
            .filter(|t| !t.text.is_empty() && !t.first_url.is_empty())
            .take(4)
            .map(|t| {
                let snippet = if t.text.len() > 120 {
                    format!("{}...", &t.text[..120])
                } else {
                    t.text.clone()
                };
                format!("• {} ({})", snippet, t.first_url)
            })
            .collect();

        if !topics.is_empty() {
            parts.push(format!("🔗 **Related:**\n{}", topics.join("\n")));
        }

        if parts.is_empty() {
            return None;
        }

        Some(format!(
            "## Instant Answer for: \"{}\"\n\n{}\n",
            query,
            parts.join("\n\n")
        ))
    }

    /// DuckDuckGo HTML search — full web results.
    async fn ddg_html(&self, query: &str, max: usize, lang: &str) -> Vec<SearchResult> {
        let kl = match lang {
            "id" => "id-id",
            "en" => "us-en",
            _ => "wt-wt", // worldwide
        };

        let resp = self
            .client
            .post("https://html.duckduckgo.com/html/")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9,id;q=0.8")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Referer", "https://duckduckgo.com/")
            .header("Origin", "https://duckduckgo.com")
            .form(&[
                ("q", query),
                ("b", ""),
                ("kl", kl),
                ("df", ""),
            ])
            .send()
            .await;

        let html = match resp {
            Ok(r) => r.text().await.unwrap_or_default(),
            Err(e) => {
                tracing::warn!("DDG HTML request failed: {}", e);
                return vec![];
            }
        };

        if html.len() < 500 {
            tracing::warn!("DDG HTML returned tiny response ({} bytes) — likely blocked", html.len());
            return vec![];
        }

        parse_ddg_html(&html, max)
    }

    /// DuckDuckGo Lite — simpler HTML, lower block rate on edge devices.
    async fn ddg_lite(&self, query: &str, max: usize, lang: &str) -> Vec<SearchResult> {
        let kl = match lang {
            "id" => "id-id",
            "en" => "us-en",
            _ => "wt-wt",
        };

        let url = format!(
            "https://lite.duckduckgo.com/lite/?q={}&kl={}",
            percent_encode(query),
            kl
        );

        let resp = self
            .client
            .get(&url)
            .header("Accept", "text/html,application/xhtml+xml")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Referer", "https://duckduckgo.com/")
            .send()
            .await;

        let html = match resp {
            Ok(r) => r.text().await.unwrap_or_default(),
            Err(e) => {
                tracing::warn!("DDG Lite request failed: {}", e);
                return vec![];
            }
        };

        if html.len() < 200 {
            return vec![];
        }

        parse_ddg_lite(&html, max)
    }

    /// Wikipedia Search API — free, JSON, no block, great for factual queries.
    async fn wikipedia_search(&self, query: &str, max: usize, lang: &str) -> Vec<SearchResult> {
        let wiki_lang = match lang {
            "id" => "id",  // Indonesian Wikipedia
            _ => "en",     // English Wikipedia
        };

        let url = format!(
            "https://{}.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&srlimit={}&format=json&utf8=1&srprop=snippet|titlesnippet",
            wiki_lang,
            percent_encode(query),
            max
        );

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await;

        let text = match resp {
            Ok(r) => r.text().await.unwrap_or_default(),
            Err(e) => {
                tracing::warn!("Wikipedia API failed: {}", e);
                return vec![];
            }
        };

        // Parse Wikipedia JSON response
        let v: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        let items = match v["query"]["search"].as_array() {
            Some(a) => a,
            None => return vec![],
        };

        items
            .iter()
            .take(max)
            .filter_map(|item| {
                let title = item["title"].as_str()?.to_string();
                let snippet = item["snippet"].as_str()
                    .map(strip_html_tags)
                    .unwrap_or_default();
                let page_id = item["pageid"].as_u64()?;
                let url = format!(
                    "https://{}.wikipedia.org/?curid={}",
                    wiki_lang, page_id
                );
                Some(SearchResult { title, url, snippet })
            })
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HTML parsers
// ─────────────────────────────────────────────────────────────────────────────

/// A single search result.
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Parse DuckDuckGo HTML (main endpoint) search results.
fn parse_ddg_html(html: &str, max: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut pos = 0;

    while results.len() < max {
        let link_marker = "class=\"result__a\"";
        let link_start = match html[pos..].find(link_marker) {
            Some(p) => pos + p,
            None => break,
        };

        // href is before the class attribute in DDG
        let search_win = link_start.saturating_sub(300);
        let url = extract_attr(&html[search_win..link_start + 50], "href")
            .unwrap_or_default();

        // Title: content between > and </a>
        let title_open = match html[link_start..].find('>') {
            Some(p) => link_start + p + 1,
            None => { pos = link_start + 20; continue; }
        };
        let title_close = match html[title_open..].find("</a>") {
            Some(p) => title_open + p,
            None => { pos = title_open; continue; }
        };
        let title = strip_html_tags(&html[title_open..title_close]);

        // Snippet: look for class="result__snippet" after title
        let snippet = find_class_content(html, title_close, "result__snippet")
            .unwrap_or_default();

        let clean = clean_ddg_url(&url);
        if !title.is_empty() && !clean.is_empty() {
            results.push(SearchResult {
                title,
                url: clean,
                snippet,
            });
        }

        pos = title_close + 5;
    }

    results
}

/// Parse DuckDuckGo Lite search results.
fn parse_ddg_lite(html: &str, max: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut pos = 0;

    while results.len() < max {
        let link_marker = "class=\"result-link\"";
        let link_start = match html[pos..].find(link_marker) {
            Some(p) => pos + p,
            None => break,
        };

        let search_win = link_start.saturating_sub(200);
        let url = match extract_attr(&html[search_win..link_start + 50], "href") {
            Some(u) if !u.is_empty() => u,
            _ => { pos = link_start + 20; continue; }
        };

        let title_open = match html[link_start..].find('>') {
            Some(p) => link_start + p + 1,
            None => { pos = link_start + 20; continue; }
        };
        let title_close = match html[title_open..].find("</a>") {
            Some(p) => title_open + p,
            None => { pos = title_open; continue; }
        };
        let title = strip_html_tags(&html[title_open..title_close]);

        let snippet = find_class_content(html, title_close, "result-snippet")
            .unwrap_or_default();

        if !title.is_empty() {
            results.push(SearchResult { title, url, snippet });
        }
        pos = title_close + 5;
    }

    results
}

// ─────────────────────────────────────────────────────────────────────────────
// HTML helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Find content inside an element with the given class, starting at `from`.
fn find_class_content(html: &str, from: usize, class: &str) -> Option<String> {
    let marker = format!("class=\"{}\"", class);
    let pos = html[from..].find(&marker)? + from;
    let cs = html[pos..].find('>')? + pos + 1;
    // Limit search window to avoid crossing into next result
    let window = &html[cs..(cs + 1000).min(html.len())];
    let ce = window.find("</")?;
    let raw = &window[..ce];
    let text = strip_html_tags(raw);
    if text.is_empty() { None } else { Some(text) }
}

/// Extract the value of an HTML attribute (e.g. href="...").
fn extract_attr(html: &str, attr: &str) -> Option<String> {
    let key = format!("{}=\"", attr);
    let start = html.find(&key)? + key.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_string())
}

/// Strip HTML tags and decode common HTML entities.
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
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Unwrap DuckDuckGo redirect URL to extract the real destination URL.
fn clean_ddg_url(url: &str) -> String {
    if let Some(start) = url.find("uddg=") {
        let encoded = &url[start + 5..];
        let end = encoded.find('&').unwrap_or(encoded.len());
        return url_decode(&encoded[..end]);
    }
    if url.starts_with("http") {
        return url.to_string();
    }
    if url.starts_with("//") {
        return format!("https:{}", url);
    }
    url.to_string()
}

/// Percent-encode a string for use in URL query parameters.
fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(ch),
            ' ' => out.push('+'),
            _ => {
                for b in ch.to_string().as_bytes() {
                    out.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    out
}

/// Percent-decode a URL query value.
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
