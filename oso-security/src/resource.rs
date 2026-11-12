//! Resource checker — RES1, RES2, RES3.
//!
//! RES1: Total ASE spend (sum of all TRANSFER + TITHE amounts) must fit in u64
//!       without overflow. Detects programs that would wrap around.
//! RES2: Unbounded loop heuristic — if any JUMP/JUMP_IF back-edge has an
//!       estimated iteration count > 1000, flag it.
//! RES3: Memory estimate — if instruction count implies > 4096 live operand
//!       slots (from BTreeMap/Vec growth), flag a warning.

use crate::IrProgram;

/// Economy opcodes whose `amount` arg contributes to ASE spend.
const SPEND_OPCODES: &[&str] = &["TRANSFER_ASE", "TITHE", "BURN_ASE", "LOCK_ASE"];

/// Maximum safe iteration estimate before warning.
const MAX_LOOP_ITERS: u64 = 1000;

/// Maximum estimated live memory slots before warning.
const MAX_MEMORY_SLOTS: usize = 4096;

pub struct ResourceChecker<'a> {
    program: &'a IrProgram,
}

impl<'a> ResourceChecker<'a> {
    pub fn new(program: &'a IrProgram) -> Self {
        Self { program }
    }

    /// Run all resource checks and return warning strings.
    pub fn check(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        self.check_res1_ase_overflow(&mut warnings);
        self.check_res2_unbounded_loops(&mut warnings);
        self.check_res3_memory(&mut warnings);
        warnings
    }

    /// RES1: Saturating sum of all `amount` args on spend opcodes.
    /// Reports if the raw (non-saturating) sum would exceed u64::MAX.
    fn check_res1_ase_overflow(&self, warnings: &mut Vec<String>) {
        let mut total: u128 = 0; // use u128 to detect u64 overflow
        for instr in &self.program.instructions {
            if SPEND_OPCODES.contains(&instr.opcode_name.as_str()) {
                let amount = instr
                    .args
                    .get("amount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                total += amount as u128;
            }
        }
        if total > u64::MAX as u128 {
            warnings.push(format!(
                "RES1: total ASE spend {} exceeds u64::MAX ({}); program would overflow on-chain counters",
                total,
                u64::MAX
            ));
        }
    }

    /// RES2: Estimate loop iterations from JUMP back-edges.
    /// A back-edge is a JUMP/JUMP_IF whose `target` index is ≤ the instruction's own index.
    fn check_res2_unbounded_loops(&self, warnings: &mut Vec<String>) {
        let instrs = &self.program.instructions;
        for (i, instr) in instrs.iter().enumerate() {
            if !matches!(instr.opcode_name.as_str(), "JUMP" | "JUMP_IF") {
                continue;
            }
            let target = match instr.args.get("target").and_then(|v| v.as_u64()) {
                Some(t) => t as usize,
                None => continue,
            };
            if target <= i {
                // Back-edge: estimate loop body size and iterations
                let loop_body_len = i - target + 1;
                // Heuristic: without explicit bound arg, assume worst-case unbounded
                let bound = instr
                    .args
                    .get("max_iters")
                    .or_else(|| instr.args.get("iterations"))
                    .and_then(|v| v.as_u64());
                let estimated_iters = bound.unwrap_or(u64::MAX);

                if estimated_iters > MAX_LOOP_ITERS {
                    warnings.push(format!(
                        "RES2: potential unbounded loop at instruction {} (line {}) — \
                         back-edge to index {}, body size={} instructions, \
                         estimated iterations={} (threshold={})",
                        i,
                        instr.line,
                        target,
                        loop_body_len,
                        if estimated_iters == u64::MAX {
                            "unbounded".to_string()
                        } else {
                            estimated_iters.to_string()
                        },
                        MAX_LOOP_ITERS
                    ));
                }
            }
        }
    }

    /// RES3: Estimate live memory slots from instruction count.
    /// Each STORE/STORE_BLOB/PUSH instruction contributes one slot.
    fn check_res3_memory(&self, warnings: &mut Vec<String>) {
        const MEMORY_OPCODES: &[&str] = &["STORE", "STORE_BLOB", "PUSH", "AGENT_MEMORY"];
        let slot_count = self
            .program
            .instructions
            .iter()
            .filter(|i| MEMORY_OPCODES.contains(&i.opcode_name.as_str()))
            .count();
        if slot_count > MAX_MEMORY_SLOTS {
            warnings.push(format!(
                "RES3: estimated {} live memory slots (threshold={}) — \
                 program may exhaust OSOVM working memory",
                slot_count, MAX_MEMORY_SLOTS
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IrInstruction, IrProgram};
    use serde_json::json;

    fn make_instr(name: &str, args: serde_json::Value, line: usize) -> IrInstruction {
        IrInstruction {
            opcode: 0,
            opcode_name: name.into(),
            args: if let serde_json::Value::Object(m) = args {
                m
            } else {
                Default::default()
            },
            line,
        }
    }

    fn bare_prog(instrs: Vec<IrInstruction>) -> IrProgram {
        IrProgram {
            instructions: instrs,
            metadata: serde_json::Value::Null,
            capabilities: vec![],
            minimum_tier: 0,
        }
    }

    #[test]
    fn res1_normal_amounts_no_warning() {
        let p = bare_prog(vec![
            make_instr("TRANSFER_ASE", json!({"amount": 100}), 1),
            make_instr("TRANSFER_ASE", json!({"amount": 200}), 2),
        ]);
        let warns = ResourceChecker::new(&p).check();
        assert!(warns.iter().all(|w| !w.contains("RES1")));
    }

    #[test]
    fn res1_overflow_detected() {
        // Two amounts that together exceed u64::MAX
        let max = u64::MAX;
        let p = bare_prog(vec![
            make_instr("TRANSFER_ASE", json!({"amount": max}), 1),
            make_instr("TRANSFER_ASE", json!({"amount": 1u64}), 2),
        ]);
        let warns = ResourceChecker::new(&p).check();
        assert!(warns.iter().any(|w| w.contains("RES1")));
    }

    #[test]
    fn res2_forward_jump_no_warning() {
        // JUMP to index 5 from index 1 = forward jump, safe
        let p = bare_prog(vec![
            make_instr("NOP", json!({}), 1),
            make_instr("JUMP", json!({"target": 5u64}), 2),
        ]);
        let warns = ResourceChecker::new(&p).check();
        assert!(warns.iter().all(|w| !w.contains("RES2")));
    }

    #[test]
    fn res2_back_edge_warns() {
        // JUMP from index 3 back to index 0 = unbounded loop
        let p = bare_prog(vec![
            make_instr("NOP", json!({}), 1),
            make_instr("NOP", json!({}), 2),
            make_instr("NOP", json!({}), 3),
            make_instr("JUMP", json!({"target": 0u64}), 4),
        ]);
        let warns = ResourceChecker::new(&p).check();
        assert!(warns.iter().any(|w| w.contains("RES2")));
    }
}
