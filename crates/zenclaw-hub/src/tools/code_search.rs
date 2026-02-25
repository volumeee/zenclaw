//! Semantic codebase search tool — recursively scan files for definitions or keywords.
//!
//! Understands `.gitignore` automatically. Highly efficient thanks to `ignore` crate.

use async_trait::async_trait;
use ignore::WalkBuilder;
use regex::RegexBuilder;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

pub struct CodebaseSearchTool;

impl CodebaseSearchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodebaseSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CodebaseSearchTool {
    fn name(&self) -> &str {
        "code_search"
    }

    fn description(&self) -> &str {
        "Search precisely for code, text, definitions, or functions across the entire repository. \
        Automatically ignores node_modules, target directories, and .gitignores. \
        Returns snippets and line numbers of the matches. \
        Use this INSTEAD of guessing file paths when trying to fix bugs or analyze code architecture."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The exact string or regular expression to search for (e.g., 'fn login', 'class User')."
                },
                "dir": {
                    "type": "string",
                    "description": "Directory to search in. Default is current directory ('.')."
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Set to true for case-sensitive, false for case-insensitive. Default: false."
                },
                "file_extension": {
                    "type": "string",
                    "description": "Optional: Only search files ending in this extension (e.g., 'rs', 'js', 'py'). Do not include the dot."
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let query = args["query"].as_str().unwrap_or("").to_string();
        if query.is_empty() {
            return Ok("Error: 'query' parameter is required.".into());
        }

        let dir = args["dir"].as_str().unwrap_or(".").to_string();
        let case_sensitive = args["case_sensitive"].as_bool().unwrap_or(false);
        let ext_filter = args["file_extension"].as_str().map(|s| s.to_string());

        // Compile robust regex, escaping if it fails standard parse but we want intelligent fallback
        let re = match RegexBuilder::new(&query)
            .case_insensitive(!case_sensitive)
            .build()
        {
            Ok(r) => r,
            Err(_) => {
                // if invalid regex (e.g., user passed symbol chars), fallback to literal escape
                let escaped = regex::escape(&query);
                match RegexBuilder::new(&escaped)
                    .case_insensitive(!case_sensitive)
                    .build()
                {
                    Ok(r) => r,
                    Err(e) => return Ok(format!("Fatal regex error: {}", e)),
                }
            }
        };

        let root_path_clone = dir.clone(); // so we can access it within `spawn_blocking`
        let root_path_check = Path::new(&dir);
        if !root_path_check.exists() {
            return Ok(format!("Directory not found: {}", dir));
        }

        // We run the walk on a blocking thread because `ignore` crate is synchronous
        let ext = ext_filter.clone();
        let result = tokio::task::spawn_blocking(move || {
            let root_path = Path::new(&root_path_clone);
            let mut walker = WalkBuilder::new(root_path);
            walker.hidden(true);   // respect hidden files
            walker.ignore(true);   // respect .ignore
            walker.git_ignore(true); // respect .gitignore

            // We want to collect results. To prevent memory overflow on huge codebases (like linux kernel search),
            // we'll cap our results to the first 30 matches.
            let max_matches = 30;
            let match_count = AtomicUsize::new(0);

            // Output collector
            let (tx, rx) = std::sync::mpsc::channel();

            walker.build_parallel().run(|| {
                let tx = tx.clone();
                let re = re.clone();
                let ext = ext.clone();
                let match_count = &match_count;

                Box::new(move |entry| {
                    if match_count.load(Ordering::Relaxed) >= max_matches {
                        return ignore::WalkState::Quit;
                    }

                    if let Ok(ent) = entry {
                        let path = ent.path();

                        // Fast fail if not a file
                        if !path.is_file() {
                            return ignore::WalkState::Continue;
                        }

                        // Check extension if provided
                        if let Some(e) = &ext {
                            let matches_ext = path
                                .extension()
                                .and_then(|s| s.to_str())
                                .map(|s| s == e)
                                .unwrap_or(false);
                            
                            if !matches_ext {
                                return ignore::WalkState::Continue;
                            }
                        }

                        // Read and scan file
                        if let Ok(content) = fs::read_to_string(path) {
                            let mut file_matches = Vec::new();

                            // Track line number manually for speed
                            for (line_idx, line) in content.lines().enumerate() {
                                if re.is_match(line) {
                                    // Found a match! grab the line number (1-based)
                                    let line_num = line_idx + 1;
                                    file_matches.push(format!("    {}: {}", line_num, line.trim()));
                                    
                                    let current = match_count.fetch_add(1, Ordering::Relaxed);
                                    if current >= max_matches {
                                        break;
                                    }
                                }
                            }

                            if !file_matches.is_empty() {
                                let _ = tx.send((path.to_string_lossy().to_string(), file_matches));
                            }
                        }
                    }
                    ignore::WalkState::Continue
                })
            });

            // Reconstruct the result from the channel
            drop(tx); // close so rx can finish
            let mut final_out = String::new();
            let mut total_files = 0;
            let mut total_hits = 0;

            for (file_path, hits) in rx {
                total_files += 1;
                total_hits += hits.len();
                final_out.push_str(&format!("\n📄 {}\n", file_path));
                for hit in hits {
                    final_out.push_str(&format!("{}\n", hit));
                }
            }

            if total_hits >= max_matches {
                final_out.push_str(&format!(
                    "\n⚠️ Showing first {} matches. Search truncated to save context.",
                    max_matches
                ));
            } else if total_hits == 0 {
                final_out.push_str("No matches found in the codebase.");
            } else {
                final_out.push_str(&format!("\n✅ Found {} matches across {} files.", total_hits, total_files));
            }

            final_out
        })
        .await;

        match result {
            Ok(data) => Ok(data.trim().to_string()),
            Err(_) => Ok("Error executing background search thread".into()),
        }
    }
}
