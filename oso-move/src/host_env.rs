//! MoveHostEnv — environment abstraction for Move codegen.
//!
//! The host environment answers questions that the compiler cannot resolve
//! statically: agent→address mappings, balances, and event emission sinks.

use std::collections::HashMap;

/// Runtime environment queries needed by Move codegen.
pub trait MoveHostEnv {
    /// Resolve a logical agent_id to a Move address (hex string, e.g. "0x…").
    /// Returns None if the agent is not known to this node.
    fn resolve_agent(&self, agent_id: &str) -> Option<String>;

    /// Return the current Àṣẹ balance (in mist) for the given agent.
    /// Returns 0 for unknown agents (safe default for stub generation).
    fn get_balance(&self, agent_id: &str) -> u64;

    /// Record an emitted event (kind + JSON payload string).
    /// In production this writes to the Nostr relay or Zàngbétò log.
    /// In tests it accumulates events for assertion.
    fn emit_event(&mut self, kind: &str, payload: &str);
}

// ---------------------------------------------------------------------------
// Mock implementation for testing and example generation
// ---------------------------------------------------------------------------

/// In-memory mock host environment.
///
/// Pre-loads a fixed agent→address table and balance map.
/// Emitted events are accumulated in `events` for test assertions.
pub struct MockMoveHostEnv {
    /// agent_id → Move address
    agents: HashMap<String, String>,
    /// agent_id → Àṣẹ balance (mist)
    balances: HashMap<String, u64>,
    /// Accumulated events: (kind, payload) pairs
    pub events: Vec<(String, String)>,
}

impl MockMoveHostEnv {
    /// Create an empty mock environment.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            balances: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Pre-register an agent address mapping.
    pub fn register_agent(&mut self, agent_id: impl Into<String>, address: impl Into<String>) {
        self.agents.insert(agent_id.into(), address.into());
    }

    /// Pre-set an agent balance.
    pub fn set_balance(&mut self, agent_id: impl Into<String>, balance: u64) {
        self.balances.insert(agent_id.into(), balance);
    }

    /// Return a default mock environment with a handful of known agents.
    pub fn with_defaults() -> Self {
        let mut env = Self::new();
        env.register_agent(
            "agent-treasury",
            "0x0000000000000000000000000000000000000001",
        );
        env.register_agent(
            "agent-council",
            "0x0000000000000000000000000000000000000002",
        );
        env.register_agent("agent-pool", "0x0000000000000000000000000000000000000003");
        env.set_balance("agent-treasury", 1_000_000_000_000);
        env.set_balance("agent-pool", 500_000_000_000);
        env
    }
}

impl Default for MockMoveHostEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveHostEnv for MockMoveHostEnv {
    fn resolve_agent(&self, agent_id: &str) -> Option<String> {
        self.agents.get(agent_id).cloned()
    }

    fn get_balance(&self, agent_id: &str) -> u64 {
        self.balances.get(agent_id).copied().unwrap_or(0)
    }

    fn emit_event(&mut self, kind: &str, payload: &str) {
        self.events.push((kind.to_string(), payload.to_string()));
    }
}
