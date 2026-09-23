//! Sovereign Mutation Bus — Phase 10C / 11A-11D.
//!
//! A `MutationPlan` is the common routing object that travels through
//! If-Script → Èṣù Gate → 16-Vessel dispatch → backend execution.
//!
//! Phase 11 adds the `MutationRouter` + `MutationBackend` trait that routes
//! plans to the correct backend (Zero / LARQL / GIX / ...) and a
//! `MutationReceipt` that gets persisted as `GixKind::Receipt` after every
//! successful apply.

pub mod backends;
pub mod receipt;
pub mod router;

pub use receipt::MutationReceipt;
pub use router::{BackendContext, MutationBackend, MutationResult, MutationRouter};

use crate::ifscript_gate::{ActionCategory, VesselAlignment};

/// The state domain a mutation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MutationDomain {
    /// Zero — change executable program/agent behaviour.
    Program,
    /// LARQL — change model knowledge representation.
    Model,
    /// GIX/Mycelium — change agent memory state.
    Memory,
    /// Bruce/HAL — change physical device state.
    Device,
    /// Tor/Oniux/overlay — change network route or session.
    Network,
    /// Vantage/external — change world-facing state (jobs, broadcasts, etc.).
    World,
    /// OSOVM/Move — change on-chain economic state.
    Economy,
}

/// Effect policy: when (if ever) should this action be written to memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MemoryEffectPolicy {
    /// Always record, regardless of outcome.
    Always,
    /// Record only on success.
    OnSuccess,
    /// Record only on failure (for error memory / post-mortems).
    OnFailure,
    /// Do not record (ephemeral / diagnostic actions).
    Never,
}

/// Receipt policy: how accountably should this action be settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReceiptPolicy {
    /// Block until a Zàngbétò receipt is produced.
    Required,
    /// Fire-and-forget receipt — don't block execution.
    Optional,
    /// No receipt needed (low-stakes / read-only).
    None,
}

/// The common routing/authorization object for every consequential agent action.
///
/// Build one with `MutationPlan::from_tool_call`, pass through If-Script for
/// vessel selection, Èṣù for authorization, then hand to the appropriate
/// mutation provider for execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MutationPlan {
    /// Stable identifier for this plan (SHA-256 of actor+intent+domain+operation+ts).
    pub id:            [u8; 32],
    /// The agent executing the plan.
    pub actor:         String,
    /// Human-readable description of the intent.
    pub intent:        String,
    /// Which state domain is being mutated.
    pub domain:        MutationDomain,
    /// String identifier of the target (tool name, node id, device id, etc.).
    pub target:        String,
    /// Hash of the state before the mutation (if known).
    pub before_hash:   Option<[u8; 32]>,
    /// The operation name (tool_name, LQL verb, Zero patch kind, etc.).
    pub operation:     String,
    /// Minimum tier required to execute this plan.
    pub required_tier: u8,
    /// The action category driving If-Script vessel selection.
    pub category:      ActionCategory,
    /// Vessel alignment result (set after If-Script evaluation).
    pub alignment:     Option<VesselAlignment>,
    /// When/if to write this action to GIX memory.
    pub memory_effect: MemoryEffectPolicy,
    /// Receipt commitment level.
    pub receipt:       ReceiptPolicy,
    /// Unix milliseconds when the plan was created.
    pub created_at_ms: u64,
}

impl MutationPlan {
    /// Build a `MutationPlan` from the data available at tool-call dispatch time.
    ///
    /// Domain is inferred from `ActionCategory`; tier is the agent's current tier.
    pub fn from_tool_call(
        actor:     &str,
        tool_name: &str,
        category:  ActionCategory,
        tier:      u8,
        ts_ms:     u64,
    ) -> Self {
        let domain = domain_for_category(category);
        let operation = tool_name.to_string();
        let intent = format!("execute tool '{}'", tool_name);

        // Stable plan ID: SHA-256 of actor ‖ tool ‖ domain_byte ‖ ts_bytes.
        let mut content = Vec::new();
        content.extend_from_slice(actor.as_bytes());
        content.push(b'|');
        content.extend_from_slice(tool_name.as_bytes());
        content.push(b'|');
        content.push(domain as u8);
        content.push(b'|');
        content.extend_from_slice(&ts_ms.to_le_bytes());

        let id = gix_types::HashDomain::CanonicalId.hash(&content);

        // High-stakes domains require a receipt; economy always required.
        let receipt = match domain {
            MutationDomain::Economy => ReceiptPolicy::Required,
            MutationDomain::World   => ReceiptPolicy::Optional,
            _                       => ReceiptPolicy::None,
        };

        // Program/Model/Memory mutations should always be recorded.
        let memory_effect = match domain {
            MutationDomain::Program |
            MutationDomain::Model   |
            MutationDomain::Memory  => MemoryEffectPolicy::Always,
            _                       => MemoryEffectPolicy::OnSuccess,
        };

        Self {
            id,
            actor: actor.to_string(),
            intent,
            domain,
            target: operation.clone(),
            before_hash: None,
            operation,
            required_tier: tier,
            category,
            alignment: None,
            memory_effect,
            receipt,
            created_at_ms: ts_ms,
        }
    }
}

/// Infer the mutation domain from the action category.
fn domain_for_category(cat: ActionCategory) -> MutationDomain {
    match cat {
        ActionCategory::Identity    |
        ActionCategory::Cryptographic => MutationDomain::Memory,
        ActionCategory::Learning    => MutationDomain::Model,
        ActionCategory::Execution   => MutationDomain::Program,
        ActionCategory::Swarm       |
        ActionCategory::Migration   => MutationDomain::World,
        ActionCategory::Receipt     => MutationDomain::Economy,
        ActionCategory::Privacy     |
        ActionCategory::Consent     => MutationDomain::Memory,
        ActionCategory::Observation |
        ActionCategory::Focus       |
        ActionCategory::Iteration   |
        ActionCategory::Telemetry   |
        ActionCategory::Restraint   |
        ActionCategory::Temporal    |
        ActionCategory::Dissolution => MutationDomain::World,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_plan_domain_inferred_from_category() {
        let plan = MutationPlan::from_tool_call("agent1", "write_file", ActionCategory::Execution, 3, 1000);
        assert_eq!(plan.domain, MutationDomain::Program);
    }

    #[test]
    fn mutation_plan_economy_requires_receipt() {
        let plan = MutationPlan::from_tool_call("agent1", "ingest_arp_receipt", ActionCategory::Receipt, 3, 1000);
        assert_eq!(plan.domain, MutationDomain::Economy);
        assert_eq!(plan.receipt, ReceiptPolicy::Required);
    }

    #[test]
    fn mutation_plan_id_is_deterministic() {
        let p1 = MutationPlan::from_tool_call("a", "read_file", ActionCategory::Observation, 2, 9999);
        let p2 = MutationPlan::from_tool_call("a", "read_file", ActionCategory::Observation, 2, 9999);
        assert_eq!(p1.id, p2.id);
    }

    #[test]
    fn all_categories_have_a_domain() {
        use crate::ifscript_gate::ActionCategory;
        let cats = [
            ActionCategory::Identity, ActionCategory::Dissolution, ActionCategory::Focus,
            ActionCategory::Iteration, ActionCategory::Receipt, ActionCategory::Privacy,
            ActionCategory::Telemetry, ActionCategory::Execution, ActionCategory::Swarm,
            ActionCategory::Restraint, ActionCategory::Migration, ActionCategory::Consent,
            ActionCategory::Observation, ActionCategory::Learning, ActionCategory::Cryptographic,
            ActionCategory::Temporal,
        ];
        for cat in cats {
            let _ = domain_for_category(cat); // must not panic
        }
    }
}
