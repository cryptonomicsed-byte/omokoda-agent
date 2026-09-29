//! DeployPipeline — 5-step authorization-tiered deployment pipeline.
//!
//! Step 1: Parse     — validate JSON is well-formed OSO-IR
//! Step 2: Lint      — inline semantic/security/resource rules
//! Step 3: Simulate  — lightweight dry-run (instruction count + opcode sanity)
//! Step 4: Authorize — tier check against target
//! Step 5: Deploy    — write manifest to ~/.oso/deployed/ (or pending/ for mainnet)

use std::collections::HashSet;
use serde::{Deserialize, Serialize};

use crate::auth::{AuthorizationGate, DeployTarget};
use crate::manifest::DeployManifest;

// ── Shared IR types (self-contained copy; mirrors oso-linter) ────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct IrInstruction {
    #[allow(dead_code)]
    pub opcode: u8,
    pub opcode_name: String,
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub line: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum IrInput {
    List(Vec<IrInstruction>),
    Wrapped(IrProgram),
}

#[derive(Debug, Clone, Deserialize)]
struct IrProgram {
    pub instructions: Vec<IrInstruction>,
    #[serde(default)]
    #[allow(dead_code)]
    pub metadata: serde_json::Value,
}

impl IrInput {
    fn into_instructions(self) -> Vec<IrInstruction> {
        match self {
            IrInput::List(v) => v,
            IrInput::Wrapped(p) => p.instructions,
        }
    }
}

// ── Pipeline result types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PipelineStep {
    pub name: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineOutput {
    pub passed: bool,
    pub target: String,
    pub steps: Vec<PipelineStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<DeployManifest>,
    /// Set when mainnet deploy is queued for governance approval.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_approval_path: Option<String>,
}

// ── Pipeline ──────────────────────────────────────────────────────────────────

pub struct DeployPipeline;

impl DeployPipeline {
    pub fn run(program_json: &str, target: &str, signer_tier: u8) -> PipelineOutput {
        let mut steps: Vec<PipelineStep> = Vec::new();

        // ── Step 1: Parse ────────────────────────────────────────────────────
        let instructions: Vec<IrInstruction> = match serde_json::from_str::<IrInput>(program_json) {
            Ok(v) => {
                steps.push(PipelineStep {
                    name: "PARSE".into(),
                    passed: true,
                    error: None,
                    warnings: vec![],
                });
                v.into_instructions()
            }
            Err(e) => {
                steps.push(PipelineStep {
                    name: "PARSE".into(),
                    passed: false,
                    error: Some(format!("JSON parse error: {}", e)),
                    warnings: vec![],
                });
                return PipelineOutput { passed: false, target: target.into(), steps, manifest: None, pending_approval_path: None };
            }
        };

        // ── Step 2: Lint ─────────────────────────────────────────────────────
        let (lint_errors, lint_warnings) = lint_inline(&instructions);
        if !lint_errors.is_empty() {
            steps.push(PipelineStep {
                name: "LINT".into(),
                passed: false,
                error: Some(lint_errors.join("; ")),
                warnings: lint_warnings,
            });
            return PipelineOutput { passed: false, target: target.into(), steps, manifest: None, pending_approval_path: None };
        }
        steps.push(PipelineStep {
            name: "LINT".into(),
            passed: true,
            error: None,
            warnings: lint_warnings,
        });

        // ── Step 3: Simulate ─────────────────────────────────────────────────
        let (sim_ok, sim_err, sim_warnings) = simulate_inline(&instructions);
        if !sim_ok {
            steps.push(PipelineStep {
                name: "SIMULATE".into(),
                passed: false,
                error: sim_err,
                warnings: sim_warnings,
            });
            return PipelineOutput { passed: false, target: target.into(), steps, manifest: None, pending_approval_path: None };
        }
        steps.push(PipelineStep {
            name: "SIMULATE".into(),
            passed: true,
            error: None,
            warnings: sim_warnings,
        });

        // ── Step 4: Authorize ────────────────────────────────────────────────
        let deploy_target = match DeployTarget::from_str(target) {
            Some(t) => t,
            None => {
                steps.push(PipelineStep {
                    name: "AUTHORIZE".into(),
                    passed: false,
                    error: Some(format!("unknown deploy target '{}'; valid: local|testnet|mainnet", target)),
                    warnings: vec![],
                });
                return PipelineOutput { passed: false, target: target.into(), steps, manifest: None, pending_approval_path: None };
            }
        };

        if let Err(required) = AuthorizationGate::check(deploy_target, signer_tier) {
            steps.push(PipelineStep {
                name: "AUTHORIZE".into(),
                passed: false,
                error: Some(format!(
                    "signer tier {} is insufficient for '{}'; requires tier {}",
                    signer_tier, target, required
                )),
                warnings: vec![],
            });
            return PipelineOutput { passed: false, target: target.into(), steps, manifest: None, pending_approval_path: None };
        }
        steps.push(PipelineStep {
            name: "AUTHORIZE".into(),
            passed: true,
            error: None,
            warnings: vec![],
        });

        // ── Step 5: Deploy ───────────────────────────────────────────────────
        let manifest = DeployManifest::new(program_json, target, signer_tier);

        // Mainnet: write to ~/.oso/pending/ and require governance approval
        let pending_approval_path = if AuthorizationGate::requires_governance_approval(deploy_target) {
            let pending_dir = shellexpand_tilde("~/.oso/pending");
            let _ = std::fs::create_dir_all(&pending_dir);
            let path = format!("{}/{}.pending.json", pending_dir, manifest.program_id);
            let _ = std::fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap_or_default());
            steps.push(PipelineStep {
                name: "DEPLOY".into(),
                passed: true,
                error: None,
                warnings: vec![format!("mainnet deploy queued for governance approval: {}", path)],
            });
            Some(path)
        } else {
            let deployed_dir = shellexpand_tilde("~/.oso/deployed");
            let _ = std::fs::create_dir_all(&deployed_dir);
            let path = format!("{}/{}.manifest.json", deployed_dir, manifest.program_id);
            let _ = std::fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap_or_default());
            steps.push(PipelineStep {
                name: "DEPLOY".into(),
                passed: true,
                error: None,
                warnings: vec![format!("manifest written to {}", path)],
            });
            None
        };

        PipelineOutput {
            passed: true,
            target: target.into(),
            steps,
            manifest: Some(manifest),
            pending_approval_path,
        }
    }
}

// ── Inline lint rules (mirrors oso-linter without importing it) ───────────────

fn lint_inline(instructions: &[IrInstruction]) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Semantic: AGENT_BIRTH odu_id must be 0–255
    for instr in instructions {
        if instr.opcode_name == "AGENT_BIRTH" {
            if let Some(odu) = instr.args.get("odu_id").and_then(|v| v.as_i64()) {
                if !(0..=255).contains(&odu) {
                    errors.push(format!(
                        "semantic: AGENT_BIRTH at line {} has invalid odu_id={} (must be 0–255)",
                        instr.line, odu
                    ));
                }
            }
        }
    }

    // Security: self-escalation via grant_tier
    for instr in instructions {
        let caller_tier = instr.args.get("caller_tier")
            .or_else(|| instr.args.get("current_tier"))
            .and_then(|v| v.as_i64());
        let grant_tier = instr.args.get("grant_tier")
            .or_else(|| instr.args.get("new_tier"))
            .and_then(|v| v.as_i64());
        if let (Some(c), Some(g)) = (caller_tier, grant_tier) {
            if g > c {
                errors.push(format!(
                    "security: {} at line {} attempts tier escalation caller_tier={} → grant_tier={}",
                    instr.opcode_name, instr.line, c, g
                ));
            }
        }
    }

    // Resource: warn on > 10 economy instructions
    const ECONOMY_OPS: &[&str] = &["TRANSFER_ASE", "LOCK_ASE", "BURN_ASE", "EMIT_ASE"];
    let economy_count = instructions.iter()
        .filter(|i| ECONOMY_OPS.contains(&i.opcode_name.as_str()))
        .count();
    if economy_count > 10 {
        warnings.push(format!(
            "resource: {} economy instructions (threshold=10); consider batching",
            economy_count
        ));
    }

    (errors, warnings)
}

// ── Inline simulation ─────────────────────────────────────────────────────────

fn simulate_inline(instructions: &[IrInstruction]) -> (bool, Option<String>, Vec<String>) {
    let mut warnings = Vec::new();

    // Check for duplicate AGENT_BIRTH (only one birth per program)
    let birth_count = instructions.iter()
        .filter(|i| i.opcode_name == "AGENT_BIRTH")
        .count();
    if birth_count > 1 {
        return (false, Some(format!("sim: {} AGENT_BIRTH instructions — only one allowed per program", birth_count)), vec![]);
    }

    // Warn on very large programs
    if instructions.len() > 10_000 {
        warnings.push(format!(
            "sim: program has {} instructions; consider splitting",
            instructions.len()
        ));
    }

    // Verify all opcodes are known (opcode byte 0 + non-NOP name = suspicious)
    let known_opcodes: HashSet<&str> = [
        "NOP","PUSH","POP","LOAD_CONST","STORE","LOAD","ADD","SUB","MUL","DIV","MOD",
        "EQ","NEQ","LT","GT","AND","OR","NOT","JUMP","JUMP_IF","CALL","RETURN","HALT",
        "AGENT_BIRTH","AGENT_THINK","AGENT_ACT","AGENT_SENSE","AGENT_MEMORY",
        "EMIT_ASE","BURN_ASE","TRANSFER_ASE","LOCK_ASE",
        "STORE_BLOB","LOAD_BLOB","SEAL_DATA","UNSEAL_DATA",
        "PROPOSE","VOTE","EXECUTE",
        "SIM_STEP","SIM_VERIFY","GPU_CONTRIB",
        "GATE_CHECK","DNA_BIND","CUSTOM",
        // pipeline extras
        "DEFINE","LABEL","DELEGATE","GRANT_TIER","TITHE",
    ].iter().copied().collect();

    for instr in instructions {
        if !instr.opcode_name.is_empty() && !known_opcodes.contains(instr.opcode_name.as_str()) {
            warnings.push(format!(
                "sim: unknown opcode '{}' at line {}",
                instr.opcode_name, instr.line
            ));
        }
    }

    (true, None, warnings)
}

// ── Tiny tilde expander (no external dep) ────────────────────────────────────

fn shellexpand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/{}", home, rest)
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn nop_json() -> String {
        json!([
            {"opcode": 0x00, "opcode_name": "NOP", "args": {}, "line": 1},
            {"opcode": 0x52, "opcode_name": "HALT", "args": {}, "line": 2}
        ]).to_string()
    }

    #[test]
    fn local_deploy_passes() {
        let out = DeployPipeline::run(&nop_json(), "local", 0);
        assert!(out.passed, "steps: {:?}", out.steps.iter().map(|s| (&s.name, &s.error)).collect::<Vec<_>>());
        assert!(out.manifest.is_some());
    }

    #[test]
    fn testnet_tier_0_fails_auth() {
        let out = DeployPipeline::run(&nop_json(), "testnet", 0);
        assert!(!out.passed);
        assert!(out.steps.iter().any(|s| s.name == "AUTHORIZE" && !s.passed));
    }

    #[test]
    fn testnet_tier_2_passes() {
        let out = DeployPipeline::run(&nop_json(), "testnet", 2);
        assert!(out.passed, "steps: {:?}", out.steps.iter().map(|s| (&s.name, &s.error)).collect::<Vec<_>>());
    }

    #[test]
    fn mainnet_queues_pending_approval() {
        let out = DeployPipeline::run(&nop_json(), "mainnet", 5);
        assert!(out.passed);
        assert!(out.pending_approval_path.is_some());
    }

    #[test]
    fn invalid_json_fails_parse() {
        let out = DeployPipeline::run("not json", "local", 0);
        assert!(!out.passed);
        assert_eq!(out.steps[0].name, "PARSE");
        assert!(!out.steps[0].passed);
    }

    #[test]
    fn double_agent_birth_fails_simulate() {
        let prog = json!([
            {"opcode": 0x60, "opcode_name": "AGENT_BIRTH", "args": {"tier": 1}, "line": 1},
            {"opcode": 0x60, "opcode_name": "AGENT_BIRTH", "args": {"tier": 2}, "line": 2},
            {"opcode": 0x52, "opcode_name": "HALT",        "args": {}, "line": 3}
        ]).to_string();
        let out = DeployPipeline::run(&prog, "local", 0);
        assert!(!out.passed);
        assert!(out.steps.iter().any(|s| s.name == "SIMULATE" && !s.passed));
    }
}
