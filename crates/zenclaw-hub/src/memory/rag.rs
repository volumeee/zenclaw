//! RAG (Retrieval-Augmented Generation) via SQLite FTS5.
//!
//! Provides full-text search across conversation history and documents.
//! Uses SQLite FTS5 — no external vector DB needed, keeps the binary tiny.

use rusqlite::{params, Connection};
use std::path::Path;

use zenclaw_core::error::{Result, ZenClawError};

/// A searchable document chunk.
#[derive(Debug, Clone)]
pub struct Document {
    pub id: i64,
    pub source: String,
    pub content: String,
    pub metadata: String,
    pub rank: f64,
}

/// RAG store — full-text search powered by SQLite FTS5.
pub struct RagStore {
    conn: Connection,
}

impl RagStore {
    /// Open or create a RAG store.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| ZenClawError::Memory(format!("RAG DB open failed: {}", e)))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                metadata TEXT DEFAULT '',
                created_at TEXT DEFAULT (datetime('now'))
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
                content,
                source,
                metadata,
                content='documents',
                content_rowid='id'
            );

            -- Trigger to keep FTS in sync
            CREATE TRIGGER IF NOT EXISTS documents_ai AFTER INSERT ON documents BEGIN
                INSERT INTO documents_fts(rowid, content, source, metadata)
                VALUES (new.id, new.content, new.source, new.metadata);
            END;

            CREATE TRIGGER IF NOT EXISTS documents_ad AFTER DELETE ON documents BEGIN
                INSERT INTO documents_fts(documents_fts, rowid, content, source, metadata)
                VALUES ('delete', old.id, old.content, old.source, old.metadata);
            END;

            CREATE TRIGGER IF NOT EXISTS documents_au AFTER UPDATE ON documents BEGIN
                INSERT INTO documents_fts(documents_fts, rowid, content, source, metadata)
                VALUES ('delete', old.id, old.content, old.source, old.metadata);
                INSERT INTO documents_fts(rowid, content, source, metadata)
                VALUES (new.id, new.content, new.source, new.metadata);
            END;",
        )
        .map_err(|e| ZenClawError::Memory(format!("RAG schema creation failed: {}", e)))?;

        Ok(Self { conn })
    }

    /// Index a document for search.
    pub fn index(&self, source: &str, content: &str, metadata: &str) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO documents (source, content, metadata) VALUES (?1, ?2, ?3)",
                params![source, content, metadata],
            )
            .map_err(|e| ZenClawError::Memory(format!("RAG index failed: {}", e)))?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Index a long text by splitting into chunks.
    pub fn index_chunked(
        &self,
        source: &str,
        text: &str,
        chunk_size: usize,
        overlap: usize,
    ) -> Result<Vec<i64>> {
        let chunks = chunk_text(text, chunk_size, overlap);
        let mut ids = Vec::new();

        for (i, chunk) in chunks.iter().enumerate() {
            let meta = format!("chunk:{}/{}", i + 1, chunks.len());
            let id = self.index(source, chunk, &meta)?;
            ids.push(id);
        }

        Ok(ids)
    }

    /// Search for relevant documents.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Document>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT d.id, d.source, d.content, d.metadata, rank
                 FROM documents_fts
                 JOIN documents d ON d.id = documents_fts.rowid
                 WHERE documents_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )
            .map_err(|e| ZenClawError::Memory(format!("RAG search prepare failed: {}", e)))?;

        let results = stmt
            .query_map(params![query, limit as i64], |row| {
                Ok(Document {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    content: row.get(2)?,
                    metadata: row.get(3)?,
                    rank: row.get(4)?,
                })
            })
            .map_err(|e| ZenClawError::Memory(format!("RAG search failed: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    /// Get document count.
    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
            .map_err(|e| ZenClawError::Memory(format!("RAG count failed: {}", e)))?;
        Ok(count as usize)
    }

    /// Delete documents by source.
    pub fn delete_by_source(&self, source: &str) -> Result<usize> {
        let deleted = self
            .conn
            .execute("DELETE FROM documents WHERE source = ?1", params![source])
            .map_err(|e| ZenClawError::Memory(format!("RAG delete failed: {}", e)))?;
        Ok(deleted)
    }

    /// Build a RAG context string from search results.
    pub fn build_context(&self, query: &str, max_results: usize) -> Result<String> {
        let results = self.search(query, max_results)?;

        if results.is_empty() {
            return Ok(String::new());
        }

        let mut context = String::from("## Relevant Context\n\n");
        for (i, doc) in results.iter().enumerate() {
            context.push_str(&format!(
                "### Source {}: {}\n{}\n\n",
                i + 1,
                doc.source,
                doc.content
            ));
        }

        Ok(context)
    }
}

/// Split text into overlapping chunks for better search coverage.
fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();

    if words.len() <= chunk_size {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let step = if chunk_size > overlap {
        chunk_size - overlap
    } else {
        1
    };

    let mut i = 0;
    while i < words.len() {
        let end = (i + chunk_size).min(words.len());
        let chunk = words[i..end].join(" ");
        chunks.push(chunk);

        if end >= words.len() {
            break;
        }
        i += step;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_chunk_text() {
        let text = "one two three four five six seven eight nine ten";
        let chunks = chunk_text(text, 4, 1);
        assert!(chunks.len() >= 3);
        assert!(chunks[0].contains("one"));
    }

    #[test]
    fn test_rag_store() {
        let tmp = NamedTempFile::new().unwrap();
        let store = RagStore::open(tmp.path()).unwrap();

        store.index("test.md", "Rust is a systems programming language", "").unwrap();
        store.index("test.md", "ZenClaw is an AI agent framework", "").unwrap();

        let results = store.search("AI agent", 5).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].content.contains("AI agent") || results[0].content.contains("ZenClaw"));

        assert_eq!(store.count().unwrap(), 2);
    }
}
