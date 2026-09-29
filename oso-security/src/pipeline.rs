//! Security pipeline — Phase 26.7.
//!
//! Full 8-step pipeline:
//!   GENERATE → PARSE → TYPE CHECK → CAP CHECK → RESOURCE CHECK → SIM → FORMAL → AUTH → DEPLOY
//!
//! Each step either passes or fails-fast with the step name and the error message.
//! `SecurityPipeline::run` is the single entry point that wires all tools together.

use serde::{Deserialize, Serialize};
use crate::{capability, formal, resource, IrInput, IrProgram, SecurityReport};

/// Result of a single pipeline step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Final result of the full pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    /// True only if ALL steps passed.
    pub passed: bool,
    /// Steps executed (in order). Stops at first hard failure.
    pub steps: Vec<StepResult>,
    /// The consolidated security report (populated after formal analysis).
    pub report: Option<SecurityReport>,
}

impl PipelineResult {
    #[allow(dead_code)]
    fn failed_at(step: &str, error: &str) -> Self {
        PipelineResult {
            passed: false,
            steps: vec![StepResult {
                step: step.to_string(),
                passed: false,
                error: Some(error.to_string()),
                warnings: vec![],
            }],
            report: None,
        }
    }
}

/// Authorization tiers for deployment targets.
///
/// - local:    tier ≥ 0  (any)
/// - testnet:  tier ≥ 2  (agent-level)
/// - mainnet:  tier ≥ 5  (governance)
fn required_tier_for(target: &str) -> u8 {
    match target {
        "mainnet" => 5,
        "testnet" => 2,
        _         => 0, // local
    }
}

pub struct SecurityPipeline;

impl SecurityPipeline {
    /// Run the full pipeline. Fails-fast at the first hard error.
    pub fn run(program_json: &str, target: &str, signer_tier: u8) -> PipelineResult {
        let mut steps: Vec<StepResult> = Vec::new();

        // ── Step 1: GENERATE (input received) ───────────────────────────────────
        steps.push(StepResult {
            step: "GENERATE".into(),
            passed: true,
            error: None,
            warnings: vec![],
        });

        // ── Step 2: PARSE (JSON → IrProgram) ────────────────────────────────────
        let program: IrProgram = match serde_json::from_str::<IrInput>(program_json) {
            Ok(v) => {
                steps.push(StepResult {
                    step: "PARSE".into(),
                    passed: true,
                    error: None,
                    warnings: vec![],
                });
                v.into_program()
            }
            Err(e) => {
                steps.push(StepResult {
                    step: "PARSE".into(),
                    passed: false,
                    error: Some(format!("JSON parse error: {}", e)),
                    warnings: vec![],
                });
                return PipelineResult { passed: false, steps, report: None };
            }
        };

        // ── Step 3: TYPE CHECK (opcode validity) ────────────────────────────────
        {
            let unknown: Vec<String> = program.instructions.iter()
                .filter(|i| i.opcode == 0 && i.opcode_name != "NOP" && !i.opcode_name.is_empty())
                .filter(|i| crate::opcode_for(&i.opcode_name).is_none())
                .map(|i| format!("unknown opcode '{}' at line {}", i.opcode_name, i.line))
                .collect();

            if !unknown.is_empty() {
                steps.push(StepResult {
                    step: "TYPE_CHECK".into(),
                    passed: false,
                    error: Some(unknown.join("; ")),
                    warnings: vec![],
                });
                return PipelineResult { passed: false, steps, report: None };
            }
            steps.push(StepResult {
                step: "TYPE_CHECK".into(),
                passed: true,
                error: None,
                warnings: vec![],
            });
        }

        // ── Step 4: CAP CHECK ───────────────────────────────────────────────────
        let cap_errors = capability::CapabilityChecker::new(&program).check();
        if !cap_errors.is_empty() {
            steps.push(StepResult {
                step: "CAP_CHECK".into(),
                passed: false,
                error: Some(cap_errors.join("; ")),
                warnings: vec![],
            });
            return PipelineResult { passed: false, steps, report: None };
        }
        steps.push(StepResult {
            step: "CAP_CHECK".into(),
            passed: true,
            error: None,
            warnings: vec![],
        });

        // ── Step 5: RESOURCE CHECK ──────────────────────────────────────────────
        let res_warnings = resource::ResourceChecker::new(&program).check();
        steps.push(StepResult {
            step: "RESOURCE_CHECK".into(),
            passed: true, // resource issues are warnings, not hard failures in pipeline
            error: None,
            warnings: res_warnings.clone(),
        });

        // ── Step 6: SIM (dry-run simulation stub) ───────────────────────────────
        // The pipeline doesn't invoke the full oso-simulator binary (separate crate);
        // it performs a lightweight instruction-count + opcode sanity check here.
        {
            let sim_warnings = simulate_inline(&program);
            steps.push(StepResult {
                step: "SIM".into(),
                passed: true,
                error: None,
                warnings: sim_warnings,
            });
        }

        // ── Step 7: FORMAL ──────────────────────────────────────────────────────
        let formal_errors = formal::FormalAnalyzer::new(&program).analyze();
        if !formal_errors.is_empty() {
            steps.push(StepResult {
                step: "FORMAL".into(),
                passed: false,
                error: Some(formal_errors.join("; ")),
                warnings: vec![],
            });
            let report = SecurityReport {
                passed: false,
                capability_errors: vec![],
                resource_warnings: res_warnings,
                formal_errors,
                risk_score: 0.8,
            };
            return PipelineResult { passed: false, steps, report: Some(report) };
        }
        steps.push(StepResult {
            step: "FORMAL".into(),
            passed: true,
            error: None,
            warnings: vec![],
        });

        // ── Step 8: AUTH ────────────────────────────────────────────────────────
        let required = required_tier_for(target);
        if signer_tier < required {
            let auth_err = format!(
                "AUTH: signer tier {} is insufficient for target '{}' (requires tier {})",
                signer_tier, target, required
            );
            steps.push(StepResult {
                step: "AUTH".into(),
                passed: false,
                error: Some(auth_err),
                warnings: vec![],
            });
            return PipelineResult { passed: false, steps, report: None };
        }
        steps.push(StepResult {
            step: "AUTH".into(),
            passed: true,
            error: None,
            warnings: vec![],
        });

        // ── Step 9: DEPLOY (signal only; actual deploy is oso-deployer) ─────────
        steps.push(StepResult {
            step: "DEPLOY".into(),
            passed: true,
            error: None,
            warnings: vec![format!("ready to deploy to '{}' with tier {}", target, signer_tier)],
        });

        // Build final report
        let risk_score = crate::compute_risk(0, res_warnings.len(), 0, false);
        let report = SecurityReport {
            passed: true,
            capability_errors: vec![],
            resource_warnings: res_warnings,
            formal_errors: vec![],
            risk_score,
        };

        PipelineResult { passed: true, steps, report: Some(report) }
    }
}

/// Inline simulation: count total instructions and check for obvious runtime issues.
fn simulate_inline(program: &IrProgram) -> Vec<String> {
    let mut warnings = Vec::new();
    let count = program.instructions.len();
    if count > 10_000 {
        warnings.push(format!(
            "SIM: program has {} instructions; consider splitting into sub-programs",
            count
        ));
    }
    // Warn if any instruction is missing an opcode_name
    let unnamed = program.instructions.iter().filter(|i| i.opcode_name.is_empty()).count();
    if unnamed > 0 {
        warnings.push(format!("SIM: {} instructions have no opcode_name", unnamed));
    }
    warnings
}

/// Expose the canonical opcode table from oso-parser's ir.rs.
/// Duplicated here to avoid a cross-crate dependency in this MVP.
pub fn opcode_for(name: &str) -> Option<u8> {
    match name {
        "NOP"          => Some(0x00),
        "PUSH"         => Some(0x01),
        "POP"          => Some(0x02),
        "LOAD_CONST"   => Some(0x03),
        "STORE"        => Some(0x04),
        "LOAD"         => Some(0x05),
        "ADD"          => Some(0x10),
        "SUB"          => Some(0x11),
        "MUL"          => Some(0x12),
        "DIV"          => Some(0x13),
        "MOD"          => Some(0x14),
        "EQ"           => Some(0x20),
        "NEQ"          => Some(0x21),
        "LT"           => Some(0x22),
        "GT"           => Some(0x23),
        "AND"          => Some(0x30),
        "OR"           => Some(0x31),
        "NOT"          => Some(0x32),
        "JUMP"         => Some(0x40),
        "JUMP_IF"      => Some(0x41),
        "CALL"         => Some(0x50),
        "RETURN"       => Some(0x51),
        "HALT"         => Some(0x52),
        "AGENT_BIRTH"  => Some(0x60),
        "AGENT_THINK"  => Some(0x61),
        "AGENT_ACT"    => Some(0x62),
        "AGENT_SENSE"  => Some(0x63),
        "AGENT_MEMORY" => Some(0x64),
        "EMIT_ASE"     => Some(0x70),
        "BURN_ASE"     => Some(0x71),
        "TRANSFER_ASE" => Some(0x72),
        "LOCK_ASE"     => Some(0x73),
        "STORE_BLOB"   => Some(0x80),
        "LOAD_BLOB"    => Some(0x81),
        "SEAL_DATA"    => Some(0x82),
        "UNSEAL_DATA"  => Some(0x83),
        "PROPOSE"      => Some(0x90),
        "VOTE"         => Some(0x91),
        "EXECUTE"      => Some(0x92),
        "SIM_STEP"     => Some(0xA0),
        "SIM_VERIFY"   => Some(0xA1),
        "GPU_CONTRIB"  => Some(0xA2),
        "GATE_CHECK"   => Some(0xB0),
        "DNA_BIND"     => Some(0xB1),
        "CUSTOM"       => Some(0xFF),
        _              => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn nop_prog() -> String {
        json!([
            {"opcode": 0x00, "opcode_name": "NOP", "args": {}, "line": 1},
            {"opcode": 0x52, "opcode_name": "HALT", "args": {}, "line": 2}
        ]).to_string()
    }

    #[test]
    fn pipeline_clean_program_passes() {
        let result = SecurityPipeline::run(&nop_prog(), "local", 0);
        assert!(result.passed, "steps: {:?}", result.steps);
    }

    #[test]
    fn pipeline_invalid_json_fails_at_parse() {
        let result = SecurityPipeline::run("not json", "local", 0);
        assert!(!result.passed);
        assert_eq!(result.steps.last().map(|s| s.step.as_str()), Some("PARSE"));
    }

    #[test]
    fn pipeline_mainnet_low_tier_fails_at_auth() {
        let result = SecurityPipeline::run(&nop_prog(), "mainnet", 2);
        assert!(!result.passed);
        assert!(result.steps.iter().any(|s| s.step == "AUTH" && !s.passed));
    }

    #[test]
    fn pipeline_mainnet_sufficient_tier_passes() {
        let result = SecurityPipeline::run(&nop_prog(), "mainnet", 5);
        assert!(result.passed, "steps: {:?}", result.steps);
    }

    #[test]
    fn pipeline_burn_without_lock_fails_formal() {
        let prog = json!([
            {"opcode": 0x71, "opcode_name": "BURN_ASE",
             "args": {"asset": "ASE", "amount": 100}, "line": 1},
            {"opcode": 0x52, "opcode_name": "HALT", "args": {}, "line": 2}
        ]).to_string();
        let result = SecurityPipeline::run(&prog, "local", 0);
        assert!(!result.passed);
        assert!(result.steps.iter().any(|s| s.step == "FORMAL" && !s.passed));
    }
}
