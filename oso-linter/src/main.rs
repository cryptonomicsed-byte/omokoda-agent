//! oso-linter — static analysis for OSO-IR JSON programs.
//!
//! Usage:
//!   oso-linter program.json
//!   cat program.json | oso-linter -
//!
//! Outputs JSON: { "passed": bool, "warnings": [...], "errors": [...] }

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::io::Read;

mod rules;
use rules::{resource, security, semantic};

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "oso-linter",
    about = "Lint an OSO-IR JSON program for semantic, security, and resource issues"
)]
struct Cli {
    /// Path to the OSO-IR JSON file, or '-' to read from stdin
    #[arg(default_value = "-")]
    file: String,
}

// ── Output types ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub rule: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct LintOutput {
    pub passed: bool,
    pub warnings: Vec<Diagnostic>,
    pub errors: Vec<Diagnostic>,
}

// ── OSO-IR input types ────────────────────────────────────────────────────────

/// Minimal OSO-IR program structure — matches omokoda-core/src/oso_ir.rs IrInstruction
/// and the OsoIR contract document from the validator.
#[derive(Debug, Clone, Deserialize)]
pub struct IrInstruction {
    pub opcode: u8,
    pub opcode_name: String,
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub line: usize,
}

/// An OSO-IR program is either:
///   - A flat Vec<IrInstruction>   (raw instruction list)
///   - An IrProgram object with a top-level `instructions` array
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum IrInput {
    /// Bare instruction list: [{ opcode, opcode_name, args, line }, ...]
    List(Vec<IrInstruction>),
    /// Wrapped: { "instructions": [...], "metadata": {...} }
    Wrapped(IrProgram),
}

#[derive(Debug, Clone, Deserialize)]
pub struct IrProgram {
    pub instructions: Vec<IrInstruction>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl IrInput {
    pub fn instructions(&self) -> &[IrInstruction] {
        match self {
            IrInput::List(v) => v,
            IrInput::Wrapped(p) => &p.instructions,
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    let json_text = if cli.file == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .expect("failed to read stdin");
        buf
    } else {
        std::fs::read_to_string(&cli.file).unwrap_or_else(|e| {
            eprintln!("error reading {}: {}", cli.file, e);
            std::process::exit(1);
        })
    };

    let output = lint_json(&json_text);
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    if !output.passed {
        std::process::exit(1);
    }
}

/// Run all lint rules on raw JSON text. Public so tests can call it.
pub fn lint_json(json_text: &str) -> LintOutput {
    let input: IrInput = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(e) => {
            return LintOutput {
                passed: false,
                warnings: vec![],
                errors: vec![Diagnostic {
                    rule: "parse".into(),
                    message: format!("JSON parse error: {}", e),
                    line: None,
                }],
            };
        }
    };

    let instructions = input.instructions();
    let mut warnings: Vec<Diagnostic> = vec![];
    let mut errors: Vec<Diagnostic> = vec![];

    // Run all rule groups
    semantic::check(instructions, &mut warnings, &mut errors);
    security::check(instructions, &mut warnings, &mut errors);
    resource::check(instructions, &mut warnings, &mut errors);

    let passed = errors.is_empty();
    LintOutput {
        passed,
        warnings,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instr(opcode_name: &str, args: serde_json::Value, line: usize) -> serde_json::Value {
        let opcode = match opcode_name {
            "TRANSFER_ASE" => 0x72u8,
            "BURN_ASE" => 0x71,
            "LOCK_ASE" => 0x73,
            "EMIT_ASE" => 0x70,
            "AGENT_BIRTH" => 0x60,
            "JUMP" => 0x40,
            "JUMP_IF" => 0x41,
            _ => 0x00,
        };
        serde_json::json!({
            "opcode": opcode,
            "opcode_name": opcode_name,
            "args": args,
            "line": line,
        })
    }

    #[test]
    fn empty_program_passes() {
        let json = serde_json::json!([]).to_string();
        let out = lint_json(&json);
        assert!(out.passed, "empty program should pass: {:?}", out.errors);
    }

    #[test]
    fn invalid_json_fails() {
        let out = lint_json("not json at all");
        assert!(!out.passed);
        assert!(out.errors[0].rule == "parse");
    }

    #[test]
    fn valid_odu_passes() {
        let prog = serde_json::json!([make_instr(
            "AGENT_BIRTH",
            serde_json::json!({"odu_id": 42, "tier": 3}),
            1
        )])
        .to_string();
        let out = lint_json(&prog);
        assert!(out.passed, "{:?}", out.errors);
    }

    #[test]
    fn invalid_odu_fails() {
        let prog = serde_json::json!([make_instr(
            "AGENT_BIRTH",
            serde_json::json!({"odu_id": 300, "tier": 1}),
            1
        )])
        .to_string();
        let out = lint_json(&prog);
        assert!(!out.passed);
        assert!(out.errors.iter().any(|e| e.rule.contains("semantic")));
    }

    #[test]
    fn resource_warn_on_too_many_transfers() {
        let mut instrs: Vec<serde_json::Value> = vec![];
        for i in 0..12 {
            instrs.push(make_instr(
                "TRANSFER_ASE",
                serde_json::json!({"amount": 1}),
                i,
            ));
        }
        let out = lint_json(&serde_json::json!(instrs).to_string());
        assert!(
            out.warnings.iter().any(|w| w.rule.contains("resource")),
            "expected resource warning: {:?}",
            out.warnings
        );
    }
}
