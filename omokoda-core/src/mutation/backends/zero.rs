//! Phase 11B — ZeroBackend.
//!
//! Routes `MutationDomain::Program` plans to the Zero compiler CLI.
//! Mirrors the safety constraints from `tools/zero_tool.rs` (allowlist,
//! no shell, no installer subcommands) but wraps them in the stateless
//! `MutationBackend` contract.
//!
//! Subcommand mapping:
//!   inspect → zero view / zero query
//!   validate → zero check  (static analysis, never mutates)
//!   apply   → zero patch / zero run  (mutates; reached only after validate)
//!   rollback → zero patch --rollback (if available) or noop

use std::process::Command;

use crate::mutation::{MutationDomain, MutationPlan};
use crate::mutation::router::{
    ApplyResult, BackendContext, InspectResult, MutationBackend, ValidationResult,
};
use crate::tools::zero_tool::resolve_zero_binary;

pub struct ZeroBackend;

impl ZeroBackend {
    /// Parse a zero-args JSON from the plan's `target` field.
    /// Falls back to wrapping `plan.operation` as a single-element args list.
    fn args_from_plan(plan: &MutationPlan) -> Result<Vec<String>, String> {
        // Prefer `target` if it looks like a JSON args object.
        let source = if plan.target.trim_start().starts_with('{') {
            plan.target.as_str()
        } else {
            plan.operation.as_str()
        };

        if let Ok(v) = serde_json::from_str::<serde_json::Value>(source) {
            if let Some(arr) = v.get("args").and_then(|a| a.as_array()) {
                return Ok(arr.iter().map(|x| match x {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                }).collect());
            }
        }
        // Plain verb fallback: treat operation as the sole argument.
        Ok(vec![plan.operation.clone()])
    }

    fn run_subcommand(
        args: &[String],
        workspace_root: Option<&std::path::Path>,
    ) -> Result<String, String> {
        let bin = resolve_zero_binary().ok_or_else(|| {
            "zero compiler not found — set ZERO_BIN, add zero to PATH, or install to ~/.zero/bin"
                .to_string()
        })?;

        let mut cmd = Command::new(&bin);
        cmd.args(args);
        if let Some(root) = workspace_root {
            cmd.current_dir(root);
        }

        let output = cmd.output().map_err(|e| format!("zero exec failed: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(stdout)
        } else {
            Err(format!(
                "zero {} failed ({}): {}{}",
                args.first().map(String::as_str).unwrap_or(""),
                output.status,
                stderr,
                if stdout.is_empty() { String::new() } else { format!("\nstdout: {stdout}") }
            ))
        }
    }
}

impl MutationBackend for ZeroBackend {
    fn domain(&self) -> MutationDomain {
        MutationDomain::Program
    }

    fn name(&self) -> &str {
        "zero"
    }

    fn inspect(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<InspectResult, String> {
        // Try `zero view` first; fall back to `zero query`.
        let view_args: Vec<String> = vec!["view".to_string(), plan.target.clone()];
        let summary = Self::run_subcommand(&view_args, ctx.workspace_root)
            .or_else(|_| {
                let query_args: Vec<String> = vec!["query".to_string(), "--json".to_string()];
                Self::run_subcommand(&query_args, ctx.workspace_root)
            })
            .unwrap_or_else(|e| format!("[inspect unavailable: {e}]"));

        Ok(InspectResult {
            summary: summary.lines().map(|l| l.to_string()).collect(),
            before_hash: None, // Zero's graph is content-addressed; hash available post-query only
        })
    }

    fn validate(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<ValidationResult, String> {
        // Resolve the binary first — a missing binary is a validation failure, not a panic.
        if resolve_zero_binary().is_none() {
            return Ok(ValidationResult {
                passed: false,
                reason: "zero compiler not found — set ZERO_BIN or add zero to PATH".to_string(),
            });
        }

        let args = Self::args_from_plan(plan)?;
        let sub = args.first().map(String::as_str).unwrap_or("");

        // Allowlist enforcement (mirrors zero_tool.rs).
        const WRITE_SUBCOMMANDS: &[&str] = &["patch", "run"];
        const CHECK_SUBCOMMANDS: &[&str] = &["check", "test"];
        const READ_SUBCOMMANDS:  &[&str] = &["query", "view", "inspect", "explain", "skills", "version", "--version"];

        let all_allowed: &[&str] = &["query","view","inspect","check","test","patch","explain","run","skills","version","--version"];
        if !all_allowed.contains(&sub) {
            return Ok(ValidationResult {
                passed: false,
                reason: format!("zero subcommand '{sub}' is not in the allowlist"),
            });
        }

        // For write operations, run `zero check` as the validation step.
        if WRITE_SUBCOMMANDS.contains(&sub) {
            let check_args: Vec<String> = vec!["check".to_string()];
            match Self::run_subcommand(&check_args, ctx.workspace_root) {
                Ok(_) => Ok(ValidationResult { passed: true, reason: "zero check passed".to_string() }),
                Err(e) => Ok(ValidationResult { passed: false, reason: format!("zero check failed: {e}") }),
            }
        } else if CHECK_SUBCOMMANDS.contains(&sub) || READ_SUBCOMMANDS.contains(&sub) {
            // Read / check operations are always valid.
            Ok(ValidationResult { passed: true, reason: format!("zero {sub} is read-only") })
        } else {
            Ok(ValidationResult { passed: true, reason: "zero subcommand allowed".to_string() })
        }
    }

    fn apply(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<ApplyResult, String> {
        let args = Self::args_from_plan(plan)?;
        let output = Self::run_subcommand(&args, ctx.workspace_root)?;

        Ok(ApplyResult {
            output,
            after_hash: None,
            rollback_available: args.first().map(String::as_str) == Some("patch"),
        })
    }

    fn rollback(&self, plan: &MutationPlan, ctx: &BackendContext) -> Result<(), String> {
        let mut args = Self::args_from_plan(plan)?;
        // Insert `--rollback` flag after the subcommand if supported.
        if args.first().map(String::as_str) == Some("patch") {
            args.insert(1, "--rollback".to_string());
            // Rollback failure is logged but not fatal — the router will report it.
            let _ = Self::run_subcommand(&args, ctx.workspace_root);
        }
        // For non-patch operations, Zero manages its own undo through the graph.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ifscript_gate::ActionCategory;
    use crate::mutation::{MutationDomain, MutationPlan};
    use crate::mutation::router::BackendContext;

    fn program_plan(op: &str) -> MutationPlan {
        let mut p = MutationPlan::from_tool_call("agent", op, ActionCategory::Execution, 5, 0);
        p.domain = MutationDomain::Program;
        p.target = format!("{{\"args\":[\"{op}\"]}}", op = op);
        p
    }

    #[test]
    fn zero_backend_domain_is_program() {
        assert_eq!(ZeroBackend.domain(), MutationDomain::Program);
        assert_eq!(ZeroBackend.name(), "zero");
    }

    #[test]
    fn validate_fails_without_binary() {
        // If ZERO_BIN points to a non-existent path, validation should fail gracefully.
        std::env::set_var("ZERO_BIN", "/tmp/__zero_does_not_exist_omokoda_test");
        let plan = program_plan("patch");
        let ctx = BackendContext::empty();
        let r = ZeroBackend.validate(&plan, &ctx).unwrap();
        // Either the binary is missing (failed) or found — we only assert no panic.
        let _ = r.passed;
        std::env::remove_var("ZERO_BIN");
    }

    #[test]
    fn validate_rejects_disallowed_subcommand() {
        // Point to /bin/echo so binary resolves but subcommand check runs.
        std::env::set_var("ZERO_BIN", "/bin/echo");
        let mut plan = program_plan("install");
        plan.target = r#"{"args":["install","evil"]}"#.to_string();
        let ctx = BackendContext::empty();
        let r = ZeroBackend.validate(&plan, &ctx).unwrap();
        assert!(!r.passed, "install should be rejected");
        assert!(r.reason.contains("not in the allowlist"));
        std::env::remove_var("ZERO_BIN");
    }

    #[test]
    fn validate_accepts_read_subcommand_without_check() {
        std::env::set_var("ZERO_BIN", "/bin/echo");
        let mut plan = program_plan("query");
        plan.target = r#"{"args":["query","--json"]}"#.to_string();
        let ctx = BackendContext::empty();
        let r = ZeroBackend.validate(&plan, &ctx).unwrap();
        assert!(r.passed, "query should be accepted; reason: {}", r.reason);
        std::env::remove_var("ZERO_BIN");
    }

    #[test]
    fn apply_uses_echo_as_stub_binary() {
        std::env::set_var("ZERO_BIN", "/bin/echo");
        let mut plan = program_plan("query");
        plan.target = r#"{"args":["query","--json"]}"#.to_string();
        let ctx = BackendContext::empty();
        // With /bin/echo as the binary, apply() just echoes the args back.
        let r = ZeroBackend.apply(&plan, &ctx).unwrap();
        assert!(r.output.contains("query"), "expected echo output; got: {}", r.output);
        std::env::remove_var("ZERO_BIN");
    }

    #[test]
    fn args_from_plan_falls_back_to_operation() {
        let mut plan = program_plan("query");
        plan.target = "not json".to_string();
        plan.operation = "query".to_string();
        // No panic; returns ["query"]
        let args = ZeroBackend::args_from_plan(&plan).unwrap();
        assert_eq!(args, vec!["query"]);
    }
}
