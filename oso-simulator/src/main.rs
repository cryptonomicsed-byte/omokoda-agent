//! oso-sim — dry-run simulator for OSO-IR programs.
//!
//! Usage:
//!   oso-sim program.json
//!   oso-sim program.json --twin-f1 0.8 --agent-tier 3
//!   cat program.json | oso-sim -
//!
//! Outputs JSON SimReport to stdout.

mod executor;
mod mock_vm;
mod report;

use clap::Parser;
use serde::Deserialize;
use std::io::Read;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "oso-sim",
    about = "Dry-run simulate an OSO-IR program against a mock OSOVM (no network)"
)]
struct Cli {
    /// Path to the OSO-IR JSON file, or '-' for stdin
    #[arg(default_value = "-")]
    file: String,

    /// Spatial twin F1 match score (0.0–1.0) injected into the VM environment
    #[arg(long, default_value = "1.0")]
    twin_f1: f64,

    /// Caller's agent tier (0–7)
    #[arg(long, default_value = "1")]
    agent_tier: u8,

    /// Initial ASE balance for the caller agent
    #[arg(long, default_value = "1000")]
    initial_balance: u64,

    /// Agent ID of the simulated caller
    #[arg(long, default_value = "did:v:agent:sim")]
    agent_id: String,
}

// ── OSO-IR input ──────────────────────────────────────────────────────────────

/// Minimal IrInstruction — mirrors oso-parser/src/ir.rs IrInstruction.
#[derive(Debug, Clone, Deserialize)]
pub struct IrInstruction {
    pub opcode: u8,
    pub opcode_name: String,
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub line: usize,
}

/// Accept either a bare list or a wrapped `{ instructions: [...] }` document.
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
}

impl IrInput {
    pub fn into_instructions(self) -> Vec<IrInstruction> {
        match self {
            IrInput::List(v) => v,
            IrInput::Wrapped(p) => p.instructions,
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

    let report = simulate_json(
        &json_text,
        cli.twin_f1,
        cli.agent_tier,
        cli.initial_balance,
        &cli.agent_id,
    );

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    if report.error.is_some() {
        std::process::exit(1);
    }
}

/// Simulate an OSO-IR program. Public for tests.
pub fn simulate_json(
    json_text: &str,
    twin_f1: f64,
    agent_tier: u8,
    initial_balance: u64,
    agent_id: &str,
) -> report::SimReport {
    let input: IrInput = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(e) => {
            return report::SimReport {
                final_state: serde_json::json!({}),
                receipts_emitted: vec![],
                ase_spent: 0,
                ase_earned: 0,
                error: Some(format!("JSON parse error: {}", e)),
            };
        }
    };

    let instructions = input.into_instructions();
    let mut vm = mock_vm::MockOsovmState::new(agent_id, initial_balance, agent_tier, twin_f1);
    let exec_result = executor::execute(&mut vm, &instructions);

    report::SimReport {
        final_state: vm.snapshot(),
        receipts_emitted: vm.receipts.clone(),
        ase_spent: vm.ase_spent,
        ase_earned: vm.ase_earned,
        error: exec_result.err(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nop_program() -> String {
        serde_json::json!([
            {"opcode": 0x00, "opcode_name": "NOP", "args": {}, "line": 1}
        ])
        .to_string()
    }

    fn transfer_program(amount: u64) -> String {
        serde_json::json!([
            {"opcode": 0x72, "opcode_name": "TRANSFER_ASE",
             "args": {"to": "did:v:agent:recv", "amount": amount}, "line": 1}
        ])
        .to_string()
    }

    #[test]
    fn nop_produces_no_receipts() {
        let r = simulate_json(&nop_program(), 1.0, 1, 1000, "did:v:agent:sim");
        assert!(r.error.is_none());
        assert_eq!(r.receipts_emitted.len(), 0);
        assert_eq!(r.ase_spent, 0);
    }

    #[test]
    fn transfer_ase_decrements_balance() {
        let r = simulate_json(&transfer_program(100), 1.0, 1, 1000, "did:v:agent:sim");
        assert!(r.error.is_none(), "error: {:?}", r.error);
        assert_eq!(r.ase_spent, 100);
    }

    #[test]
    fn transfer_insufficient_balance_errors() {
        let r = simulate_json(&transfer_program(5000), 1.0, 1, 100, "did:v:agent:sim");
        assert!(r.error.is_some());
    }

    #[test]
    fn empty_program_ok() {
        let r = simulate_json("[]", 1.0, 0, 0, "did:v:agent:empty");
        assert!(r.error.is_none());
    }
}
