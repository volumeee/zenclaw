//! File indexer tool — index files into RAG for context-aware responses.
//!
//! Reads files and indexes their content into the RAG store,
//! enabling the agent to search and reference them during conversations.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

use crate::memory::RagStore;

/// File indexer tool — indexes files into RAG.
pub struct IndexerTool {
    rag: Arc<Mutex<Option<RagStore>>>,
}

impl IndexerTool {
    pub fn new(rag: Arc<Mutex<Option<RagStore>>>) -> Self {
        Self { rag }
    }
}

#[async_trait]
impl Tool for IndexerTool {
    fn name(&self) -> &str {
        "index_file"
    }

    fn description(&self) -> &str {
        "Index a file or directory into the knowledge base (RAG) for semantic search. Indexed files can be searched and referenced during conversations."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File or directory path to index"
                },
                "action": {
                    "type": "string",
                    "enum": ["index", "search", "stats"],
                    "description": "Action: 'index' a file, 'search' the knowledge base, or show 'stats'"
                },
                "query": {
                    "type": "string",
                    "description": "Search query (for 'search' action)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let action = args["action"].as_str().unwrap_or("stats");

        let rag_lock = self.rag.lock().await;
        let rag = match rag_lock.as_ref() {
            Some(r) => r,
            None => return Ok("RAG store not available. Start with --rag flag.".to_string()),
        };

        match action {
            "index" => {
                let path_str = args["path"].as_str().unwrap_or(".");
                let path = PathBuf::from(path_str);

                if !path.exists() {
                    return Ok(format!("Path not found: {}", path.display()));
                }

                if path.is_file() {
                    // Index single file
                    match index_single_file(rag, &path) {
                        Ok(chunks) => Ok(format!(
                            "✅ Indexed {} ({} chunks)",
                            path.display(),
                            chunks
                        )),
                        Err(e) => Ok(format!("❌ Failed to index {}: {}", path.display(), e)),
                    }
                } else if path.is_dir() {
                    // Index directory recursively
                    let mut total_files = 0;
                    let mut total_chunks = 0;
                    let mut errors = Vec::new();

                    index_directory(rag, &path, &mut total_files, &mut total_chunks, &mut errors);

                    let mut result = format!(
                        "✅ Indexed {} files ({} chunks) from {}",
                        total_files,
                        total_chunks,
                        path.display()
                    );

                    if !errors.is_empty() {
                        result.push_str(&format!("\n⚠️ {} errors:", errors.len()));
                        for err in errors.iter().take(5) {
                            result.push_str(&format!("\n  • {}", err));
                        }
                    }

                    Ok(result)
                } else {
                    Ok(format!("Not a file or directory: {}", path.display()))
                }
            }
            "search" => {
                let query = args["query"].as_str().unwrap_or("");
                if query.is_empty() {
                    return Ok("Search query is required.".to_string());
                }

                match rag.search(query, 5) {
                    Ok(results) => {
                        if results.is_empty() {
                            return Ok(format!("No results found for: {}", query));
                        }

                        let mut output = format!("Found {} results:\n", results.len());
                        for (i, doc) in results.iter().enumerate() {
                            let preview = if doc.content.len() > 200 {
                                format!("{}...", &doc.content[..200])
                            } else {
                                doc.content.clone()
                            };
                            output.push_str(&format!(
                                "\n{}. {} (score: {:.2})\n   {}\n",
                                i + 1,
                                doc.source,
                                doc.rank,
                                preview.replace('\n', "\n   ")
                            ));
                        }
                        Ok(output)
                    }
                    Err(e) => Ok(format!("Search error: {}", e)),
                }
            }
            "stats" => match rag.count() {
                Ok(count) => Ok(format!("📊 RAG Knowledge Base: {} documents indexed", count)),
                Err(e) => Ok(format!("Stats error: {}", e)),
            },
            _ => Ok(format!(
                "Unknown action: {}. Use 'index', 'search', or 'stats'.",
                action
            )),
        }
    }
}

/// Index a single file into RAG.
fn index_single_file(rag: &RagStore, path: &std::path::Path) -> std::result::Result<usize, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

    if content.is_empty() {
        return Ok(0);
    }

    let source = path.display().to_string();

    // For large files, chunk them
    if content.split_whitespace().count() > 200 {
        let ids = rag
            .index_chunked(&source, &content, 200, 30)
            .map_err(|e| e.to_string())?;
        Ok(ids.len())
    } else {
        rag.index(&source, &content, "").map_err(|e| e.to_string())?;
        Ok(1)
    }
}

/// Index a directory recursively.
fn index_directory(
    rag: &RagStore,
    dir: &std::path::Path,
    total_files: &mut usize,
    total_chunks: &mut usize,
    errors: &mut Vec<String>,
) {
    // Supported text extensions
    let text_exts = [
        "rs", "py", "js", "ts", "go", "c", "cpp", "h", "java", "rb", "sh", "bash",
        "md", "txt", "toml", "yaml", "yml", "json", "xml", "html", "css", "sql",
        "dockerfile", "makefile", "cfg", "ini", "env", "csv",
    ];

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("{}: {}", dir.display(), e));
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden files/dirs
        if path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false) {
            continue;
        }

        // Skip common build/dependency dirs
        let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
        if matches!(dir_name.as_ref(), "node_modules" | "target" | ".git" | "__pycache__" | "dist" | "build") {
            continue;
        }

        if path.is_dir() {
            index_directory(rag, &path, total_files, total_chunks, errors);
        } else if path.is_file() {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            if text_exts.contains(&ext.as_str()) || path.file_name().map(|n| {
                let name = n.to_string_lossy().to_lowercase();
                name == "makefile" || name == "dockerfile" || name == "readme"
            }).unwrap_or(false) {
                match index_single_file(rag, &path) {
                    Ok(chunks) => {
                        *total_files += 1;
                        *total_chunks += chunks;
                    }
                    Err(e) => errors.push(format!("{}: {}", path.display(), e)),
                }
            }
        }
    }
}
