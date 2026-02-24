//! Web search tool — multi-engine, robust, zero API key required.
//!
//! Search strategy (all run in parallel, results merged + deduplicated):
//!
//! 1. DDG Instant Answer API  — JSON, direct factual answers (no HTML parsing)
//! 2. Jina Search (s.jina.ai) — reliable web search, returns clean markdown
//! 3. DDG Lite endpoint        — Simpler HTML, good fallback on restricted nets
//! 4. Wikipedia Search API     — Best for encyclopedic/factual queries
//!
//! Language detection is delegated to the LLM agent loop via the `lang`
//! parameter — the AI detects the user's language and passes 'id'/'en'/etc.
//! This keeps the tool generic with no hardcoded language heuristics.

use async_trait::async_trait;
use reqwest::Client;
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

fn pick_user_agent() -> &'static str {
    let idx = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as usize)
        .unwrap_or(0)
        % USER_AGENTS.len();
    USER_AGENTS[idx]
}

fn percent_encode(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "+".to_string(),
        c => {
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            encoded.bytes().map(|b| format!("%{:02X}", b)).collect()
        }
    }).collect()
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
        "Search the internet for current information. Returns titles, URLs, and snippets.\n\
        Provides direct answers for factual questions (who, what, when, where, how).\n\
        Uses multiple search engines in parallel for maximum reliability.\n\
        IMPORTANT: Detect the language of the user's query and pass it as 'lang' parameter:\n\
        - If user writes in Indonesian/Bahasa Indonesia → pass lang='id'\n\
        - If user writes in English → pass lang='en'\n\
        - If unsure → pass lang='auto'\n\
        Always search in the same language as the user's question for best results."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query. Write in the same language as the user's question."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results to return (1-15, default: 8)"
                },
                "lang": {
                    "type": "string",
                    "enum": ["id", "en", "auto"],
                    "description": "Language of the query. 'id' for Indonesian (uses id.wikipedia.org + Jina in Indonesian), 'en' for English, 'auto' to let the tool decide."
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
        let (instant_opt, jina_results, lite_results, wiki_results) = tokio::join!(
            self.ddg_instant(query),
            self.jina_search(query, max_results, lang),
            self.ddg_lite(query, max_results, lang),
            self.wikipedia_search(query, 4, lang),
        );

        // ── Strategy 1: DDG Instant Answer (direct factual answer, highest priority) ──
        if let Some(instant) = instant_opt {
            output.push_str(&instant);
            output.push_str("\n\n");
        }

        // ── Strategy 2: Jina Search (primary web results, clean markdown) ──
        for r in jina_results {
            if seen_urls.insert(r.url.clone()) {
                all_results.push(r);
            }
        }

        // ── Strategy 3: DDG Lite (fallback, always merge — dedup handles overlap) ──
        for r in lite_results {
            if seen_urls.insert(r.url.clone()) {
                all_results.push(r);
            }
        }

        // ── Strategy 4: Wikipedia (always — best for factual/encyclopedic queries) ──
        let mut wiki_top_url: Option<String> = None;
        for r in wiki_results {
            if wiki_top_url.is_none() {
                wiki_top_url = Some(r.url.clone());
            }
            if seen_urls.insert(r.url.clone()) {
                all_results.push(r);
            }
        }

        // ── Strategy 5: Jina Reader — full article when no instant answer ──
        // Fetches the top Wikipedia article for rich factual context.
        // Useful on networks where DuckDuckGo endpoints are blocked.
        if output.trim().is_empty() && let Some(wiki_url) = wiki_top_url {
            tracing::info!("web_search: Jina Reader fallback for: {}", wiki_url);
            let jina_url = format!("https://r.jina.ai/{}", wiki_url);
            if let Ok(resp) = self.client.get(&jina_url)
                .header("Accept", "text/plain")
                .send().await
                && let Ok(text) = resp.text().await
            {
                let trimmed = if text.len() > 2000 {
                    format!("{}...\n*(truncated — use web_fetch for full article)*", &text[..2000])
                } else {
                    text
                };
                if trimmed.len() > 100 {
                    output.push_str("## Article Content (via Jina Reader)\n\n");
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
                • Use `web_scrape` for deep content extraction\n\
                • The network may be restricted — check connectivity",
                query
            ));
        }

        let bytes = output.len();
        tracing::debug!("web_search: {} total results ({} bytes)", take, bytes);
        Ok(output)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Engine implementations
// ─────────────────────────────────────────────────────────────────────────────

impl WebSearchTool {
    /// DDG Instant Answer API — returns direct answers, abstracts, related topics.
    /// JSON format, no HTML parsing required, very low block risk.
    async fn ddg_instant(&self, query: &str) -> Option<String> {
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            percent_encode(query)
        );

        let resp = self.client.get(&url)
            .header("Accept", "application/json")
            .send().await.ok()?;

        let text = resp.text().await.ok()?;
        if text.len() < 10 { return None; }

        let v: Value = serde_json::from_str(&text).ok()?;
        let mut out = String::new();

        // Direct answer (e.g. "2+2 = 4", unit conversions)
        if let Some(ans) = v["Answer"].as_str()
            && !ans.is_empty()
        {
            out.push_str(&format!("💡 **Direct Answer**: {}\n", ans));
        }


        // Wikipedia abstract
        if let Some(abs) = v["AbstractText"].as_str()
            && !abs.is_empty()
        {
            let src = v["AbstractURL"].as_str().unwrap_or("");
            out.push_str(&format!("📖 **Summary**: {}\n", abs));
            if !src.is_empty() {
                out.push_str(&format!("   🔗 {}\n", src));
            }
        }


        // Definition
        if let Some(def) = v["Definition"].as_str()
            && !def.is_empty()
        {
            out.push_str(&format!("📚 **Definition**: {}\n", def));
        }


        // Related topics (top 3)
        if let Some(topics) = v["RelatedTopics"].as_array() {
            let mut count = 0;
            for topic in topics {
                if count >= 3 { break; }
                if let (Some(text), Some(url)) = (topic["Text"].as_str(), topic["FirstURL"].as_str())
                    && !text.is_empty()
                {
                    out.push_str(&format!("• {} — {}\n", text, url));
                    count += 1;
                }

            }
        }

        if out.is_empty() { None } else { Some(format!("## Instant Answer\n{}", out)) }
    }

    /// Jina Search (s.jina.ai) — reliable web search returning clean markdown.
    /// Free without API key, works on most networks.
    /// Handles both JSON response and plain-text/markdown fallback.
    async fn jina_search(&self, query: &str, max: usize, lang: &str) -> Vec<SearchResult> {
        // Jina Search supports locale via X-Locale header
        let locale = match lang {
            "id" => "id-ID",
            "en" => "en-US",
            _ => "",
        };

        let url = format!("https://s.jina.ai/{}", percent_encode(query));

        let mut req = self.client.get(&url)
            .header("Accept", "application/json")
            .header("X-Retain-Images", "none");

        if !locale.is_empty() {
            req = req.header("X-Locale", locale);
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Jina Search request failed: {}", e);
                return vec![];
            }
        };

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if text.len() < 20 {
            tracing::warn!("Jina Search empty response (status={}, {} bytes)", status, text.len());
            return vec![];
        }

        // ── Try JSON parsing first ──────────────────────────────────────────
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            // Try both 'data' and 'results' keys (API may vary)
            let items_opt = v["data"].as_array()
                .or_else(|| v["results"].as_array())
                .or_else(|| v["items"].as_array());

            if let Some(items) = items_opt {
                tracing::debug!("Jina Search JSON: {} results", items.len());
                return items.iter().take(max).filter_map(|item| {
                    let title = item["title"].as_str().unwrap_or("").to_string();
                    let url = item["url"].as_str().unwrap_or("").to_string();
                    if url.is_empty() { return None; }

                    let snippet = if let Some(desc) = item["description"].as_str() {
                        desc.to_string()
                    } else if let Some(content) = item["content"].as_str() {
                        let t = content.trim();
                        if t.len() > 200 { format!("{}...", &t[..200]) } else { t.to_string() }
                    } else {
                        String::new()
                    };

                    Some(SearchResult { title, url, snippet })
                }).collect();
            }

            // JSON parsed but no known array key — log first 300 chars for debugging
            tracing::warn!(
                "Jina Search JSON has no 'data'/'results'/'items' key (status={}). Response preview: {}",
                status,
                &text[..text.len().min(300)]
            );
        }

        // ── Fallback: parse plain-text/markdown response ────────────────────
        // Jina text format:
        //   1. [Title](URL)
        //   Description text...
        //
        //   2. [Title](URL)
        //   ...
        tracing::info!("Jina Search: falling back to text parser (status={})", status);
        parse_jina_text(&text, max)
    }


    /// DDG Lite — simpler HTML format, low block rate on congested/edge IPs.
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

    /// Wikipedia Search API — free, JSON, no auth, great for factual queries.
    /// Lang is passed directly from the LLM tool call — no hardcoded detection.
    async fn wikipedia_search(&self, query: &str, max: usize, lang: &str) -> Vec<SearchResult> {
        // Map lang to Wikipedia language code; default to English
        // The LLM is instructed in the tool description to pass 'id' for Indonesian
        let wiki_lang = match lang {
            "id" => "id",
            "en" => "en",
            // For 'auto', try both in parallel and merge
            _ => "en",
        };

        // When auto, also search Indonesian Wikipedia if query might be Indonesian
        // We rely on simple heuristic: any non-ASCII Latin characters → likely Indonesian
        // (but this is just a bonus, not the primary detection mechanism)
        let langs_to_try: &[&str] = if lang == "auto" {
            &["en", "id"]
        } else {
            std::slice::from_ref(&wiki_lang)
        };

        let mut results = Vec::new();
        let mut seen: HashSet<u64> = HashSet::new();

        for &wl in langs_to_try {
            let url = format!(
                "https://{}.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&srlimit={}&format=json&utf8=1&srprop=snippet|titlesnippet",
                wl,
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
                    tracing::warn!("Wikipedia {} API failed: {}", wl, e);
                    continue;
                }
            };

            let v: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let items = match v["query"]["search"].as_array() {
                Some(a) => a,
                None => continue,
            };

            for item in items.iter().take(max) {
                let Some(title) = item["title"].as_str() else { continue };
                let Some(page_id) = item["pageid"].as_u64() else { continue };

                if !seen.insert(page_id) { continue; } // dedup across wikis

                let snippet = item["snippet"].as_str()
                    .map(strip_html_tags)
                    .unwrap_or_default();
                let url = format!("https://{}.wikipedia.org/?curid={}", wl, page_id);
                results.push(SearchResult { title: title.to_string(), url, snippet });

                if results.len() >= max { break; }
            }

            if results.len() >= max { break; }
        }

        results
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HTML parsers + helpers
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

        // Extract href
        let href_slice = &html[link_start..];
        let href_start = match href_slice.find("href=\"") {
            Some(p) => link_start + p + 6,
            None => { pos = link_start + 1; continue; }
        };
        let href_end = match html[href_start..].find('"') {
            Some(p) => href_start + p,
            None => { pos = link_start + 1; continue; }
        };
        let raw_url = &html[href_start..href_end];

        // DDG wraps URLs — extract the real one
        let url = if let Some(idx) = raw_url.find("uddg=") {
            let encoded = &raw_url[idx + 5..];
            encoded.replace("%3A", ":").replace("%2F", "/")
                .replace("%3F", "?").replace("%3D", "=").replace("%26", "&")

        } else {
            raw_url.to_string()
        };

        if url.is_empty() || url.starts_with("//") {
            pos = link_start + 1;
            continue;
        }

        // Extract title
        let text_start = match html[href_end..].find('>') {
            Some(p) => href_end + p + 1,
            None => { pos = link_start + 1; continue; }
        };
        let text_end = match html[text_start..].find('<') {
            Some(p) => text_start + p,
            None => { pos = link_start + 1; continue; }
        };
        let title = html[text_start..text_end].trim().to_string();

        // Extract snippet
        let snippet = {
            let snip_marker = "class=\"result__snippet\"";
            if let Some(sp) = html[link_start..].find(snip_marker) {
                let snip_slice = &html[link_start + sp..];
                if let Some(s) = snip_slice.find('>') {
                    let snip_text_start = link_start + sp + s + 1;
                    if let Some(e) = html[snip_text_start..].find('<') {
                        html[snip_text_start..snip_text_start + e].trim().to_string()
                    } else { String::new() }
                } else { String::new() }
            } else { String::new() }
        };

        if !title.is_empty() && !url.is_empty() {
            results.push(SearchResult { title, url, snippet });
        }

        pos = link_start + 1;
    }

    results
}

/// Parse DuckDuckGo Lite HTML search results.
fn parse_ddg_lite(html: &str, max: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut pos = 0;

    while results.len() < max {
        // DDG Lite results use <a class="result-link"> or plain <a> inside result table
        let link_marker = "class=\"result-link\"";
        let fallback_marker = "<td class=\"result-snippet\">";

        let link_start = match html[pos..].find(link_marker) {
            Some(p) => pos + p,
            None => break,
        };

        let href_start = match html[..link_start].rfind("href=\"") {
            Some(p) => p + 6,
            None => {
                // Try finding href after marker
                match html[link_start..].find("href=\"") {
                    Some(p) => link_start + p + 6,
                    None => { pos = link_start + 1; continue; }
                }
            }
        };
        let href_end = match html[href_start..].find('"') {
            Some(p) => href_start + p,
            None => { pos = link_start + 1; continue; }
        };
        let url = html[href_start..href_end].to_string();

        if url.is_empty() || !url.starts_with("http") {
            pos = link_start + 1;
            continue;
        }

        // Title: text inside the <a class="result-link">...</a>
        let text_start = match html[link_start..].find('>') {
            Some(p) => link_start + p + 1,
            None => { pos = link_start + 1; continue; }
        };
        let text_end = match html[text_start..].find('<') {
            Some(p) => text_start + p,
            None => { pos = link_start + 1; continue; }
        };
        let title = html[text_start..text_end].trim().to_string();

        // Snippet: next <td class="result-snippet">
        let snippet = if let Some(sp) = html[text_end..].find(fallback_marker) {
            let snip_abs = text_end + sp + fallback_marker.len();
            if let Some(e) = html[snip_abs..].find("</td>") {
                strip_html_tags(&html[snip_abs..snip_abs + e])
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if !title.is_empty() {
            results.push(SearchResult { title, url, snippet });
        }

        pos = link_start + 1;
    }

    results
}

/// Strip common HTML tags from a string (for Wikipedia snippets).
fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// Parse Jina Search plain-text/markdown response.
///
/// Jina text format (when JSON is unavailable or rate-limited):
/// ```text
/// 1. [Title](https://url)
/// Description or content snippet...
///
/// 2. [Title](https://url)
/// ...
/// ```
fn parse_jina_text(text: &str, max: usize) -> Vec<SearchResult> {

    let mut results = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() && results.len() < max {
        let line = lines[i].trim();

        // Match numbered entries: "1." "2." etc, possibly with markdown link
        let is_numbered = line.starts_with(|c: char| c.is_ascii_digit())
            && line.contains(". ");

        if is_numbered {
            // Try to extract [Title](URL) from this line or next non-empty line
            let search_line = if line.contains("](http") {
                line
            } else if i + 1 < lines.len() && lines[i+1].contains("](http") {
                lines[i+1].trim()
            } else {
                i += 1;
                continue;
            };

            // Parse markdown link: [Title](URL)
            if let (Some(title_start), Some(title_end)) = (search_line.find('['), search_line.find("](")) {
                let title = search_line[title_start + 1..title_end].to_string();
                let url_start = title_end + 2;
                if let Some(url_end) = search_line[url_start..].find(')') {
                    let url = search_line[url_start..url_start + url_end].to_string();
                    if !url.is_empty() && url.starts_with("http") {
                        // Collect snippet from subsequent lines until blank or next entry
                        let mut snippet_lines = Vec::new();
                        let mut j = i + 1;
                        while j < lines.len() {
                            let next = lines[j].trim();
                            // Stop at blank line after collecting some content,
                            // or at next numbered entry
                            if next.is_empty() && !snippet_lines.is_empty() { break; }
                            if next.starts_with(|c: char| c.is_ascii_digit()) && next.contains(". ") { break; }
                            if !next.is_empty() && !next.starts_with("](") {
                                snippet_lines.push(next);
                            }
                            j += 1;
                        }
                        let snippet = snippet_lines.join(" ");
                        let snippet = if snippet.len() > 200 {
                            format!("{}...", &snippet[..200])
                        } else {
                            snippet
                        };

                        results.push(SearchResult { title, url, snippet });
                    }
                }
            }
        }

        i += 1;
    }

    results
}


// Keep parse_ddg_html available (used internally — suppress dead_code warning)
#[allow(dead_code)]
fn _ensure_ddg_html_parser_available() {
    let _ = parse_ddg_html("", 0);
}
