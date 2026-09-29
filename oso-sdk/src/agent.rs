//! Agent client — SDK for agent-to-agent interactions.

use crate::error::SdkError;
use serde::{Deserialize, Serialize};

/// Agent capability advertisement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub agent_id: String,
    pub npub: Option<String>,
    pub tier: u8,
    pub skills: Vec<String>,
    pub is_online: bool,
}

/// Agent client — builds on AgentCard discovery and delegation.
pub struct AgentClient {
    pub agent_id: String,
    pub principal_id: String,
    known_agents: std::collections::HashMap<String, AgentCard>,
}

impl AgentClient {
    pub fn new(agent_id: impl Into<String>, principal_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            principal_id: principal_id.into(),
            known_agents: Default::default(),
        }
    }

    /// Register a discovered peer agent.
    pub fn register_peer(&mut self, card: AgentCard) {
        self.known_agents.insert(card.agent_id.clone(), card);
    }

    /// Find an online peer with a given skill.
    pub fn find_by_skill(&self, skill: &str) -> Vec<&AgentCard> {
        self.known_agents
            .values()
            .filter(|c| c.is_online && c.skills.iter().any(|s| s == skill))
            .collect()
    }

    /// Check if an agent is known and online.
    pub fn is_peer_online(&self, agent_id: &str) -> bool {
        self.known_agents.get(agent_id).is_some_and(|c| c.is_online)
    }
}

/// The full SDK handle combining job, contract, and agent clients.
pub struct AgentSdk {
    pub agent_id: String,
    pub principal_id: String,
}

impl AgentSdk {
    pub fn new(agent_id: impl Into<String>, principal_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            principal_id: principal_id.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_by_skill() {
        let mut client = AgentClient::new("did:v:a:1", "did:p:1");
        client.register_peer(AgentCard {
            agent_id: "did:v:a:2".into(),
            npub: None,
            tier: 2,
            skills: vec!["inference".into(), "splat".into()],
            is_online: true,
        });
        client.register_peer(AgentCard {
            agent_id: "did:v:a:3".into(),
            npub: None,
            tier: 1,
            skills: vec!["governance".into()],
            is_online: false,
        });
        let found = client.find_by_skill("inference");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].agent_id, "did:v:a:2");
    }

    #[test]
    fn offline_peer_not_returned() {
        let mut client = AgentClient::new("did:v:a:1", "did:p:1");
        client.register_peer(AgentCard {
            agent_id: "did:v:a:4".into(),
            npub: None,
            tier: 1,
            skills: vec!["inference".into()],
            is_online: false,
        });
        assert!(client.find_by_skill("inference").is_empty());
    }
}
