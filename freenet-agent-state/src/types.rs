/// Shared types for the Freenet agent state contract.
///
/// These are the ONLY fields that belong in Freenet (public, non-canonical,
/// non-financial state).  Canonical state lives on Ọ̀ṢỌ́ L1.  Private state
/// lives in Walrus/Seal.
use serde::{Deserialize, Serialize};

/// The agent's mutable public state replicated across the Freenet network.
/// Keyed by `agent_npub` (Nostr hex pubkey — also the Freenet contract key).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPublicState {
    /// Nostr hex pubkey — immutable primary key.
    pub agent_npub: String,
    /// Human-readable display name (mutable).
    pub display_name: String,
    /// Short bio / purpose statement.
    pub bio: String,
    /// Preferred Nostr relay list.
    pub relay_list: Vec<String>,
    /// Agent tier (T0–T5) — informational, not enforced here.
    pub tier: u8,
    /// Unix seconds of the most recent activity the agent chose to broadcast.
    pub last_seen: u64,
    /// Capabilities the agent has declared publicly.
    pub declared_capabilities: Vec<String>,
    /// Agent's current lifecycle stage (born/active/hibernating/migrating).
    pub lifecycle_stage: String,
    /// Optional link to the agent's Sui dNFT object ID.
    pub sui_object_id: Option<String>,
    /// BIPON-39 mnemonic fingerprint (first 4 words only — not the full secret).
    pub bipon39_hint: Option<String>,
    /// Monotonically increasing version counter — used for merge conflict resolution.
    pub version: u64,
    /// Unix seconds when this state was last signed by the agent.
    pub updated_at: u64,
    /// Hex-encoded Ed25519 signature of the canonical state hash by agent_npub key.
    pub signature: String,
}

impl AgentPublicState {
    /// Produce the canonical bytes that must be signed by the agent key.
    /// Covers all mutable fields except `signature` itself.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let canonical = serde_json::json!({
            "agent_npub":            &self.agent_npub,
            "display_name":          &self.display_name,
            "bio":                   &self.bio,
            "relay_list":            &self.relay_list,
            "tier":                  self.tier,
            "last_seen":             self.last_seen,
            "declared_capabilities": &self.declared_capabilities,
            "lifecycle_stage":       &self.lifecycle_stage,
            "sui_object_id":         &self.sui_object_id,
            "bipon39_hint":          &self.bipon39_hint,
            "version":               self.version,
            "updated_at":            self.updated_at,
        });
        serde_json::to_vec(&canonical).unwrap_or_default()
    }

    /// BLAKE3 of the signable bytes — used as the Freenet state commitment.
    pub fn state_hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }
}

/// An incremental update to apply on top of an existing `AgentPublicState`.
/// Only fields present (Some) are applied; None fields are left unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDelta {
    pub agent_npub: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub relay_list: Option<Vec<String>>,
    pub tier: Option<u8>,
    pub last_seen: Option<u64>,
    pub declared_capabilities: Option<Vec<String>>,
    pub lifecycle_stage: Option<String>,
    pub sui_object_id: Option<Option<String>>,
    pub bipon39_hint: Option<Option<String>>,
    /// Must be exactly `prev_version + 1` for the delta to be accepted.
    pub new_version: u64,
    pub updated_at: u64,
    /// Ed25519 signature of the delta's canonical bytes by agent_npub key.
    pub signature: String,
}

impl StateDelta {
    /// Canonical bytes that the agent must sign when creating a delta.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let canonical = serde_json::json!({
            "agent_npub":    &self.agent_npub,
            "new_version":   self.new_version,
            "updated_at":    self.updated_at,
            "display_name":  &self.display_name,
            "bio":           &self.bio,
            "relay_list":    &self.relay_list,
            "tier":          self.tier,
            "last_seen":     self.last_seen,
            "declared_capabilities": &self.declared_capabilities,
            "lifecycle_stage": &self.lifecycle_stage,
        });
        serde_json::to_vec(&canonical).unwrap_or_default()
    }
}

/// Compact summary produced by `summarize()` — Freenet uses this for
/// efficient state sync between relay nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSummary {
    pub agent_npub: String,
    pub version: u64,
    pub state_hash: String, // hex
    pub last_seen: u64,
    pub lifecycle_stage: String,
}

/// Errors from contract validation.
#[derive(Debug, Serialize, Deserialize)]
pub enum ContractError {
    InvalidNpub,
    VersionMismatch { expected: u64, got: u64 },
    SignatureInvalid,
    NpubMismatch,
    Deserialize(String),
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> AgentPublicState {
        AgentPublicState {
            agent_npub: "abc123".into(),
            display_name: "TestAgent".into(),
            bio: "A sovereign agent".into(),
            relay_list: vec!["wss://relay.example".into()],
            tier: 2,
            last_seen: 1_700_000_000,
            declared_capabilities: vec!["think".into(), "act".into()],
            lifecycle_stage: "active".into(),
            sui_object_id: None,
            bipon39_hint: Some("flame river oak".into()),
            version: 1,
            updated_at: 1_700_000_000,
            signature: String::new(),
        }
    }

    #[test]
    fn state_hash_is_deterministic() {
        let s = sample_state();
        assert_eq!(s.state_hash(), s.state_hash());
    }

    #[test]
    fn state_hash_changes_with_mutation() {
        let mut s = sample_state();
        let h1 = s.state_hash();
        s.display_name = "Changed".into();
        let h2 = s.state_hash();
        assert_ne!(h1, h2);
    }

    #[test]
    fn state_roundtrips_json() {
        let s = sample_state();
        let json = serde_json::to_string(&s).unwrap();
        let back: AgentPublicState = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
