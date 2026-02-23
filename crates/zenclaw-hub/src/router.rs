//! Multi-agent coordination — route tasks to specialized agents.
//!
//! The AgentRouter dispatches messages to specialized sub-agents
//! based on intent detection. Each agent can have different skills,
//! tools, and models optimized for their domain.

use tracing::info;

use zenclaw_core::agent::Agent;
use zenclaw_core::error::{Result, ZenClawError};
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::LlmProvider;

/// A named agent with routing rules.
pub struct AgentSlot {
    /// Agent name.
    pub name: String,
    /// Description of what this agent handles.
    pub description: String,
    /// Keywords that trigger routing to this agent.
    pub keywords: Vec<String>,
    /// The agent instance.
    pub agent: Agent,
}

/// Multi-agent router — routes messages to the best agent.
pub struct AgentRouter {
    agents: Vec<AgentSlot>,
    default_agent: Option<String>,
}

impl AgentRouter {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            default_agent: None,
        }
    }

    /// Register an agent with routing keywords.
    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        keywords: Vec<String>,
        agent: Agent,
    ) {
        self.agents.push(AgentSlot {
            name: name.to_string(),
            description: description.to_string(),
            keywords,
            agent,
        });
    }

    /// Set the default agent (used when no keywords match).
    pub fn set_default(&mut self, name: &str) {
        self.default_agent = Some(name.to_string());
    }

    /// Route a message to the best agent based on keywords.
    pub fn route(&self, message: &str) -> &AgentSlot {
        let msg_lower = message.to_lowercase();

        // Score each agent based on keyword matches
        let mut best_score = 0;
        let mut best_agent: Option<&AgentSlot> = None;

        for slot in &self.agents {
            let score: usize = slot
                .keywords
                .iter()
                .filter(|kw| msg_lower.contains(&kw.to_lowercase()))
                .count();

            if score > best_score {
                best_score = score;
                best_agent = Some(slot);
            }
        }

        // Return matched agent or default
        if let Some(agent) = best_agent {
            return agent;
        }

        // Fall back to default or first agent
        if let Some(ref default_name) = self.default_agent {
            if let Some(slot) = self.agents.iter().find(|s| s.name == *default_name) {
                return slot;
            }
        }

        // Absolute fallback — first registered agent
        &self.agents[0]
    }

    /// Process a message through the router.
    pub async fn process(
        &self,
        provider: &dyn LlmProvider,
        memory: &dyn MemoryStore,
        message: &str,
        session_key: &str,
    ) -> Result<(String, String)> {
        if self.agents.is_empty() {
            return Err(ZenClawError::Other(
                "No agents registered in router".to_string(),
            ));
        }

        let slot = self.route(message);
        info!("🔀 Routing to agent: {} — {}", slot.name, slot.description);

        let response = slot
            .agent
            .process(provider, memory, message, session_key)
            .await?;

        Ok((slot.name.clone(), response))
    }

    /// List all registered agents.
    pub fn list(&self) -> Vec<(&str, &str)> {
        self.agents
            .iter()
            .map(|s| (s.name.as_str(), s.description.as_str()))
            .collect()
    }

    /// Get an agent by name.
    pub fn get(&self, name: &str) -> Option<&AgentSlot> {
        self.agents.iter().find(|s| s.name == name)
    }

    /// Number of registered agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

impl Default for AgentRouter {
    fn default() -> Self {
        Self::new()
    }
}
