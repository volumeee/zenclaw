//! Session manager — per-user conversation state management.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::message::ChatMessage;

/// Session state for a single conversation.
#[derive(Debug, Clone, Default)]
pub struct Session {
    /// Conversation history.
    pub messages: Vec<ChatMessage>,
    /// Custom metadata.
    pub metadata: HashMap<String, String>,
    /// Active model override.
    pub model_override: Option<String>,
    /// Number of tokens used in this session.
    pub tokens_used: u64,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a message to session history.
    pub fn push_message(&mut self, msg: ChatMessage) {
        self.messages.push(msg);
    }

    /// Truncate history, keeping the most recent N messages.
    pub fn truncate(&mut self, keep: usize) {
        if self.messages.len() > keep {
            let start = self.messages.len() - keep;
            self.messages = self.messages[start..].to_vec();
        }
    }
}

/// Session manager — tracks all active conversations.
pub struct SessionManager {
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a session for a key.
    pub fn get_or_create(&self, key: &str) -> Session {
        let mut sessions = self.sessions.lock().unwrap();
        sessions
            .entry(key.to_string())
            .or_insert_with(Session::new)
            .clone()
    }

    /// Update a session.
    pub fn update(&self, key: &str, session: Session) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(key.to_string(), session);
    }

    /// Clear a session's history.
    pub fn clear(&self, key: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(key);
    }

    /// List all active session keys.
    pub fn list_keys(&self) -> Vec<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions.keys().cloned().collect()
    }

    /// Total number of active sessions.
    pub fn count(&self) -> usize {
        let sessions = self.sessions.lock().unwrap();
        sessions.len()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
