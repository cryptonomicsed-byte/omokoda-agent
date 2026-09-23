//! Phase 11D — MutationReceipt.
//!
//! After `MutationRouter::route_plan` succeeds, the caller builds a
//! `MutationReceipt` and persists it as a `GixKind::Receipt` envelope.
//! This closes the acceptance invariant:
//!
//!   MutationResult → MutationReceipt → Gix1(GixKind::Receipt) → GixStore
//!
//! A receipt is only ever created for a `MutationResult` that reached
//! `apply()` (i.e. `validation_passed == true`).  Failed validations
//! produce no receipt — the failure is recorded as action memory by
//! `record_action_memory()` instead.

use crate::mutation::{MutationDomain, MutationPlan};
use crate::mutation::router::MutationResult;

/// A sealed record of a completed mutation.
///
/// Persisted as `GixKind::Receipt` via `to_gix1()`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MutationReceipt {
    /// Stable identifier copied from the plan.
    pub mutation_id: [u8; 32],
    /// The agent that executed the mutation.
    pub actor: String,
    /// Which state domain was mutated.
    pub domain: MutationDomain,
    /// Name of the backend that executed the plan.
    pub backend: String,
    /// State hash before the mutation.
    pub before_hash: Option<[u8; 32]>,
    /// State hash after the mutation.
    pub after_hash: Option<[u8; 32]>,
    /// SHA-256 of the serialized receipt content (self-referential integrity).
    pub mutation_hash: [u8; 32],
    /// Tier level at which the mutation was authorized.
    pub authorization_tier: u8,
    /// Always true in a MutationReceipt (failed validations produce no receipt).
    pub validation_passed: bool,
    /// Summary from `apply_output`.
    pub result_summary: String,
    /// Whether the backend supports rollback.
    pub rollback_available: bool,
    /// Canonical id hex of the corresponding Phase 10A action memory envelope.
    pub memory_id: Option<String>,
    /// Canonical id hex of a preceding mutation in the same session chain.
    pub parent_mutation_id: Option<[u8; 32]>,
    /// Unix milliseconds when this receipt was sealed.
    pub created_at_ms: u64,
}

impl MutationReceipt {
    /// Build a `MutationReceipt` from a plan and the result of `route_plan`.
    ///
    /// Panics if `result.validation_passed == false` — callers must never build
    /// a receipt for a failed mutation.
    pub fn from_result(
        plan: &MutationPlan,
        result: &MutationResult,
        memory_id: Option<String>,
        parent_mutation_id: Option<[u8; 32]>,
    ) -> Self {
        assert!(
            result.validation_passed,
            "MutationReceipt::from_result called with validation_passed=false — this is a bug"
        );

        let created_at_ms = result.completed_at_ms;

        // Compute mutation_hash: SHA-256 of serialized key fields.
        let mut content = Vec::new();
        content.extend_from_slice(&plan.id);
        content.extend_from_slice(plan.actor.as_bytes());
        content.push(b'|');
        content.extend_from_slice(result.backend_name.as_bytes());
        content.push(b'|');
        content.extend_from_slice(&created_at_ms.to_le_bytes());
        let mutation_hash = gix_types::HashDomain::ContentHash.hash(&content);

        Self {
            mutation_id: plan.id,
            actor: plan.actor.clone(),
            domain: result.domain,
            backend: result.backend_name.clone(),
            before_hash: result.before_hash,
            after_hash: result.after_hash,
            mutation_hash,
            authorization_tier: plan.required_tier,
            validation_passed: true,
            result_summary: result.apply_output.chars().take(500).collect(),
            rollback_available: result.rollback_available,
            memory_id,
            parent_mutation_id,
            created_at_ms,
        }
    }

    /// Canonical hex of the mutation_id.
    pub fn mutation_id_hex(&self) -> String {
        hex::encode(self.mutation_id)
    }

    /// Convert this receipt into a `Gix1` envelope for insertion into a
    /// `CanonicalObjectStore` as `GixKind::Receipt`.
    ///
    /// The canonical_id of the returned envelope is the content-addressed
    /// SHA-256 of the serialized receipt.
    pub fn to_gix1(&self) -> gix_types::Gix1 {
        let payload = serde_json::to_vec(self).unwrap_or_else(|_| {
            // Fallback: serialize mutation_id as hex string if serde fails.
            hex::encode(self.mutation_id).into_bytes()
        });

        gix_types::Gix1::new(
            gix_core::GixKind::Receipt,
            gix_types::GixNamespace::ArpReceipt,
            &payload,
            None,
            self.created_at_ms,
            gix_types::RoutingHints::default(),
        )
    }

    /// Persist this receipt into a `CanonicalObjectStore`.
    ///
    /// Returns the canonical id hex of the inserted `GixKind::Receipt` envelope.
    pub fn persist(&self, store: &mut gix_core::CanonicalObjectStore) -> String {
        let env = self.to_gix1();
        store.insert_object(env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ifscript_gate::ActionCategory;
    use crate::mutation::{MutationDomain, MutationPlan};
    use crate::mutation::router::MutationResult;

    fn make_plan() -> MutationPlan {
        MutationPlan::from_tool_call("agent-1", "zero_patch", ActionCategory::Execution, 5, 1_000_000)
    }

    fn make_result(plan: &MutationPlan) -> MutationResult {
        MutationResult {
            plan_id: plan.id,
            domain: MutationDomain::Program,
            backend_name: "zero".to_string(),
            validation_passed: true,
            apply_output: "patch applied ok".to_string(),
            before_hash: None,
            after_hash: None,
            rollback_available: true,
            completed_at_ms: 1_000_100,
        }
    }

    #[test]
    fn receipt_built_from_valid_result() {
        let plan = make_plan();
        let result = make_result(&plan);
        let receipt = MutationReceipt::from_result(&plan, &result, None, None);
        assert!(receipt.validation_passed);
        assert_eq!(receipt.backend, "zero");
        assert_eq!(receipt.domain, MutationDomain::Program);
        assert_eq!(receipt.authorization_tier, 5);
        assert!(receipt.rollback_available);
        assert!(receipt.result_summary.contains("patch applied ok"));
    }

    #[test]
    fn receipt_mutation_hash_is_deterministic() {
        let plan = make_plan();
        let result = make_result(&plan);
        let r1 = MutationReceipt::from_result(&plan, &result, None, None);
        let r2 = MutationReceipt::from_result(&plan, &result, None, None);
        assert_eq!(r1.mutation_hash, r2.mutation_hash);
    }

    #[test]
    fn receipt_to_gix1_has_receipt_kind() {
        let plan = make_plan();
        let result = make_result(&plan);
        let receipt = MutationReceipt::from_result(&plan, &result, None, None);
        let env = receipt.to_gix1();
        assert_eq!(env.kind, gix_core::GixKind::Receipt);
        assert!(env.verify_integrity());
    }

    #[test]
    fn receipt_persist_registers_in_store() {
        use gix_core::CanonicalObjectStore;
        let plan = make_plan();
        let result = make_result(&plan);
        let mut receipt = MutationReceipt::from_result(&plan, &result, None, None);
        let mut store = CanonicalObjectStore::new();
        let id = receipt.persist(&mut store);
        assert_eq!(id.len(), 64, "canonical id must be 64-char hex");
        // Verify the receipt is findable by kind.
        let found = store.lookup_by_kind(&gix_core::GixKind::Receipt);
        assert!(!found.is_empty(), "receipt not found in store");
        assert_eq!(found[0].canonical_id, id);
    }

    #[test]
    fn receipt_carries_memory_id_link() {
        let plan = make_plan();
        let result = make_result(&plan);
        let mem_id = Some("deadbeef1234".to_string());
        let receipt = MutationReceipt::from_result(&plan, &result, mem_id.clone(), None);
        assert_eq!(receipt.memory_id, mem_id);
    }

    #[test]
    #[should_panic(expected = "validation_passed=false")]
    fn building_receipt_from_failed_result_panics() {
        let plan = make_plan();
        let mut result = make_result(&plan);
        result.validation_passed = false; // force invalid state
        // Must panic — no receipt for a failed mutation.
        let _ = MutationReceipt::from_result(&plan, &result, None, None);
    }
}
