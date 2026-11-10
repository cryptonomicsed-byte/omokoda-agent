//! oso-security — formal capability, resource, and liveness analyzer for OSO-IR programs.
//!
//! Phase 26.5: runs checks BEYOND the oso-linter, including:
//!   - Capability declaration vs usage audit
//!   - Resource bounds (u64 overflow, unbounded loops, memory)
//!   - Formal analysis (termination, liveness, BURN/LOCK safety)
//!   - Full 8-step security pipeline (generate→parse→type→cap→resource→sim→formal→auth→deploy)

pub mod capability;
pub mod formal;
pub mod pipeline;
pub mod report;
pub mod resource;

pub use pipeline::{opcode_for, PipelineResult, SecurityPipeline};
pub use report::SecurityReport;

// ── Shared IR types (self-contained; mirrors oso-linter + oso-simulator) ────────

use serde::{Deserialize, Serialize};

/// Minimal IrInstruction — matches the JSON schema emitted by oso-parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrInstruction {
    pub opcode: u8,
    pub opcode_name: String,
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub line: usize,
}

/// Accept a bare list or a `{ instructions: [...], metadata: {...} }` wrapper.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum IrInput {
    List(Vec<IrInstruction>),
    Wrapped(IrProgram),
}

#[derive(Debug, Clone, Deserialize)]
pub struct IrProgram {
    pub instructions: Vec<IrInstruction>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// Declared capabilities e.g. ["TRANSFER", "DELEGATE"]
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Minimum tier required to run this program (0–7).
    #[serde(default)]
    pub minimum_tier: u8,
}

impl IrInput {
    pub fn into_program(self) -> IrProgram {
        match self {
            IrInput::List(v) => IrProgram {
                instructions: v,
                metadata: serde_json::Value::Null,
                capabilities: vec![],
                minimum_tier: 0,
            },
            IrInput::Wrapped(p) => p,
        }
    }
}

/// Run full security analysis. Public entry point used by the CLI.
pub fn analyze(json_text: &str, strict: bool) -> SecurityReport {
    let program: IrProgram = match serde_json::from_str::<IrInput>(json_text) {
        Ok(v) => v.into_program(),
        Err(e) => {
            return SecurityReport {
                passed: false,
                capability_errors: vec![format!("JSON parse error: {}", e)],
                resource_warnings: vec![],
                formal_errors: vec![],
                risk_score: 1.0,
            };
        }
    };

    let cap_errors = capability::CapabilityChecker::new(&program).check();
    let res_warnings = resource::ResourceChecker::new(&program).check();
    let formal_errors = formal::FormalAnalyzer::new(&program).analyze();

    let risk_score = compute_risk(
        cap_errors.len(),
        res_warnings.len(),
        formal_errors.len(),
        strict,
    );

    // In strict mode resource warnings are treated as errors for pass/fail.
    let passed =
        cap_errors.is_empty() && formal_errors.is_empty() && (!strict || res_warnings.is_empty());

    SecurityReport {
        passed,
        capability_errors: cap_errors,
        resource_warnings: res_warnings,
        formal_errors,
        risk_score,
    }
}

fn compute_risk(cap_errs: usize, res_warns: usize, formal_errs: usize, strict: bool) -> f32 {
    let mut score: f32 = 0.0;
    // Capability errors are high-weight (0.3 each, cap 0.6)
    score += (cap_errs as f32 * 0.3).min(0.6);
    // Formal errors are critical (0.2 each, cap 0.6)
    score += (formal_errs as f32 * 0.2).min(0.6);
    // Resource warnings are low-weight unless strict
    let res_weight = if strict { 0.15 } else { 0.05 };
    score += (res_warns as f32 * res_weight).min(0.3);
    score.min(1.0)
}
