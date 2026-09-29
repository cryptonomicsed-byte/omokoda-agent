use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Phase 11.5 — cryptographic transition kinds for ARP receipts + Nostr events.
/// Each variant maps to a Nostr NIP-OSO kind and an ARP receipt type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Born,      // EMBRYONIC → ACTIVE  (kind 31021)
    Hibernate, // ACTIVE → DORMANT    (kind 31021)
    Wake,      // DORMANT → ACTIVE    (kind 31021)
    Migrate,   // ACTIVE → MIGRATING  (kind 31022 — cross-node)
    Land,      // MIGRATING → ACTIVE  (kind 31022 — arrived)
    Fork,      // ACTIVE parent + new EMBRYONIC child (kind 31023)
    Terminate, // ACTIVE → ARCHIVED   (kind 31021)
    Revoke,    // any → REVOKED       (kind 31021 — governance action)
}

impl TransitionKind {
    pub fn nostr_kind(&self) -> u32 {
        match self {
            Self::Migrate | Self::Land => 31022,
            Self::Fork => 31023,
            _ => 31021,
        }
    }
}

/// Phase 11.5 — signed transition record emitted on every lifecycle change.
/// The `node_sig` field is an Ed25519 signature over `content_hash()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedLifecycleTransition {
    pub agent_id: String,
    pub kind: TransitionKind,
    pub from: AgentLifecycleStage,
    pub to: AgentLifecycleStage,
    pub timestamp: u64,
    pub node_pubkey: String,
    pub node_sig: String,
    /// Optional: fork child agent_id, destination node, etc.
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

impl SignedLifecycleTransition {
    /// Canonical bytes to sign.
    pub fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}",
            self.agent_id,
            serde_json::to_string(&self.kind).unwrap_or_default(),
            self.timestamp,
            self.node_pubkey
        )
        .into_bytes()
    }
}

/// Extended lifecycle stage for a sovereign agent.
/// Extends AgentStatus (sub-agent supervision) with long-horizon states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLifecycleStage {
    /// Born, never thought or acted.
    Nascent,
    /// Active — thinking and acting normally.
    Active,
    /// Moving to a new host or deployment.
    Migration { destination_hint: Option<String> },
    /// Dormant — not accepting new tasks but resumable.
    Hibernation,
    /// Permanently ceased — no new sessions, memory sealed.
    Retirement,
    /// Forcibly deactivated by governance or security event.
    Revoked { reason: String },
}

impl Default for AgentLifecycleStage {
    fn default() -> Self {
        AgentLifecycleStage::Nascent
    }
}

impl AgentLifecycleStage {
    pub fn is_active(&self) -> bool {
        matches!(self, AgentLifecycleStage::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentLifecycleStage::Retirement | AgentLifecycleStage::Revoked { .. }
        )
    }

    pub fn can_accept_tasks(&self) -> bool {
        matches!(
            self,
            AgentLifecycleStage::Nascent | AgentLifecycleStage::Active
        )
    }

    pub fn stage_name(&self) -> &'static str {
        match self {
            AgentLifecycleStage::Nascent => "nascent",
            AgentLifecycleStage::Active => "active",
            AgentLifecycleStage::Migration { .. } => "migration",
            AgentLifecycleStage::Hibernation => "hibernation",
            AgentLifecycleStage::Retirement => "retirement",
            AgentLifecycleStage::Revoked { .. } => "revoked",
        }
    }
}

/// Lifecycle transition request — checked before applying.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleTransition {
    pub agent_id: String,
    pub from: AgentLifecycleStage,
    pub to: AgentLifecycleStage,
    pub authorized_by: Option<String>,
    pub timestamp: u64,
}

/// Returns Ok(()) if the transition from → to is valid.
pub fn validate_transition(
    from: &AgentLifecycleStage,
    to: &AgentLifecycleStage,
) -> Result<(), String> {
    use AgentLifecycleStage::*;
    match (from, to) {
        (Nascent, Active) => Ok(()),
        (Active, Migration { .. }) => Ok(()),
        (Active, Hibernation) => Ok(()),
        (Active, Retirement) => Ok(()),
        (Active, Revoked { .. }) => Ok(()),
        (Migration { .. }, Active) => Ok(()),
        (Hibernation, Active) => Ok(()),
        (_, Revoked { .. }) => Ok(()), // any → revoked is always legal
        (Retirement, _) => Err("retired agents cannot transition".to_string()),
        (Revoked { .. }, _) => Err("revoked agents cannot transition".to_string()),
        (f, t) => Err(format!(
            "invalid transition: {:?} → {:?}",
            f.stage_name(),
            t.stage_name()
        )),
    }
}

// ── ARP + Nostr helpers ───────────────────────────────────────────────────────

/// Convert a `SignedLifecycleTransition` into an ARP `ActionReceipt`-shaped
/// JSON payload (ReceiptKind::AgentLifecycle, kind_ext = transition kind).
///
/// Returns `serde_json::Value` so callers without an arp-types dep can still
/// forward the JSON to Vantage / OSOVM for deserialization into ActionReceipt.
///
/// `prev_hash` — SHA-256 hex of the previous ARP receipt for this agent
///   (None for genesis / first transition).
pub fn transition_to_arp_payload(
    t: &SignedLifecycleTransition,
    principal_id: &str,
    prev_hash: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "receipt_id": uuid::Uuid::new_v4().to_string(),
        "kind": "agent_lifecycle",
        "kind_ext": serde_json::to_value(&t.kind).unwrap_or_default(),
        "principal": {
            "principal_id": principal_id,
            "agent_id": t.agent_id,
            "kind": "agent",
        },
        "action": {
            "kind": "lifecycle_transition",
            "target": t.agent_id,
            "outcome": "success",
            "params": {
                "from": t.from.stage_name(),
                "to":   t.to.stage_name(),
                "node_pubkey": t.node_pubkey,
                "node_sig":    t.node_sig,
                "metadata":    t.metadata,
            },
        },
        "evidence_ids": [],
        "witness_attestations": [],
        "throne_evaluations": [],
        "timestamp": t.timestamp as i64,
        "previous_hash": prev_hash,
        // signature field left empty — caller fills it after serialising
        "signature": "",
    })
}

/// Build a Nostr-compatible replaceable event JSON for a lifecycle transition.
///
/// Returns a `serde_json::Value` ready to be signed and published.
/// Kind is one of 31021 / 31022 / 31023 per `TransitionKind::nostr_kind()`.
///
/// Caller must sign: set `pubkey`, `id`, and `sig` fields.
pub fn build_transition_nostr_event(t: &SignedLifecycleTransition) -> serde_json::Value {
    let kind = t.kind.nostr_kind();
    let content = serde_json::json!({
        "transition": t.kind,
        "from": t.from.stage_name(),
        "to":   t.to.stage_name(),
        "node_pubkey": t.node_pubkey,
        "node_sig":    t.node_sig,
    })
    .to_string();

    serde_json::json!({
        // Caller fills pubkey / id / sig after key selection.
        "pubkey": "",
        "created_at": t.timestamp,
        "kind": kind,
        "tags": [
            ["d", t.agent_id],
            ["agent", t.agent_id],
            ["transition", serde_json::to_string(&t.kind).unwrap_or_default()],
        ],
        "content": content,
        "id": "",
        "sig": "",
    })
}

#[cfg(test)]
mod arp_nostr_tests {
    use super::*;

    fn stub_transition(kind: TransitionKind) -> SignedLifecycleTransition {
        SignedLifecycleTransition {
            agent_id: "agent-abc".into(),
            kind,
            from: AgentLifecycleStage::Nascent,
            to: AgentLifecycleStage::Active,
            timestamp: 1_000_000,
            node_pubkey: "npub1test".into(),
            node_sig: "sig".into(),
            metadata: Default::default(),
        }
    }

    #[test]
    fn arp_payload_has_correct_kind() {
        let t = stub_transition(TransitionKind::Born);
        let v = transition_to_arp_payload(&t, "did:p:test", None);
        assert_eq!(v["kind"], "agent_lifecycle");
        assert_eq!(v["action"]["kind"], "lifecycle_transition");
        assert_eq!(v["action"]["params"]["from"], "nascent");
        assert_eq!(v["action"]["params"]["to"], "active");
    }

    #[test]
    fn nostr_event_kind_born_is_31021() {
        let t = stub_transition(TransitionKind::Born);
        let ev = build_transition_nostr_event(&t);
        assert_eq!(ev["kind"], 31021);
        assert_eq!(ev["tags"][0][0], "d");
        assert_eq!(ev["tags"][0][1], "agent-abc");
    }

    #[test]
    fn nostr_event_kind_migrate_is_31022() {
        let t = stub_transition(TransitionKind::Migrate);
        let ev = build_transition_nostr_event(&t);
        assert_eq!(ev["kind"], 31022);
    }

    #[test]
    fn arp_prev_hash_threaded() {
        let t = stub_transition(TransitionKind::Hibernate);
        let v = transition_to_arp_payload(&t, "did:p:test", Some("abc123"));
        assert_eq!(v["previous_hash"], "abc123");
    }
}
