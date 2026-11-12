//! Executor — walks an IrProgram instruction list and dispatches
//! each opcode to MockOsovmState.

use crate::{mock_vm::MockOsovmState, IrInstruction};

/// Execute a slice of IrInstructions against the mock VM.
/// Returns Ok(()) on success or Err(message) on the first fatal error.
pub fn execute(vm: &mut MockOsovmState, instructions: &[IrInstruction]) -> Result<(), String> {
    let mut pc: usize = 0;

    while pc < instructions.len() {
        let instr = &instructions[pc];
        let result = dispatch(vm, instr, &mut pc, instructions);
        match result {
            Err(e) => return Err(format!("line {}: {}", instr.line, e)),
            Ok(ControlFlow::Halt) => return Ok(()),
            Ok(ControlFlow::Jump(target)) => {
                if target >= instructions.len() {
                    return Err(format!(
                        "JUMP target {} out of bounds (len={})",
                        target,
                        instructions.len()
                    ));
                }
                pc = target;
                continue;
            }
            Ok(ControlFlow::Next) => {}
        }
        pc += 1;
    }
    Ok(())
}

enum ControlFlow {
    Next,
    Jump(usize),
    Halt,
}

fn dispatch(
    vm: &mut MockOsovmState,
    instr: &IrInstruction,
    _pc: &mut usize,
    _all: &[IrInstruction],
) -> Result<ControlFlow, String> {
    let args = &instr.args;
    // Cache caller_id as an owned String to avoid borrow conflicts below.
    let caller_id = vm.caller_id.clone();

    match instr.opcode_name.as_str() {
        // ── Control flow ──────────────────────────────────────────────────────
        "NOP" => {}

        "HALT" | "RETURN" => return Ok(ControlFlow::Halt),

        "JUMP" => {
            let target = get_usize(args, "target")?;
            return Ok(ControlFlow::Jump(target));
        }

        "JUMP_IF" => {
            let target = get_usize(args, "target")?;
            // condition: args["condition"] = true/false or 0/1
            let cond = args
                .get("condition")
                .map(|v| match v {
                    serde_json::Value::Bool(b) => *b,
                    serde_json::Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
                    _ => false,
                })
                .unwrap_or(false);
            if cond {
                return Ok(ControlFlow::Jump(target));
            }
        }

        // ── Stack / memory ────────────────────────────────────────────────────
        "PUSH" | "POP" | "LOAD_CONST" | "ADD" | "SUB" | "MUL" | "DIV" | "MOD" | "EQ" | "NEQ"
        | "LT" | "GT" | "AND" | "OR" | "NOT" | "CALL" => {
            // Arithmetic and stack ops are no-ops in the economic sim model
        }

        "STORE" => {
            let key = get_str(args, "key")?;
            let value = args
                .get("value")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            vm.kv_store(key, value);
        }

        "LOAD" => {
            let key = get_str(args, "key")?;
            // Load is a read — result is not captured in this model
            let _ = vm.kv_load(key);
        }

        // ── Agent ops ─────────────────────────────────────────────────────────
        "AGENT_BIRTH" => {
            let agent_id = get_str(args, "agent_id").unwrap_or("did:v:agent:newborn");
            let initial = args
                .get("initial_balance")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            vm.balances.entry(agent_id.to_string()).or_insert(initial);
        }

        "AGENT_THINK" | "AGENT_ACT" | "AGENT_SENSE" | "AGENT_MEMORY" => {
            // Cognitive ops — no ASE cost in base model; future: gas metering
        }

        // ── Economy ops ───────────────────────────────────────────────────────
        "TRANSFER_ASE" => {
            let to = get_str(args, "to")?;
            let from: String = args
                .get("from")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| caller_id.clone());
            let amount = get_u64(args, "amount")?;
            vm.transfer(&from, to, amount)?;
        }

        "BURN_ASE" => {
            let from: String = args
                .get("from")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| caller_id.clone());
            let amount = get_u64(args, "amount")?;
            vm.burn(&from, amount)?;
        }

        "LOCK_ASE" => {
            let from: String = args
                .get("from")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| caller_id.clone());
            let amount = get_u64(args, "amount")?;
            let job_id = get_str(args, "job_id").unwrap_or("job:unknown");
            vm.lock(&from, amount, job_id)?;
        }

        "EMIT_ASE" => {
            let to: String = args
                .get("to")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| caller_id.clone());
            let amount = get_u64(args, "amount")?;
            vm.emit_ase(&to, amount);
        }

        // ── Storage ops ───────────────────────────────────────────────────────
        "STORE_BLOB" => {
            let key = get_str(args, "key")?;
            let data = args
                .get("data")
                .map(|v| v.to_string().into_bytes())
                .unwrap_or_default();
            vm.blobs.insert(key.to_string(), data);
        }

        "LOAD_BLOB" => {
            let _key = get_str(args, "key")?;
            // Result not captured in economic model
        }

        "SEAL_DATA" | "UNSEAL_DATA" => {
            // Cryptographic ops — no-op in sim (no real Seal key)
        }

        // ── Governance ops ────────────────────────────────────────────────────
        "PROPOSE" | "VOTE" | "EXECUTE" => {
            // No-ops in economic sim — governance state not tracked at this level
        }

        // ── Simulation ops ────────────────────────────────────────────────────
        "SIM_STEP" | "SIM_VERIFY" | "GPU_CONTRIB" => {
            // Simulation control — validate twin_f1 threshold if provided
            if instr.opcode_name == "SIM_VERIFY" {
                if let Some(min_f1) = args.get("min_f1").and_then(|v| v.as_f64()) {
                    if vm.twin_f1 < min_f1 {
                        return Err(format!(
                            "SIM_VERIFY failed: twin_f1={:.3} < required min_f1={:.3}",
                            vm.twin_f1, min_f1
                        ));
                    }
                }
            }
        }

        // ── Hermetic ops ─────────────────────────────────────────────────────
        "GATE_CHECK" => {
            if let Some(min_tier) = args.get("min_tier").and_then(|v| v.as_u64()) {
                if (vm.caller_tier as u64) < min_tier {
                    return Err(format!(
                        "GATE_CHECK failed: caller_tier={} < min_tier={}",
                        vm.caller_tier, min_tier
                    ));
                }
            }
        }

        "DNA_BIND" => {
            // No-op in sim — DNA binding requires real chain state
        }

        "CUSTOM" => {
            // Extension opcode — no-op
        }

        unknown => {
            // Unknown opcodes are a warning-level issue, not fatal
            eprintln!(
                "warn: unknown opcode '{}' (0x{:02X}) at line {} — skipping",
                unknown, instr.opcode, instr.line
            );
        }
    }

    Ok(ControlFlow::Next)
}

// ── Argument helpers ──────────────────────────────────────────────────────────

fn get_str<'a>(
    args: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing or non-string arg '{}'", key))
}

fn get_u64(args: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<u64, String> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| format!("missing or non-u64 arg '{}'", key))
}

fn get_usize(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<usize, String> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .ok_or_else(|| format!("missing or non-numeric arg '{}'", key))
}
