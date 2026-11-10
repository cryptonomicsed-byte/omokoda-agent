//! Semantic rules:
//!   S1 — odu_id must be 0–255
//!   S2 — tier must be 0–7
//!   S3 — no circular agent references (an agent cannot reference its own agent_id
//!         in a CALL or AGENT_* instruction's target arg)
//!   S4 — opcode_name must match the known opcode table (warn on unknown)

use crate::{Diagnostic, IrInstruction};

pub fn check(
    instructions: &[IrInstruction],
    warnings: &mut Vec<Diagnostic>,
    errors: &mut Vec<Diagnostic>,
) {
    // Collect all agent_ids defined via AGENT_BIRTH for circular-ref check
    let mut declared_agents: Vec<String> = vec![];
    for instr in instructions {
        if instr.opcode_name == "AGENT_BIRTH" {
            if let Some(serde_json::Value::String(id)) = instr.args.get("agent_id") {
                declared_agents.push(id.clone());
            }
        }
    }

    for instr in instructions {
        let line = Some(instr.line);

        // S1: validate odu_id
        if let Some(odu_val) = instr.args.get("odu_id") {
            match odu_val {
                serde_json::Value::Number(n) => {
                    let v = n.as_i64().unwrap_or(-1);
                    if !(0..=255).contains(&v) {
                        errors.push(Diagnostic {
                            rule: "semantic.S1.odu_id_range".into(),
                            message: format!(
                                "odu_id {} is out of range (must be 0–255) at {}",
                                v, instr.opcode_name
                            ),
                            line,
                        });
                    }
                }
                _ => {
                    errors.push(Diagnostic {
                        rule: "semantic.S1.odu_id_type".into(),
                        message: format!(
                            "odu_id must be a number in {}, got {:?}",
                            instr.opcode_name, odu_val
                        ),
                        line,
                    });
                }
            }
        }

        // S2: validate tier
        if let Some(tier_val) = instr.args.get("tier") {
            match tier_val {
                serde_json::Value::Number(n) => {
                    let v = n.as_i64().unwrap_or(-1);
                    if !(0..=7).contains(&v) {
                        errors.push(Diagnostic {
                            rule: "semantic.S2.tier_range".into(),
                            message: format!(
                                "tier {} is out of range (must be 0–7) in {}",
                                v, instr.opcode_name
                            ),
                            line,
                        });
                    }
                }
                _ => {
                    errors.push(Diagnostic {
                        rule: "semantic.S2.tier_type".into(),
                        message: format!(
                            "tier must be a number in {}, got {:?}",
                            instr.opcode_name, tier_val
                        ),
                        line,
                    });
                }
            }
        }

        // S3: circular agent reference
        // Check CALL/AGENT_* target arg against caller's own agent_id
        if matches!(
            instr.opcode_name.as_str(),
            "CALL" | "AGENT_THINK" | "AGENT_ACT"
        ) {
            if let Some(serde_json::Value::String(target)) = instr
                .args
                .get("target")
                .or_else(|| instr.args.get("agent_id"))
            {
                if declared_agents.contains(target) {
                    // Self-call: the target is an agent defined in the same program
                    // This is only an error if the caller_id matches the target
                    if let Some(serde_json::Value::String(caller)) = instr.args.get("caller_id") {
                        if caller == target {
                            errors.push(Diagnostic {
                                rule: "semantic.S3.circular_agent_ref".into(),
                                message: format!(
                                    "{} creates a circular agent reference: agent '{}' calls itself",
                                    instr.opcode_name, target
                                ),
                                line,
                            });
                        }
                    }
                }
            }
        }

        // S4: warn on unknown opcode names (not in canonical table)
        if !is_known_opcode(&instr.opcode_name) {
            warnings.push(Diagnostic {
                rule: "semantic.S4.unknown_opcode".into(),
                message: format!(
                    "unknown opcode_name '{}' (opcode 0x{:02X}) — not in canonical table",
                    instr.opcode_name, instr.opcode
                ),
                line,
            });
        }
    }
}

fn is_known_opcode(name: &str) -> bool {
    matches!(
        name,
        "NOP"
            | "PUSH"
            | "POP"
            | "LOAD_CONST"
            | "STORE"
            | "LOAD"
            | "ADD"
            | "SUB"
            | "MUL"
            | "DIV"
            | "MOD"
            | "EQ"
            | "NEQ"
            | "LT"
            | "GT"
            | "AND"
            | "OR"
            | "NOT"
            | "JUMP"
            | "JUMP_IF"
            | "CALL"
            | "RETURN"
            | "HALT"
            | "AGENT_BIRTH"
            | "AGENT_THINK"
            | "AGENT_ACT"
            | "AGENT_SENSE"
            | "AGENT_MEMORY"
            | "EMIT_ASE"
            | "BURN_ASE"
            | "TRANSFER_ASE"
            | "LOCK_ASE"
            | "STORE_BLOB"
            | "LOAD_BLOB"
            | "SEAL_DATA"
            | "UNSEAL_DATA"
            | "PROPOSE"
            | "VOTE"
            | "EXECUTE"
            | "SIM_STEP"
            | "SIM_VERIFY"
            | "GPU_CONTRIB"
            | "GATE_CHECK"
            | "DNA_BIND"
            | "CUSTOM"
    )
}
