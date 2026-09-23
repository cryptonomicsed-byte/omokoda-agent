//! Phase 11A — Unified Mutation Gateway.
//!
//! Every consequential agent action flows through this gateway:
//!
//!   MutationPlan → MutationRouter → validate() → apply() → MutationResult
//!
//! The hard invariant: `validate == failure → apply MUST NOT execute`.
//! The router enforces this unconditionally; backends never see `apply`
//! unless `validate` returned `ValidationResult { passed: true }`.

use std::collections::HashMap;
use std::path::Path;

use crate::mutation::{MutationDomain, MutationPlan};

// ── Context ──────────────────────────────────────────────────────────────────

/// Thin context bag passed to every backend method so backends stay stateless.
/// All fields are optional; backends that need a field check it themselves.
pub struct BackendContext<'a> {
    /// Root directory of the agent's workspace (required by ZeroBackend).
    pub workspace_root: Option<&'a Path>,
    /// Stable string name of the acting agent.
    pub agent_id: Option<&'a str>,
    /// Live memory directory snapshot (required by LARQLBackend).
    pub memory_dir: Option<&'a crate::memory::memdir::OduDirectory>,
    /// Causal DAG snapshot (required by LARQLBackend).
    pub causal_dag: Option<&'a crate::memory::dag::CausalMemoryDag>,
    /// Reflection ledger snapshot (required by LARQLBackend).
    pub reflection: Option<&'a crate::memory::reflection::ReflectionLedger>,
}

impl<'a> BackendContext<'a> {
    pub fn empty() -> Self {
        Self {
            workspace_root: None,
            agent_id: None,
            memory_dir: None,
            causal_dag: None,
            reflection: None,
        }
    }
}

// ── Backend sub-results ───────────────────────────────────────────────────────

/// Output from a backend `inspect` call: a pre-mutation state snapshot.
#[derive(Debug, Clone)]
pub struct InspectResult {
    /// Human-readable lines describing current state.
    pub summary: Vec<String>,
    /// Hash of state before any mutation (if the backend can produce one).
    pub before_hash: Option<[u8; 32]>,
}

/// Output from a backend `validate` call.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// True iff the mutation plan is safe to execute.
    pub passed: bool,
    /// Why validation passed or failed.
    pub reason: String,
}

/// Output from a backend `apply` call.
#[derive(Debug, Clone)]
pub struct ApplyResult {
    /// Human-readable output from the execution.
    pub output: String,
    /// Hash of state after mutation (if the backend can produce one).
    pub after_hash: Option<[u8; 32]>,
    /// Whether the backend supports `rollback` for this operation.
    pub rollback_available: bool,
}

// ── The trait ─────────────────────────────────────────────────────────────────

/// Contract every mutation backend must fulfil.
///
/// Backends are stateless: all per-call state flows through `plan` + `ctx`.
/// The router guarantees that `apply` is never called unless `validate`
/// returned `passed = true`.
pub trait MutationBackend: Send + Sync {
    /// The domain this backend handles.
    fn domain(&self) -> MutationDomain;
    /// Human-readable name (used in receipts and logs).
    fn name(&self) -> &str;

    /// Read current state — never mutates anything.
    fn inspect(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<InspectResult, String>;

    /// Check whether the plan is safe to execute.
    /// MUST NOT mutate any state.
    fn validate(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<ValidationResult, String>;

    /// Execute the mutation.
    /// Router calls this ONLY when `validate` returned `passed = true`.
    fn apply(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<ApplyResult, String>;

    /// Attempt to undo the most recent `apply`.
    /// Backends that cannot roll back return `Ok(())` with a log note.
    fn rollback(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<(), String>;
}

// ── Router ────────────────────────────────────────────────────────────────────

/// Outcome of a successfully validated-and-applied mutation.
#[derive(Debug, Clone)]
pub struct MutationResult {
    /// Stable ID copied from the plan.
    pub plan_id: [u8; 32],
    /// Which state domain was mutated.
    pub domain: MutationDomain,
    /// Name of the backend that executed the plan.
    pub backend_name: String,
    /// Always true — router only produces a MutationResult when validation passed.
    pub validation_passed: bool,
    /// Output from `apply`.
    pub apply_output: String,
    /// State hash before mutation.
    pub before_hash: Option<[u8; 32]>,
    /// State hash after mutation.
    pub after_hash: Option<[u8; 32]>,
    /// Whether the backend supports rollback.
    pub rollback_available: bool,
    /// Unix milliseconds when the mutation completed.
    pub completed_at_ms: u64,
}

/// Routes `MutationPlan`s to the appropriate backend, enforcing the
/// validate-before-apply invariant.
pub struct MutationRouter {
    backends: HashMap<MutationDomain, Box<dyn MutationBackend>>,
}

impl MutationRouter {
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
        }
    }

    /// Register a backend.  Overwrites any existing registration for the same domain.
    pub fn register(&mut self, backend: Box<dyn MutationBackend>) {
        self.backends.insert(backend.domain(), backend);
    }

    /// Route a plan: inspect → validate → (if passed) apply → MutationResult.
    ///
    /// Returns `Err` if:
    /// - no backend is registered for the plan's domain, or
    /// - `validate` returns `passed = false` (apply is NOT called), or
    /// - `apply` returns an error.
    pub fn route_plan(
        &self,
        plan: &MutationPlan,
        ctx: &BackendContext,
    ) -> Result<MutationResult, String> {
        let backend = self
            .backends
            .get(&plan.domain)
            .ok_or_else(|| format!("no backend registered for domain {:?}", plan.domain))?;

        // Inspect (best-effort; failure doesn't block mutation).
        let inspect = backend.inspect(plan, ctx).unwrap_or(InspectResult {
            summary: vec!["[inspect unavailable]".to_string()],
            before_hash: None,
        });

        // Validate — MUST pass before apply is ever called.
        let validation = backend.validate(plan, ctx)?;
        if !validation.passed {
            return Err(format!(
                "mutation validation failed for plan {:?} ({:?}): {}",
                hex::encode(plan.id),
                plan.domain,
                validation.reason
            ));
        }

        // Apply — only reached when validation passed.
        let apply_result = backend.apply(plan, ctx)?;

        let completed_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(MutationResult {
            plan_id: plan.id,
            domain: plan.domain,
            backend_name: backend.name().to_string(),
            validation_passed: true,
            apply_output: apply_result.output,
            before_hash: inspect.before_hash.or(plan.before_hash),
            after_hash: apply_result.after_hash,
            rollback_available: apply_result.rollback_available,
            completed_at_ms,
        })
    }

    /// Attempt rollback for a plan using the registered backend.
    pub fn rollback_plan(
        &self,
        plan: &MutationPlan,
        ctx: &BackendContext,
    ) -> Result<(), String> {
        let backend = self
            .backends
            .get(&plan.domain)
            .ok_or_else(|| format!("no backend for domain {:?}", plan.domain))?;
        backend.rollback(plan, ctx)
    }

    /// Returns true if a backend is registered for the given domain.
    pub fn has_backend(&self, domain: MutationDomain) -> bool {
        self.backends.contains_key(&domain)
    }
}

impl Default for MutationRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ifscript_gate::ActionCategory;
    use crate::mutation::{MutationDomain, MutationPlan};

    /// Minimal always-pass backend for testing the router's invariant enforcement.
    struct PassBackend;
    impl MutationBackend for PassBackend {
        fn domain(&self) -> MutationDomain { MutationDomain::Network }
        fn name(&self) -> &str { "pass" }
        fn inspect(&self, _p: &MutationPlan, _c: &BackendContext) -> Result<InspectResult, String> {
            Ok(InspectResult { summary: vec!["ok".to_string()], before_hash: None })
        }
        fn validate(&self, _p: &MutationPlan, _c: &BackendContext) -> Result<ValidationResult, String> {
            Ok(ValidationResult { passed: true, reason: "always valid".to_string() })
        }
        fn apply(&self, _p: &MutationPlan, _c: &BackendContext) -> Result<ApplyResult, String> {
            Ok(ApplyResult { output: "applied".to_string(), after_hash: None, rollback_available: false })
        }
        fn rollback(&self, _p: &MutationPlan, _c: &BackendContext) -> Result<(), String> { Ok(()) }
    }

    /// Minimal always-fail validation backend.
    struct FailBackend;
    impl MutationBackend for FailBackend {
        fn domain(&self) -> MutationDomain { MutationDomain::Device }
        fn name(&self) -> &str { "fail" }
        fn inspect(&self, _p: &MutationPlan, _c: &BackendContext) -> Result<InspectResult, String> {
            Ok(InspectResult { summary: vec![], before_hash: None })
        }
        fn validate(&self, _p: &MutationPlan, _c: &BackendContext) -> Result<ValidationResult, String> {
            Ok(ValidationResult { passed: false, reason: "stale plan".to_string() })
        }
        fn apply(&self, _p: &MutationPlan, _c: &BackendContext) -> Result<ApplyResult, String> {
            // This MUST never be called — the router invariant guarantees it.
            panic!("apply called after validation failure — router invariant violated")
        }
        fn rollback(&self, _p: &MutationPlan, _c: &BackendContext) -> Result<(), String> { Ok(()) }
    }

    fn make_plan(domain_cat: ActionCategory) -> MutationPlan {
        MutationPlan::from_tool_call("test-agent", "test_tool", domain_cat, 3, 1_000_000)
    }

    #[test]
    fn router_routes_to_correct_backend() {
        let mut router = MutationRouter::new();
        router.register(Box::new(PassBackend));
        let plan = MutationPlan::from_tool_call("a", "t", ActionCategory::Observation, 2, 0);
        // Network domain (from Observation → World, but let's use a plan that hits PassBackend's domain)
        // Actually Observation → World, PassBackend handles Network. Let's build a Network plan.
        // Override by building a plan directly with Network:
        let mut plan = plan;
        plan.domain = MutationDomain::Network;
        let ctx = BackendContext::empty();
        let result = router.route_plan(&plan, &ctx).expect("should succeed");
        assert!(result.validation_passed);
        assert_eq!(result.backend_name, "pass");
    }

    #[test]
    fn validation_failure_blocks_apply() {
        let mut router = MutationRouter::new();
        router.register(Box::new(FailBackend));
        let mut plan = make_plan(ActionCategory::Observation);
        plan.domain = MutationDomain::Device;
        let ctx = BackendContext::empty();
        // route_plan must return Err; the panic in apply must never fire.
        let err = router.route_plan(&plan, &ctx).unwrap_err();
        assert!(err.contains("stale plan"), "expected stale plan in: {err}");
    }

    #[test]
    fn missing_backend_is_an_error() {
        let router = MutationRouter::new(); // empty
        let mut plan = make_plan(ActionCategory::Execution);
        plan.domain = MutationDomain::Program;
        let ctx = BackendContext::empty();
        let err = router.route_plan(&plan, &ctx).unwrap_err();
        assert!(err.contains("no backend"), "expected 'no backend' in: {err}");
    }

    #[test]
    fn has_backend_reports_registration_state() {
        let mut router = MutationRouter::new();
        assert!(!router.has_backend(MutationDomain::Network));
        router.register(Box::new(PassBackend));
        assert!(router.has_backend(MutationDomain::Network));
        assert!(!router.has_backend(MutationDomain::Device));
    }
}
