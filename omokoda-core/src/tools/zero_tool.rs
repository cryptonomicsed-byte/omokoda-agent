//! Zero (Zerolang) tool — Sovereign-tier self-modification through the graph.
//!
//! Zerolang is an agent-first language where `zero.graph` is the program and
//! `.0` files are projections of it. Agents do not edit source text: they read
//! compiler facts with `zero query`/`zero view` and mutate the program with
//! checked, atomized `zero patch` operations that reject stale or invalid
//! edits. That makes it the natural medium for Tier 5 self-modification — every
//! invocation goes through the normal `act` pipeline, so tier gating, hermetic
//! gates, Sabbath queueing (the tool is classified irreversible), and receipts
//! all apply.
//!
//! The binary is resolved from `ZERO_BIN`, then `zero` on PATH, then
//! `~/.zero/bin/zero`. Subcommands are allowlisted — this tool is a governed
//! window onto the Zero CLI, not a general shell.

use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Command;

use crate::tools::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

/// Subcommands the tool will forward. Read/verify operations plus the checked
/// graph-edit surface. Deliberately excludes anything that would make this a
/// package manager or installer.
const ALLOWED_SUBCOMMANDS: &[&str] = &[
    "query",
    "view",
    "inspect",
    "check",
    "test",
    "patch",
    "explain",
    "run",
    "skills",
    "version",
    "--version",
    "import",  // needed for agent edit loop: import → diagnose → patch cycle
    "doc",
    "size",
    "mem",
];

/// Resolve the Zero compiler binary: `ZERO_BIN` → PATH → `~/.zero/bin/zero`.
pub fn resolve_zero_binary() -> Option<PathBuf> {
    if let Ok(bin) = std::env::var("ZERO_BIN") {
        if !bin.is_empty() {
            let p = PathBuf::from(bin);
            if p.exists() {
                return Some(p);
            }
        }
    }
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("zero");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let home = std::env::var("HOME").ok()?;
    let fallback = PathBuf::from(home).join(".zero/bin/zero");
    fallback.exists().then_some(fallback)
}

/// Parse and validate tool params: `{"args": ["patch", "--op", "addMain"]}`.
/// Pure — unit-testable without a binary or an ExecutionContext.
pub fn parse_zero_args(params: &str) -> Result<Vec<String>, String> {
    let v: serde_json::Value =
        serde_json::from_str(params).map_err(|e| format!("invalid params: {e}"))?;
    let args: Vec<String> = v
        .get("args")
        .and_then(|a| a.as_array())
        .ok_or("missing 'args' — e.g. {\"args\":[\"query\"]}")?
        .iter()
        .map(|x| match x {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect();

    let sub = args
        .first()
        .ok_or("'args' must not be empty — first element is the zero subcommand")?;
    if !ALLOWED_SUBCOMMANDS.contains(&sub.as_str()) {
        return Err(format!(
            "zero subcommand '{sub}' is not allowed. Allowed: {}",
            ALLOWED_SUBCOMMANDS.join(", ")
        ));
    }
    if args.len() > 64 {
        return Err("too many arguments (max 64)".to_string());
    }
    Ok(args)
}

/// A structured diagnostic produced by `zero check --json`.
#[derive(Debug, serde::Deserialize)]
pub struct ZeroDiagnostic {
    pub code: String,
    pub message: Option<String>,
    pub repair: Option<ZeroRepair>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ZeroRepair {
    pub id: String,
    pub description: Option<String>,
}

/// Result of `zero check --json` on a graph target.
#[derive(Debug, serde::Deserialize)]
pub struct ZeroCheckResult {
    pub ok: bool,
    #[serde(default)]
    pub diagnostics: Vec<ZeroDiagnostic>,
}

/// Agent edit loop helper: check → collect diagnostics → return repair plan.
///
/// This models the zerolang agent-repair-demo workflow:
///   1. `zero check --json <target>` → collect all diagnostics
///   2. For each diagnostic with a repair: `zero explain --json <code>`
///   3. Return a structured plan the agent can act on with `zero patch`
///
/// E-49: called from agentic turns when self-modification is requested.
pub fn build_repair_plan(
    bin: &std::path::Path,
    target: &str,
    workspace_root: &std::path::Path,
) -> Result<Vec<(ZeroDiagnostic, Option<serde_json::Value>)>, String> {
    let check_out = std::process::Command::new(bin)
        .args(["check", "--json", target])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| format!("zero check failed: {e}"))?;

    let check_json = String::from_utf8_lossy(&check_out.stdout);
    let result: ZeroCheckResult =
        serde_json::from_str(&check_json).map_err(|e| format!("failed to parse zero check output: {e}"))?;

    if result.ok {
        return Ok(vec![]);
    }

    let mut plan = Vec::new();
    for diag in result.diagnostics {
        let explain = if diag.repair.is_some() {
            let explain_out = std::process::Command::new(bin)
                .args(["explain", "--json", &diag.code])
                .current_dir(workspace_root)
                .output()
                .ok()
                .and_then(|o| serde_json::from_slice(&o.stdout).ok());
            explain_out
        } else {
            None
        };
        plan.push((diag, explain));
    }
    Ok(plan)
}

pub struct ZeroTool;

#[async_trait]
impl Tool for ZeroTool {
    fn name(&self) -> &str {
        "zero"
    }
    fn description(&self) -> &str {
        "Zerolang graph-first self-modification — zero query/view/patch/check/test \
         against the project's zero.graph. Params: {\"args\":[\"query\", ...]}. \
         Requires the zero compiler (ZERO_BIN, PATH, or ~/.zero/bin/zero)."
    }
    fn required_tier(&self) -> u8 {
        5 // Sovereign — self-modification
    }
    fn is_write_operation(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args = parse_zero_args(params)?;
        let bin = resolve_zero_binary().ok_or(
            "zero compiler not found — set ZERO_BIN, add zero to PATH, or install to ~/.zero/bin",
        )?;

        // Direct exec, no shell: arguments cannot be reinterpreted. Runs in
        // the workspace root so zero.graph resolution matches the project.
        let output = Command::new(&bin)
            .args(&args)
            .current_dir(&context.workspace_root)
            .output()
            .map_err(|e| format!("failed to execute zero: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok((stdout, TokenUsage::default()))
        } else {
            // Zero emits structured diagnostics on failure — surface them
            // whole so the agent can build a typed repair plan.
            Err(format!(
                "zero {} failed with status {}: {}{}",
                args.first().map(String::as_str).unwrap_or(""),
                output.status,
                stderr,
                if stdout.is_empty() {
                    String::new()
                } else {
                    format!("\nstdout: {stdout}")
                }
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_missing_args() {
        assert!(parse_zero_args("{}").is_err());
        assert!(parse_zero_args("{\"args\":[]}").is_err());
        assert!(parse_zero_args("not json").is_err());
    }

    #[test]
    fn parse_rejects_non_allowlisted_subcommand() {
        let err = parse_zero_args(r#"{"args":["install","evil"]}"#).unwrap_err();
        assert!(err.contains("not allowed"));
        assert!(parse_zero_args(r#"{"args":["rm","-rf","/"]}"#).is_err());
    }

    #[test]
    fn parse_accepts_query_and_patch_forms() {
        assert_eq!(
            parse_zero_args(r#"{"args":["query"]}"#).unwrap(),
            vec!["query"]
        );
        let patch = parse_zero_args(r#"{"args":["patch","--op","addMain","--json"]}"#).unwrap();
        assert_eq!(patch[0], "patch");
        assert_eq!(patch.len(), 4);
    }

    #[test]
    fn zero_tool_is_sovereign_tier_write() {
        let t = ZeroTool;
        assert_eq!(t.name(), "zero");
        assert_eq!(t.required_tier(), 5);
        assert!(t.is_write_operation());
    }

    #[tokio::test]
    async fn execute_forwards_args_via_zero_bin() {
        // Point ZERO_BIN at /bin/echo: the tool must exec it directly with
        // the validated args, proving no shell is involved.
        std::env::set_var("OMOKODA_TEST_ZERO_BIN_GUARD", "1");
        std::env::set_var("ZERO_BIN", "/bin/echo");
        let t = ZeroTool;
        let ctx = ExecutionContext {
            agent_id: crate::identity::AgentId::from_str("agent-test"),
            name: "luna".to_string(),
            tier: 5,
            reputation: 100.0,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: String::new(),
            },
            workspace_root: std::env::temp_dir(),
            sandbox_mode: false,
        };
        let (out, _) = t
            .execute(r#"{"args":["query","--json"]}"#, &ctx)
            .await
            .expect("echo succeeds");
        assert_eq!(out.trim(), "query --json");
        std::env::remove_var("ZERO_BIN");
    }
}
