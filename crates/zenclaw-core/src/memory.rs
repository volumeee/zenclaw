//! Memory store trait — conversation history & knowledge persistence.

use async_trait::async_trait;

use crate::error::Result;
use crate::message::ChatMessage;

/// Memory store trait — implement for different storage backends.
///
/// Provides conversation history and simple key-value storage.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Get conversation history for a session.
    async fn get_history(&self, session_key: &str, limit: usize) -> Result<Vec<ChatMessage>>;

    /// Save a conversation turn (user message + assistant response).
    async fn save_turn(
        &self,
        session_key: &str,
        user_message: &str,
        assistant_response: &str,
    ) -> Result<()>;

    /// Save a full chat message to history.
    async fn save_message(&self, session_key: &str, message: &ChatMessage) -> Result<()>;

    /// Clear history for a session.
    async fn clear_history(&self, session_key: &str) -> Result<()>;

    /// Store a fact/preference for later retrieval.
    async fn save_fact(&self, key: &str, value: &str) -> Result<()>;

    /// Retrieve a stored fact.
    async fn get_fact(&self, key: &str) -> Result<Option<String>>;

    /// Search facts by keyword.
    async fn search_facts(&self, query: &str, limit: usize) -> Result<Vec<(String, String)>>;
}

/// In-memory store for testing and lightweight usage.
pub struct InMemoryStore {
    history: std::sync::Mutex<std::collections::HashMap<String, Vec<ChatMessage>>>,
    facts: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            history: std::sync::Mutex::new(std::collections::HashMap::new()),
            facts: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn get_history(&self, session_key: &str, limit: usize) -> Result<Vec<ChatMessage>> {
        let store = self.history.lock().unwrap();
        let messages = store.get(session_key).cloned().unwrap_or_default();
        let start = messages.len().saturating_sub(limit);
        Ok(messages[start..].to_vec())
    }

    async fn save_turn(
        &self,
        session_key: &str,
        user_message: &str,
        assistant_response: &str,
    ) -> Result<()> {
        let mut store = self.history.lock().unwrap();
        let history = store.entry(session_key.to_string()).or_default();
        history.push(ChatMessage::user(user_message));
        history.push(ChatMessage::assistant(assistant_response));
        Ok(())
    }

    async fn save_message(&self, session_key: &str, message: &ChatMessage) -> Result<()> {
        let mut store = self.history.lock().unwrap();
        let history = store.entry(session_key.to_string()).or_default();
        history.push(message.clone());
        Ok(())
    }

    async fn clear_history(&self, session_key: &str) -> Result<()> {
        let mut store = self.history.lock().unwrap();
        store.remove(session_key);
        Ok(())
    }

    async fn save_fact(&self, key: &str, value: &str) -> Result<()> {
        let mut store = self.facts.lock().unwrap();
        store.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn get_fact(&self, key: &str) -> Result<Option<String>> {
        let store = self.facts.lock().unwrap();
        Ok(store.get(key).cloned())
    }

    async fn search_facts(&self, query: &str, limit: usize) -> Result<Vec<(String, String)>> {
        let store = self.facts.lock().unwrap();
        let query_lower = query.to_lowercase();
        let results: Vec<_> = store
            .iter()
            .filter(|(k, v)| {
                k.to_lowercase().contains(&query_lower)
                    || v.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(results)
    }
}
