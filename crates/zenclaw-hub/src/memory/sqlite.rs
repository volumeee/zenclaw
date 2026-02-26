//! SQLite-backed persistent memory store.

use async_trait::async_trait;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::message::ChatMessage;

/// SQLite memory store — persistent conversation history & facts.
///
/// Perfect for STB and embedded Linux — tiny footprint, no external services.
pub struct SqliteMemory {
    conn: Mutex<Connection>,
    rag: Option<crate::memory::RagStore>,
}

impl SqliteMemory {
    /// Open or create a SQLite database.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| ZenClawError::Memory(format!("SQLite open error: {}", e)))?;

        // Create tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_key TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                tool_calls TEXT,
                tool_call_id TEXT,
                name TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_history_session ON history(session_key);

            CREATE TABLE IF NOT EXISTS facts (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .map_err(|e| ZenClawError::Memory(format!("SQLite init error: {}", e)))?;

        let rag = path.parent().map(|p| p.join("rag.db")).and_then(|p| crate::memory::RagStore::open(&p).ok());

        Ok(Self {
            conn: Mutex::new(conn),
            rag,
        })
    }

    /// Create an in-memory SQLite database (for testing).
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| ZenClawError::Memory(format!("SQLite error: {}", e)))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_key TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                tool_calls TEXT,
                tool_call_id TEXT,
                name TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_history_session ON history(session_key);

            CREATE TABLE IF NOT EXISTS facts (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .map_err(|e| ZenClawError::Memory(format!("SQLite init error: {}", e)))?;

        let rag = crate::memory::RagStore::in_memory().ok();

        Ok(Self {
            conn: Mutex::new(conn),
            rag,
        })
    }
}

#[async_trait]
impl MemoryStore for SqliteMemory {
    async fn get_history(&self, session_key: &str, limit: usize) -> Result<Vec<ChatMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT role, content, tool_calls, tool_call_id, name
                 FROM history WHERE session_key = ?1
                 ORDER BY id DESC LIMIT ?2",
            )
            .map_err(|e| ZenClawError::Memory(e.to_string()))?;

        let rows = stmt
            .query_map(rusqlite::params![session_key, limit], |row| {
                let role: String = row.get(0)?;
                let content: Option<String> = row.get(1)?;
                let tool_calls_json: Option<String> = row.get(2)?;
                let tool_call_id: Option<String> = row.get(3)?;
                let name: Option<String> = row.get(4)?;

                let role_enum = match role.as_str() {
                    "system" => zenclaw_core::message::Role::System,
                    "user" => zenclaw_core::message::Role::User,
                    "assistant" => zenclaw_core::message::Role::Assistant,
                    "tool" => zenclaw_core::message::Role::Tool,
                    _ => zenclaw_core::message::Role::User,
                };

                let tool_calls = tool_calls_json
                    .and_then(|j| serde_json::from_str(&j).ok());

                Ok(ChatMessage {
                    role: role_enum,
                    content,
                    media: Vec::new(),
                    tool_calls,
                    tool_call_id,
                    name,
                })
            })
            .map_err(|e| ZenClawError::Memory(e.to_string()))?;

        let mut messages: Vec<ChatMessage> = rows
            .filter_map(|r| r.ok())
            .collect();

        // Reverse to get chronological order
        messages.reverse();
        Ok(messages)
    }

    async fn save_turn(
        &self,
        session_key: &str,
        user_message: &str,
        assistant_response: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO history (session_key, role, content) VALUES (?1, 'user', ?2)",
            rusqlite::params![session_key, user_message],
        )
        .map_err(|e| ZenClawError::Memory(e.to_string()))?;

        conn.execute(
            "INSERT INTO history (session_key, role, content) VALUES (?1, 'assistant', ?2)",
            rusqlite::params![session_key, assistant_response],
        )
        .map_err(|e| ZenClawError::Memory(e.to_string()))?;

        Ok(())
    }

    async fn save_message(&self, session_key: &str, message: &ChatMessage) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let role = format!("{:?}", message.role).to_lowercase();
        let tool_calls_json = message
            .tool_calls
            .as_ref()
            .map(|tc| serde_json::to_string(tc).unwrap_or_default());

        conn.execute(
            "INSERT INTO history (session_key, role, content, tool_calls, tool_call_id, name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                session_key,
                role,
                message.content,
                tool_calls_json,
                message.tool_call_id,
                message.name,
            ],
        )
        .map_err(|e| ZenClawError::Memory(e.to_string()))?;

        Ok(())
    }

    async fn clear_history(&self, session_key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM history WHERE session_key = ?1",
            rusqlite::params![session_key],
        )
        .map_err(|e| ZenClawError::Memory(e.to_string()))?;
        Ok(())
    }

    async fn save_fact(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO facts (key, value, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)",
            rusqlite::params![key, value],
        )
        .map_err(|e| ZenClawError::Memory(e.to_string()))?;
        Ok(())
    }

    async fn get_fact(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT value FROM facts WHERE key = ?1",
                rusqlite::params![key],
                |row| row.get(0),
            )
            .ok();
        Ok(result)
    }

    async fn search_facts(&self, query: &str, limit: usize) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query);
        let mut stmt = conn
            .prepare(
                "SELECT key, value FROM facts
                 WHERE key LIKE ?1 OR value LIKE ?1
                 LIMIT ?2",
            )
            .map_err(|e| ZenClawError::Memory(e.to_string()))?;

        let results = stmt
            .query_map(rusqlite::params![pattern, limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| ZenClawError::Memory(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    async fn search_knowledge(&self, query: &str, limit: usize) -> Result<Option<String>> {
        if let Some(rag) = &self.rag {
            let context = rag.build_context(query, limit)?;
            if context.is_empty() {
                Ok(None)
            } else {
                Ok(Some(context))
            }
        } else {
            Ok(None)
        }
    }
}
