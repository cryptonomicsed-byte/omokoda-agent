/// Phase 17.2 — Freenet → L1 Bridge (state commitment finalization)
///
/// Defines the `StateCommitmentTx` format and the logic for deciding when
/// a Freenet-replicated agent state change warrants an L1 anchoring record.
///
/// NOT every profile update hits L1 — only significant transitions:
///   - Tier change
///   - Lifecycle stage change (born → active → hibernating → migrating)
///   - New capability declared
///   - Reputation crossing a threshold
///   - Version milestone (every 10th version, or explicit request)
///
/// The current submission path (Phase 17.2) routes via `onchain.rs` which
/// shells out to the `sui` CLI.  When Phase 15 (Ọ̀ṢỌ́ ABCI L1) is complete,
/// `submit_to_abci()` becomes the real path and the Sui call becomes the
/// fallback/migration path.

use serde::{Deserialize, Serialize};

/// A commitment from Ọmọ Kọ́dà to the L1 that an agent's Freenet public
/// state has advanced to a new canonical hash.  This is the wire format
/// sent to the L1 (currently via Sui CLI; eventually ABCI).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateCommitmentTx {
    /// Agent identifier (Nostr hex pubkey).
    pub agent_npub: String,
    /// BLAKE3 hex of the new `AgentPublicState` after the transition.
    pub freenet_state_hash: String,
    /// Version of the `AgentPublicState` this hash corresponds to.
    pub state_version: u64,
    /// Optional ARP receipt ID anchoring the work that caused this transition.
    pub arp_receipt_id: Option<String>,
    /// Unix seconds when this commitment was constructed.
    pub committed_at: u64,
    /// Human-readable reason — not stored on-chain, only in local ledger.
    pub reason: CommitmentReason,
}

/// Why we are committing this state to L1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentReason {
    TierChange { from: u8, to: u8 },
    LifecycleChange { from: String, to: String },
    CapabilityAdded { capability: String },
    VersionMilestone { version: u64 },
    ExplicitRequest,
}

impl std::fmt::Display for CommitmentReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TierChange { from, to } => write!(f, "tier T{from}→T{to}"),
            Self::LifecycleChange { from, to } => write!(f, "lifecycle {from}→{to}"),
            Self::CapabilityAdded { capability } => write!(f, "capability +{capability}"),
            Self::VersionMilestone { version } => write!(f, "version milestone v{version}"),
            Self::ExplicitRequest => write!(f, "explicit request"),
        }
    }
}

/// Inspect a state transition and return the `CommitmentReason` if it is
/// significant enough to warrant an L1 anchoring record.
///
/// Called after `apply_delta()` with the old and new `AgentPublicState`
/// fields we care about.
pub fn is_significant_transition(
    old_tier: u8,
    new_tier: u8,
    old_lifecycle: &str,
    new_lifecycle: &str,
    old_capabilities: &[String],
    new_capabilities: &[String],
    new_version: u64,
) -> Option<CommitmentReason> {
    if new_tier != old_tier {
        return Some(CommitmentReason::TierChange {
            from: old_tier,
            to: new_tier,
        });
    }
    if new_lifecycle != old_lifecycle {
        return Some(CommitmentReason::LifecycleChange {
            from: old_lifecycle.to_string(),
            to: new_lifecycle.to_string(),
        });
    }
    let added: Vec<_> = new_capabilities
        .iter()
        .filter(|c| !old_capabilities.contains(c))
        .collect();
    if let Some(cap) = added.first() {
        return Some(CommitmentReason::CapabilityAdded {
            capability: cap.to_string(),
        });
    }
    if new_version > 0 && new_version % 10 == 0 {
        return Some(CommitmentReason::VersionMilestone {
            version: new_version,
        });
    }
    None
}

/// Build a `StateCommitmentTx` for a significant state transition.
pub fn build_commitment(
    agent_npub: &str,
    freenet_state_hash: &str,
    state_version: u64,
    arp_receipt_id: Option<String>,
    reason: CommitmentReason,
) -> StateCommitmentTx {
    let committed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    StateCommitmentTx {
        agent_npub: agent_npub.to_string(),
        freenet_state_hash: freenet_state_hash.to_string(),
        state_version,
        arp_receipt_id,
        committed_at,
        reason,
    }
}

/// ABCI submission path (Phase 15 stub).
/// When Phase 15 is complete, this replaces the Sui call as the primary path.
pub async fn submit_to_abci(_tx: &StateCommitmentTx) -> Result<String, String> {
    Err("ABCI L1 not yet deployed (Phase 15 pending)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_change_is_significant() {
        let reason = is_significant_transition(1, 2, "active", "active", &[], &[], 5);
        assert!(matches!(reason, Some(CommitmentReason::TierChange { from: 1, to: 2 })));
    }

    #[test]
    fn lifecycle_change_is_significant() {
        let reason = is_significant_transition(1, 1, "active", "hibernating", &[], &[], 5);
        assert!(matches!(reason, Some(CommitmentReason::LifecycleChange { .. })));
    }

    #[test]
    fn new_capability_is_significant() {
        let old = vec!["think".to_string()];
        let new = vec!["think".to_string(), "forge".to_string()];
        let reason = is_significant_transition(1, 1, "active", "active", &old, &new, 5);
        assert!(matches!(reason, Some(CommitmentReason::CapabilityAdded { .. })));
    }

    #[test]
    fn version_milestone_every_10() {
        let reason = is_significant_transition(1, 1, "active", "active", &[], &[], 10);
        assert!(matches!(reason, Some(CommitmentReason::VersionMilestone { version: 10 })));
    }

    #[test]
    fn non_milestone_version_not_significant() {
        let reason = is_significant_transition(1, 1, "active", "active", &[], &[], 7);
        assert!(reason.is_none());
    }

    #[test]
    fn build_commitment_fields() {
        let tx = build_commitment(
            "npub1abc",
            "deadbeef",
            10,
            None,
            CommitmentReason::VersionMilestone { version: 10 },
        );
        assert_eq!(tx.agent_npub, "npub1abc");
        assert_eq!(tx.freenet_state_hash, "deadbeef");
        assert_eq!(tx.state_version, 10);
        assert!(tx.committed_at > 0);
    }

    #[test]
    fn commitment_reason_display() {
        assert_eq!(
            CommitmentReason::TierChange { from: 1, to: 2 }.to_string(),
            "tier T1→T2"
        );
        assert_eq!(
            CommitmentReason::LifecycleChange {
                from: "active".into(),
                to: "hibernating".into()
            }
            .to_string(),
            "lifecycle active→hibernating"
        );
    }

    #[tokio::test]
    async fn abci_stub_returns_err() {
        let tx = build_commitment(
            "npub1xyz",
            "abc123",
            1,
            None,
            CommitmentReason::ExplicitRequest,
        );
        let result = submit_to_abci(&tx).await;
        assert!(result.is_err());
    }
}
