//! Phase 11C — LARQLBackend.
//!
//! Routes `MutationDomain::Model` plans to the LARQL-over-memory query engine
//! (`memory::larql_query`).  The model graph is the agent's knowledge
//! representation — LARQL reads it, patches update it.
//!
//! Subcommand mapping:
//!   inspect  → DESCRIBE entities / DESCRIBE reflections
//!   validate → parse the query (dry-run); check context is available
//!   apply    → execute the query against the live OduDirectory snapshot
//!   rollback → not supported (LARQL queries are read-only or append-only)
//!
//! The `plan.operation` field carries the LARQL query string.  For write-side
//! mutations (knowledge ingestion), `plan.target` carries a JSON payload
//! `{"op": "ingest", "content": "...", "path": "..."}`.

use crate::memory::dag::CausalMemoryDag;
use crate::memory::larql_query::{self, MemoryQuery};
use crate::memory::memdir::OduDirectory;
use crate::memory::reflection::ReflectionLedger;
use crate::mutation::{MutationDomain, MutationPlan};
use crate::mutation::router::{
    ApplyResult, BackendContext, InspectResult, MutationBackend, ValidationResult,
};

pub struct LARQLBackend;

impl LARQLBackend {
    /// Parse the LARQL query from `plan.operation`.
    fn parse_query(plan: &MutationPlan) -> Result<MemoryQuery, String> {
        larql_query::parse_query(&plan.operation)
            .map_err(|e| format!("LARQL parse error: {e}"))
    }

    /// Execute a query against the provided context, returning formatted lines.
    fn execute_query(
        query: &MemoryQuery,
        dir: &OduDirectory,
        dag: &CausalMemoryDag,
        reflection: &ReflectionLedger,
    ) -> Vec<String> {
        let answer = larql_query::execute(query, dir, dag, reflection);
        let mut lines = answer.summary;
        if let Some(passed) = answer.passed {
            lines.push(format!("  → {}", if passed { "PASS" } else { "FAIL" }));
        }
        lines
    }

    /// Extract OduDirectory, CausalMemoryDag, and ReflectionLedger from ctx.
    /// Returns Err if any required field is absent.
    fn require_context<'a>(
        ctx: &'a BackendContext<'_>,
    ) -> Result<(&'a OduDirectory, &'a CausalMemoryDag, &'a ReflectionLedger), String> {
        let dir = ctx.memory_dir.ok_or("LARQL backend requires memory_dir in context")?;
        let dag = ctx.causal_dag.ok_or("LARQL backend requires causal_dag in context")?;
        let refl = ctx.reflection.ok_or("LARQL backend requires reflection in context")?;
        Ok((dir, dag, refl))
    }
}

impl MutationBackend for LARQLBackend {
    fn domain(&self) -> MutationDomain {
        MutationDomain::Model
    }

    fn name(&self) -> &str {
        "larql"
    }

    fn inspect(&self, _plan: &MutationPlan, ctx: &BackendContext) -> Result<InspectResult, String> {
        // Best-effort: describe the current memory state.
        let Ok((dir, dag, reflection)) = Self::require_context(ctx) else {
            return Ok(InspectResult {
                summary: vec!["[LARQL inspect: no memory context available]".to_string()],
                before_hash: None,
            });
        };

        let entities_q = larql_query::parse_query("DESCRIBE entities").unwrap();
        let mut summary = Self::execute_query(&entities_q, dir, dag, reflection);
        let reflections_q = larql_query::parse_query("DESCRIBE reflections").unwrap();
        summary.extend(Self::execute_query(&reflections_q, dir, dag, reflection));

        Ok(InspectResult {
            summary,
            before_hash: None,
        })
    }

    fn validate(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<ValidationResult, String> {
        // 1. Parse the query — if it doesn't parse, the plan is invalid.
        let query = match Self::parse_query(plan) {
            Ok(q) => q,
            Err(e) => {
                return Ok(ValidationResult {
                    passed: false,
                    reason: e,
                });
            }
        };

        // 2. Check that context is available for execution.
        if Self::require_context(ctx).is_err() {
            // No memory context — read-only queries can still be described as valid
            // since they'll fail at apply time with a clear error; but VERIFY queries
            // need live data to be meaningful.
            let needs_live = matches!(
                query,
                MemoryQuery::VerifyEntity(_) | MemoryQuery::VerifyPathContains(_) | MemoryQuery::TraceCausal(_)
            );
            if needs_live {
                return Ok(ValidationResult {
                    passed: false,
                    reason: "LARQL VERIFY/TRACE requires live memory context (none provided)".to_string(),
                });
            }
        }

        Ok(ValidationResult {
            passed: true,
            reason: format!("LARQL query parsed: {:?}", plan.operation),
        })
    }

    fn apply(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<ApplyResult, String> {
        let query = Self::parse_query(plan)?;
        let (dir, dag, reflection) = Self::require_context(ctx)?;

        let lines = Self::execute_query(&query, dir, dag, reflection);
        let passed = {
            let answer = larql_query::execute(&query, dir, dag, reflection);
            answer.passed
        };

        let output = lines.join("\n");
        let rollback_available = false; // LARQL queries are read-only / append-only

        // For VERIFY queries that fail, return an Err so the router logs a failure receipt.
        if let Some(false) = passed {
            return Err(format!("LARQL query assertion failed:\n{output}"));
        }

        Ok(ApplyResult {
            output,
            after_hash: None,
            rollback_available,
        })
    }

    fn rollback(&self, _plan: &MutationPlan, _ctx: &BackendContext) -> Result<(), String> {
        // LARQL queries produce no side-effects; nothing to roll back.
        tracing::debug!("LARQLBackend: rollback is a no-op (queries are read-only)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ifscript_gate::ActionCategory;
    use crate::memory::dag::CausalMemoryDag;
    use crate::memory::memdir::{OduDirectory, OduEntry};
    use crate::memory::reflection::ReflectionLedger;
    use crate::mutation::{MutationDomain, MutationPlan};
    use crate::mutation::router::BackendContext;

    fn model_plan(op: &str) -> MutationPlan {
        let mut p = MutationPlan::from_tool_call("agent", op, ActionCategory::Learning, 3, 0);
        p.domain = MutationDomain::Model;
        p.operation = op.to_string();
        p
    }

    fn dir_with_entry(content: &str, path: &str) -> OduDirectory {
        let mut dir = OduDirectory::new();
        dir.insert(OduEntry::new("e1", content, path));
        dir
    }

    #[test]
    fn larql_backend_domain_is_model() {
        assert_eq!(LARQLBackend.domain(), MutationDomain::Model);
        assert_eq!(LARQLBackend.name(), "larql");
    }

    #[test]
    fn validate_rejects_unparseable_query() {
        let plan = model_plan("WALK something");
        let ctx = BackendContext::empty();
        let r = LARQLBackend.validate(&plan, &ctx).unwrap();
        assert!(!r.passed);
        assert!(r.reason.contains("LARQL parse error"));
    }

    #[test]
    fn validate_accepts_describe_without_context() {
        let plan = model_plan("DESCRIBE entities");
        let ctx = BackendContext::empty();
        let r = LARQLBackend.validate(&plan, &ctx).unwrap();
        assert!(r.passed, "DESCRIBE should be valid without context");
    }

    #[test]
    fn validate_rejects_verify_without_context() {
        let plan = model_plan(r#"VERIFY WHERE entity = "Vantage""#);
        let ctx = BackendContext::empty();
        let r = LARQLBackend.validate(&plan, &ctx).unwrap();
        assert!(!r.passed);
        assert!(r.reason.contains("requires live memory context"));
    }

    #[test]
    fn apply_verify_entity_passes_when_present() {
        let dir = dir_with_entry("we discussed Vantage at length", "think/test");
        let dag = CausalMemoryDag::default();
        let refl = ReflectionLedger::default();
        let plan = model_plan(r#"VERIFY WHERE entity = "Vantage""#);
        let ctx = BackendContext {
            workspace_root: None,
            agent_id: None,
            memory_dir: Some(&dir),
            causal_dag: Some(&dag),
            reflection: Some(&refl),
        };
        let r = LARQLBackend.apply(&plan, &ctx).unwrap();
        assert!(r.output.contains("PASS"), "output: {}", r.output);
    }

    #[test]
    fn apply_verify_entity_fails_when_absent() {
        let dir = dir_with_entry("nothing relevant here", "think/test");
        let dag = CausalMemoryDag::default();
        let refl = ReflectionLedger::default();
        let plan = model_plan(r#"VERIFY WHERE entity = "Vantage""#);
        let ctx = BackendContext {
            workspace_root: None,
            agent_id: None,
            memory_dir: Some(&dir),
            causal_dag: Some(&dag),
            reflection: Some(&refl),
        };
        let r = LARQLBackend.apply(&plan, &ctx);
        assert!(r.is_err(), "expected Err for failed VERIFY");
        assert!(r.unwrap_err().contains("assertion failed"));
    }

    #[test]
    fn apply_describe_entities_succeeds() {
        let dir = dir_with_entry("Vantage and Zangbeto discussed", "think/session");
        let dag = CausalMemoryDag::default();
        let refl = ReflectionLedger::default();
        let plan = model_plan("DESCRIBE entities");
        let ctx = BackendContext {
            workspace_root: None,
            agent_id: None,
            memory_dir: Some(&dir),
            causal_dag: Some(&dag),
            reflection: Some(&refl),
        };
        let r = LARQLBackend.apply(&plan, &ctx).unwrap();
        assert!(!r.output.is_empty());
        assert!(!r.rollback_available);
    }

    #[test]
    fn rollback_is_noop() {
        let plan = model_plan("DESCRIBE entities");
        let ctx = BackendContext::empty();
        assert!(LARQLBackend.rollback(&plan, &ctx).is_ok());
    }
}
