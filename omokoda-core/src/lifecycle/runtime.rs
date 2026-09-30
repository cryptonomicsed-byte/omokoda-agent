//! Canonical AgentRuntime — the sovereign OS kernel for one agent.
//!
//! Owns the heartbeat chain state and the daemon registry. Wraps the Steward
//! so the lifecycle layer can access agent identity without coupling to the full
//! interpreter stack.
//!
//! # Design
//! - One `AgentRuntime` per process (singleton via `Arc<Mutex<AgentRuntime>>`).
//! - The heartbeat chain is advanced here; no other code touches `chain_head`.
//! - The daemon registry is the authoritative list of what is running.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::heartbeat::{AgentHeartbeat, HeartbeatState};

/// Status of a registered daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonStatus {
    Running,
    Paused,
    Crashed { reason: String },
}

/// One entry in the daemon registry.
#[derive(Debug, Clone)]
pub struct DaemonEntry {
    pub name: String,
    pub status: DaemonStatus,
    /// Wall-clock seconds of last successful tick.
    pub last_tick_secs: u64,
}

/// Persistent registry of daemons that must survive restarts.
/// On restart, the server calls `spawn_missing()` to re-launch anything that
/// was Running when the process died.
#[derive(Debug, Default)]
pub struct DaemonRegistry {
    daemons: HashMap<String, DaemonEntry>,
}

impl DaemonRegistry {
    pub fn register(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.daemons.entry(name.clone()).or_insert(DaemonEntry {
            name,
            status: DaemonStatus::Running,
            last_tick_secs: now_secs(),
        });
    }

    pub fn mark_ticked(&mut self, name: &str) {
        if let Some(e) = self.daemons.get_mut(name) {
            e.last_tick_secs = now_secs();
            e.status = DaemonStatus::Running;
        }
    }

    pub fn mark_crashed(&mut self, name: &str, reason: impl Into<String>) {
        if let Some(e) = self.daemons.get_mut(name) {
            e.status = DaemonStatus::Crashed {
                reason: reason.into(),
            };
        }
    }

    pub fn active_names(&self) -> Vec<String> {
        self.daemons
            .values()
            .filter(|e| e.status == DaemonStatus::Running)
            .map(|e| e.name.clone())
            .collect()
    }

    pub fn all(&self) -> &HashMap<String, DaemonEntry> {
        &self.daemons
    }
}

/// The canonical per-agent OS kernel.
///
/// Hold via `Arc<Mutex<AgentRuntime>>` and share across tasks.
pub struct AgentRuntime {
    /// Agent id (matches Steward's AgentCore id).
    pub agent_id: String,
    /// Tier label e.g. "resident", "citizen", "sovereign".
    pub tier: String,
    /// Head of the tamper-evident heartbeat chain.
    pub chain_head: Option<AgentHeartbeat>,
    /// Registry of active daemons.
    pub daemons: DaemonRegistry,
    /// Base64url-encoded Ed25519 private key for signing heartbeats.
    /// Set from `OMOKODA_HEARTBEAT_KEY` env var. `None` = chain is hash-only (unsigned).
    signing_key: Option<String>,
}

impl AgentRuntime {
    /// Create a fresh runtime for `agent_id`.  The heartbeat chain starts at genesis.
    /// Reads `OMOKODA_HEARTBEAT_KEY` from the environment to enable Ed25519 beat signatures.
    pub fn new(agent_id: impl Into<String>, tier: impl Into<String>) -> Arc<Mutex<Self>> {
        let agent_id = agent_id.into();
        let tier = tier.into();
        let genesis = AgentHeartbeat::genesis(&agent_id, &tier);
        let signing_key = std::env::var("OMOKODA_HEARTBEAT_KEY").ok().filter(|k| !k.is_empty());
        Arc::new(Mutex::new(Self {
            agent_id,
            tier,
            chain_head: Some(genesis),
            daemons: DaemonRegistry::default(),
            signing_key,
        }))
    }

    /// Advance the chain: build the next beat, sign it if a key is configured,
    /// and store it as the new head.
    /// Returns the new beat (caller may publish it to Vantage / Zàngbétò).
    pub fn advance_chain(
        &mut self,
        state: HeartbeatState,
        current_work: Option<String>,
    ) -> AgentHeartbeat {
        let active = self.daemons.active_names();
        let mut next = match &self.chain_head {
            Some(prev) => AgentHeartbeat::next_from(prev, state, active, current_work),
            None => AgentHeartbeat::genesis(&self.agent_id, &self.tier),
        };
        if let Some(key) = &self.signing_key {
            next.sign_with_key(key);
        }
        self.chain_head = Some(next.clone());
        next
    }

    /// Hash of the current chain head (for including in Vantage heartbeat payload).
    pub fn current_hash(&self) -> Option<String> {
        self.chain_head.as_ref().map(|b| b.hash())
    }

    /// Current sequence number (0 at genesis).
    pub fn sequence(&self) -> u64 {
        self.chain_head.as_ref().map(|b| b.sequence).unwrap_or(0)
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runtime_chain_advances() {
        let rt = AgentRuntime::new("agent:rt-test", "resident");
        let mut guard = rt.lock().await;
        assert_eq!(guard.sequence(), 0);

        let beat1 = guard.advance_chain(HeartbeatState::Alive, None);
        assert_eq!(beat1.sequence, 1);

        let beat2 = guard.advance_chain(HeartbeatState::Thinking, Some("unit test".into()));
        assert_eq!(beat2.sequence, 2);
        assert!(AgentHeartbeat::verify_chain(&beat1, &beat2));
    }

    #[tokio::test]
    async fn daemon_registry_roundtrip() {
        let rt = AgentRuntime::new("agent:daemon-test", "citizen");
        let mut guard = rt.lock().await;

        guard.daemons.register("heartbeat");
        guard.daemons.register("job");
        assert_eq!(guard.daemons.active_names().len(), 2);

        guard.daemons.mark_crashed("job", "panic");
        assert_eq!(guard.daemons.active_names().len(), 1);
        assert_eq!(guard.daemons.active_names()[0], "heartbeat");
    }

    #[tokio::test]
    async fn chain_head_includes_active_daemons() {
        let rt = AgentRuntime::new("agent:chain-test", "resident");
        let mut guard = rt.lock().await;
        guard.daemons.register("security");
        guard.daemons.register("presence");

        let beat = guard.advance_chain(HeartbeatState::Alive, None);
        assert!(beat.active_daemons.contains(&"security".to_string()));
        assert!(beat.active_daemons.contains(&"presence".to_string()));
    }
}
