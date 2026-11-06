//! Capability checker — CAP1, CAP2, CAP3.
//!
//! CAP1: Every capability *used* in instructions must be listed in the
//!       program's declared `capabilities` array.
//! CAP2: No instruction may require a tier higher than `minimum_tier`.
//! CAP3: Self-escalation: the `caller` of a DELEGATE or GRANT_TIER cannot
//!       also be the `recipient` (same agent ID string).

use crate::IrProgram;

/// Maps opcode_name → the capability string it requires.
fn required_capability(opcode_name: &str) -> Option<&'static str> {
    match opcode_name {
        "TRANSFER_ASE" => Some("TRANSFER"),
        "BURN_ASE" => Some("BURN"),
        "EMIT_ASE" => Some("EMIT"),
        "LOCK_ASE" => Some("LOCK"),
        "PROPOSE" => Some("GOVERN"),
        "VOTE" => Some("GOVERN"),
        "EXECUTE" => Some("GOVERN"),
        "SEAL_DATA" => Some("SEAL"),
        "UNSEAL_DATA" => Some("SEAL"),
        "GPU_CONTRIB" => Some("COMPUTE"),
        "SIM_STEP" => Some("SIMULATE"),
        "SIM_VERIFY" => Some("SIMULATE"),
        "AGENT_BIRTH" => Some("BIRTH"),
        _ => None,
    }
}

/// Minimum tier required by specific opcodes.
fn opcode_min_tier(opcode_name: &str) -> u8 {
    match opcode_name {
        "PROPOSE" | "VOTE" | "EXECUTE" => 5, // governance
        "AGENT_BIRTH" => 2,
        "GPU_CONTRIB" => 3,
        "SEAL_DATA" | "UNSEAL_DATA" => 2,
        _ => 0,
    }
}

pub struct CapabilityChecker<'a> {
    program: &'a IrProgram,
}

impl<'a> CapabilityChecker<'a> {
    pub fn new(program: &'a IrProgram) -> Self {
        Self { program }
    }

    /// Run all three capability checks and return a list of error strings.
    pub fn check(&self) -> Vec<String> {
        let mut errors = Vec::new();
        self.check_cap1(&mut errors);
        self.check_cap2(&mut errors);
        self.check_cap3(&mut errors);
        errors
    }

    /// CAP1: capability declaration vs usage.
    fn check_cap1(&self, errors: &mut Vec<String>) {
        // Only check if the program declared at least one capability.
        // Programs with an empty capability list are treated as "unconstrained"
        // (legacy / raw IR dumps). This avoids false positives on bare instruction lists.
        if self.program.capabilities.is_empty() {
            return;
        }

        let declared: std::collections::HashSet<&str> = self
            .program
            .capabilities
            .iter()
            .map(|s| s.as_str())
            .collect();

        for instr in &self.program.instructions {
            if let Some(cap) = required_capability(&instr.opcode_name) {
                if !declared.contains(cap) {
                    errors.push(format!(
                        "CAP1: instruction '{}' at line {} requires capability '{}' \
                         but it is not declared in the program capabilities list {:?}",
                        instr.opcode_name, instr.line, cap, self.program.capabilities
                    ));
                }
            }
        }
    }

    /// CAP2: no instruction may require a tier higher than minimum_tier.
    fn check_cap2(&self, errors: &mut Vec<String>) {
        for instr in &self.program.instructions {
            let required = opcode_min_tier(&instr.opcode_name);
            if required > self.program.minimum_tier {
                errors.push(format!(
                    "CAP2: instruction '{}' at line {} requires tier {} \
                     but program declares minimum_tier={}",
                    instr.opcode_name, instr.line, required, self.program.minimum_tier
                ));
            }
        }
    }

    /// CAP3: self-escalation via DELEGATE or GRANT_TIER.
    fn check_cap3(&self, errors: &mut Vec<String>) {
        for instr in &self.program.instructions {
            if !matches!(instr.opcode_name.as_str(), "DELEGATE" | "GRANT_TIER") {
                continue;
            }
            let caller = instr.args.get("caller").and_then(|v| v.as_str());
            let recipient = instr
                .args
                .get("recipient")
                .or_else(|| instr.args.get("to"))
                .and_then(|v| v.as_str());

            if let (Some(c), Some(r)) = (caller, recipient) {
                if c == r {
                    errors.push(format!(
                        "CAP3: self-escalation at line {} — '{}' grants to itself (caller=recipient='{}')",
                        instr.line, instr.opcode_name, c
                    ));
                }
            }
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

    fn prog(instrs: Vec<IrInstruction>, caps: Vec<&str>, min_tier: u8) -> IrProgram {
        IrProgram {
            instructions: instrs,
            metadata: serde_json::Value::Null,
            capabilities: caps.into_iter().map(|s| s.to_string()).collect(),
            minimum_tier: min_tier,
        }
    }

    #[test]
    fn cap1_missing_transfer_capability() {
        let p = prog(
            vec![make_instr("TRANSFER_ASE", json!({"amount": 100}), 1)],
            vec!["BURN"], // TRANSFER not declared
            0,
        );
        let errs = CapabilityChecker::new(&p).check();
        assert!(errs.iter().any(|e| e.contains("CAP1")));
    }

    #[test]
    fn cap1_declared_capability_passes() {
        let p = prog(
            vec![make_instr("TRANSFER_ASE", json!({"amount": 100}), 1)],
            vec!["TRANSFER"],
            0,
        );
        let errs = CapabilityChecker::new(&p).check();
        assert!(errs.iter().all(|e| !e.contains("CAP1")));
    }

    #[test]
    fn cap2_tier_too_low() {
        // PROPOSE requires tier 5 but program says minimum_tier=2
        let p = prog(vec![make_instr("PROPOSE", json!({}), 3)], vec!["GOVERN"], 2);
        let errs = CapabilityChecker::new(&p).check();
        assert!(errs.iter().any(|e| e.contains("CAP2")));
    }

    #[test]
    fn cap2_tier_sufficient_passes() {
        let p = prog(vec![make_instr("PROPOSE", json!({}), 3)], vec!["GOVERN"], 5);
        let errs = CapabilityChecker::new(&p).check();
        assert!(errs.iter().all(|e| !e.contains("CAP2")));
    }

    #[test]
    fn cap3_self_escalation_delegate() {
        let p = prog(
            vec![make_instr(
                "DELEGATE",
                json!({"caller": "did:v:agent:alice", "recipient": "did:v:agent:alice"}),
                5,
            )],
            vec![],
            0,
        );
        let errs = CapabilityChecker::new(&p).check();
        assert!(errs.iter().any(|e| e.contains("CAP3")));
    }

    #[test]
    fn cap3_different_recipient_passes() {
        let p = prog(
            vec![make_instr(
                "DELEGATE",
                json!({"caller": "did:v:agent:alice", "recipient": "did:v:agent:bob"}),
                5,
            )],
            vec![],
            0,
        );
        let errs = CapabilityChecker::new(&p).check();
        assert!(errs.iter().all(|e| !e.contains("CAP3")));
    }
}
