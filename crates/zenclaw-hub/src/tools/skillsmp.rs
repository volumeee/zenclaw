//! SkillsMP Integration — Dymanically fetch and learn new AI skills.
//!
//! Connects to SkillsMP.com, a community marketplace for SKILL.md files.
//! Bypasses Cloudflare block by utilizing Jina Reader API natively.
//!

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{info, warn, debug};

use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::tool::Tool;

pub struct SkillsMpTool {
    client: Client,
    api_key: String,
    jina_api_key: Option<String>,
}

impl SkillsMpTool {
    pub fn new(config: Option<zenclaw_core::config::ToolSettings>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("zenclaw-bot")
            .build()
            .unwrap_or_default();

        let api_key = config
            .as_ref()
            .and_then(|c| c.skillsmp_api_key.clone())
            .unwrap_or_default();

        let jina_api_key = config.and_then(|c| c.jina_api_key);

        Self { client, api_key, jina_api_key }
    }
}

impl Default for SkillsMpTool {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl Tool for SkillsMpTool {
    fn name(&self) -> &str {
        "skillsmp_fetch"
    }

    fn description(&self) -> &str {
        "Search and dynamically fetch new AI Agent Skills from SkillsMP.com marketplace.\n\
        Skills are standardized instructions (SKILL.md) that give you new abilities to solve complex multi-step tasks.\n\
        Use this WHENEVER you face a task you are unsure how to handle optimally (e.g. integrating a new API, deploying to a specific cloud, setting up a framework).\n\
        It returns expert instructions on exactly how to accomplish the task."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "What skill do you need? E.g. 'deploy react app to vercel', 'build python fastapi', 'analyze github repo'."
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        if query.trim().is_empty() {
            return Err(ZenClawError::ToolExecution { 
                tool: "skillsmp".into(), 
                message: "query is required".into() 
            });
        }

        info!("Searching SkillsMP for: {}", query);

        let encoded_query = crate::tools::web_search::percent_encode(query);
        let search_url = format!("https://skillsmp.com/api/v1/skills/search?q={}", encoded_query);
        
        let mut req = self.client.get(&search_url).header("Accept", "application/json");

        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let res = req.send().await.map_err(|e| ZenClawError::ToolExecution { 
            tool: "skillsmp".into(), 
            message: format!("Search failed: {}", e) 
        })?;
        
        if res.status() == 401 {
             return Ok("SkillsMP API requires an API key. Please generate one at https://skillsmp.com and set it using `zenclaw config set skillsmp_api_key <KEY>` or by setting SKILLSMP_API_KEY environment variable.".to_string());
        } else if !res.status().is_success() {
             return Err(ZenClawError::ToolExecution { 
                 tool: "skillsmp".into(), 
                 message: format!("SkillsMP API Error: {}", res.status()) 
             });
        }

        let search_text = res.text().await.map_err(|e| ZenClawError::ToolExecution { 
            tool: "skillsmp".into(), 
            message: format!("Parse text error: {}", e) 
        })?;
        
        let search_data: Value = serde_json::from_str(&search_text)
            .map_err(|e| ZenClawError::ToolExecution { 
                tool: "skillsmp".into(), 
                message: format!("Parse json error: {}", e) 
            })?;
            
        let items_arr = search_data.get("data").and_then(|d| d.get("items")).and_then(|i| i.as_array());
        if items_arr.is_none() || items_arr.unwrap().is_empty() {
            return Ok(format!("No relevant skills found on SkillsMP for '{}'. You will have to use your own knowledge or standard web search.", query));
        }

        // Try to get the first valid github URL
        let mut skill_url_opt = None;
        let mut title_opt = "Unknown Skill".to_string();
        
        for item in items_arr.unwrap() {
            if let Some(url) = item.get("githubUrl").and_then(|u| u.as_str()) {
                // If it's a GitHub tree link, we fetch the SKILL.md inside it
                // e.g., https://github.com/author/repo/tree/main/skills/foo -> https://github.com/author/repo/tree/main/skills/foo/SKILL.md
                let mut scrape_url = url.to_string();
                if !scrape_url.ends_with(".md") && scrape_url.ends_with('/') {
                    scrape_url.push_str("SKILL.md");
                } else if !scrape_url.ends_with(".md") {
                    scrape_url.push_str("/SKILL.md");
                }
                skill_url_opt = Some(scrape_url);
                if let Some(t) = item.get("name").and_then(|name| name.as_str()) {
                    title_opt = t.to_string();
                }
                break;
            }
        }
        
        if skill_url_opt.is_none() {
             return Ok("No valid GitHub links found in the search results.".to_string());
        }
        
        let skill_url = skill_url_opt.unwrap();

        info!("Found skill '{}' at {}, fetching content...", title_opt, skill_url);
        
        // Fetch via Jina Reader
        let read_url = format!("https://r.jina.ai/{}", skill_url);
        let mut read_req = self.client.get(&read_url)
            .header("X-Return-Format", "markdown");
            
        let jina_key_env = self.jina_api_key.as_deref().unwrap_or("");
        if !jina_key_env.is_empty() {
            read_req = read_req.header("Authorization", format!("Bearer {}", jina_key_env));
        }
        
        let read_res = read_req.send().await.map_err(|e| ZenClawError::ToolExecution { 
            tool: "skillsmp".into(), 
            message: format!("Read failed: {}", e) 
        })?;
        
        if !read_res.status().is_success() {
             warn!("Failed to fetch skill content from {}", skill_url);
             return Ok(format!("Found skill '{}' at {} but failed to fetch its content. Try another approach.", title_opt, skill_url));
        }

        let skill_content = read_res.text().await.map_err(|e| ZenClawError::ToolExecution { 
            tool: "skillsmp".into(), 
            message: e.to_string() 
        })?;
        
        debug!("Fetched skill content from SkillsMP.");
        
        Ok(format!(
            "🎯 SKILL FOUND FROM SkillsMP Marketplace 🎯\n\n\
            Title: {}\n\
            URL: {}\n\
            \n\
            === SKILL INSTRUCTIONS ===\n\
            {}\n\
            === END OF SKILL ===\n\n\
            Please carefully read and execute the instructions above step-by-step to complete the user's task.", 
            title_opt, skill_url, skill_content
        ))
    }
}
